# Lot 33 — le recadrage et la fenêtre suivent le viewport, et la barre des tâches sort du cadre

**31 août 2026.** Branche `package-nivuus`. Commits `ff8f436` (extraction) et
`25d6d78` (correction).

Symptômes rapportés par le propriétaire, qui testait en direct : **« des
bordures noires de chaque côté »** et **« ça ne resize pas la fenêtre Notepad
quand je resize la PWA »**.

✅ **DÉPLOYÉ LE 31 AOÛT 2026 À 11:48, SUR AUTORISATION EXPLICITE DU PROPRIÉTAIRE**, qui a choisi le moment en sachant que son hub se viderait. Voir le § 7. *(La phrase qui suivait ici — « RIEN N'EST DÉPLOYÉ PAR CE LOT » — était vraie à la rédaction et ne l'est plus : elle est remplacée plutôt que laissée à contredire le § 7.)*

⚠️ **Ce qui reste vrai du paragraphe d'origine :** L'agent n'a pas été redémarré, aucune
fenêtre du propriétaire n'a été touchée, aucune recette navigateur n'a été
jouée — le rôle `client` est exclusif et il était connecté. Le binaire croisé
est bâti et attesté ; sa mise en service appartient au propriétaire.

---

## 1. Ce qui a été MESURÉ

Toutes les mesures ci-dessous sont des **lectures** de l'agent en production
(`C:\nivuus\agent.log`, tenu par les 5 processus vivants) par
`installer/console/guest/winrm_exec.py`. Aucune écriture, aucune tâche
planifiée, aucun redémarrage.

### 1.1 Le garde jette TOUT, et l'écart d'aspect est réel

`Select-String -SimpleMatch 'redimensionnement ignor'` sur le journal entier
rend **34** lignes. Distribution des tailles demandées, ANSI retiré :

```
Count Name
   15 778x491   ratio=1,5845
    8 1723x1303 ratio=1,3223
    2 1580x1197 ratio=1,3200
    2 1098x978  ratio=1,1227
    1 1201x978  ratio=1,2280
    1 1580x978  ratio=1,6155
    1 1580x1237 ratio=1,2773
    1 1098x938  ratio=1,1706
    1 5118x1438 ratio=3,5591
    1 1081x978  ratio=1,1053
    1 1400x978  ratio=1,4315
```

**Rapports demandés : de 1,105 à 3,559. Rapport servi : 1428/1080 = 1,3222**,
constant, relevé indépendamment dans le champ `vers` du placement périodique.

🔴 **L'ATTENDU NE VIENT PAS DE LA MESURE QU'IL JUGE.** Le `1428x1080` est lu
dans une AUTRE ligne du journal que celles qu'on juge (le placement périodique,
pas le garde), et il est corroboré par un troisième relevé : les origines des
sorties sont 1280, 3140, 5000, 6860, **espacées d'exactement 1860**.

### 1.2 Le `#remote` peint les bandes lui-même

`client/src/style.css` :

```css
#remote { width: 100vw; height: 100vh; object-fit: contain;
          background: var(--video-letterbox); }
```

et `client/src/design/tokens/couleurs.css` : `--video-letterbox: #000`.

**Les bandes noires ne sont pas une métaphore : c'est un token**, peint par
`object-fit: contain` partout où le rapport servi diffère du rapport du
conteneur. Le `<video>` faisant `100vw × 100vh`, `video.clientWidth` mesure
bien le CONTENEUR, donc l'écart est réel et non un artefact de mesure.

### 1.3 Une hypothèse à moi, RÉFUTÉE par la mesure

J'avais lu « 30 430 replacements sur 60 000 lignes » et conclu à une **bataille
permanente** du placement périodique. **C'est faux.** Restreint à la journée du
31 août, le même relevé rend **6** replacements, en deux bourrées. Les 30 430
couvrent plusieurs jours, et leur cas dominant est `1860x1080 -> 1860x1080` :
un replacement de POSITION, pas de taille. Le `1428x1039 -> 1428x1080` du jour
est un **transitoire**, pas un régime.

Consigné parce que je m'en étais servi comme d'un argument avant de le
vérifier.

### 1.4 Le trajet du `Resize`, établi par lecture de code

| Étape | Où |
| --- | --- |
| `ResizeObserver` (lissage 200 ms) | `client/src/resize-dom.ts` |
| `ClientControl::Resize` sur le canal de contrôle | → l'**ENFANT** |
| `Session::appliquer_redimensionnement` | `transport/redimensionnement.rs` |
| `SourceDistante::resize` → `VersCapteur::Redimensionner` | tube nommé |
| `servir_les_commandes` → `WindowsSource::resize` | le **CAPTEUR** |
| **le garde retournait `Ok(())`** | `windows_source/redimensionnement.rs` |

Et en parallèle, la fenêtre Windows appartient au **SUPERVISEUR**, qui la
repose **chaque seconde** (`PERIODE_PLACEMENT`) sur
`Table::taille_sortie_de` — une valeur posée à la création et **jamais
rafraîchie** : `Table::rafraichir_taille_sortie` existait, avec ses tests, et
**n'avait plus aucun appelant depuis D10**.

🔴 **Les deux moitiés du défaut se couvraient l'une l'autre** : le superviseur
ignorait tout viewport reçu sur une session `Vivante` (`viewport_recu` rendait
`Vec::new()`), et le client n'en réannonçait aucun (le `postMessage` de
`main.ts` ne partait qu'au chargement). Aucune des deux n'était visible depuis
l'autre.

### 1.5 La barre des tâches — mesure d'un lot voisin, reprise ici

`docs/superpowers/plans/2026-08-31-barre-des-taches-diagnostic.md`, session 1 :
`mon=1428x1080  work=1428x1032`, `Shell_SecondaryTrayWnd 1428x48` sur **chacune
des deux sorties servies**, et **`rcWork` n'était lu nulle part dans le dépôt**.
Les deux fonctions à corriger étaient exactement les miennes.

---

## 2. Le remède, et pourquoi il ne peut pas se battre avec lui-même

**Une règle pure, deux processus, une seule mesure côté client.**

```
windows_source_sortie::taille_pour_viewport(demande, borne)
  = taille_retenue(borner_a_la_taille_max(demande), borne)

windows_source_sortie::borne_de_la_sortie(moniteur, travail)
  = la ZONE DE TRAVAIL si Windows la donne et qu'elle n'est pas dégénérée,
    le rectangle du moniteur sinon (= le comportement d'avant le lot)
```

- Le **CAPTEUR** (`WindowsSource::suivre_le_viewport`) retaille la fenêtre,
  refait le recadrage et l'encodeur — **sans jamais relâcher la duplication**,
  ce qui serait le correctif C1 de D1 défait. Il lit sa borne par
  `window::zones_du_moniteur_de(hwnd)`.
- Le **SUPERVISEUR** (`Effet::SuivreLeViewport` →
  `placement_periodique::suivre_le_viewport`) corrige `taille_sortie` et repose
  la fenêtre. Il lit sa borne par `window::zones_du_moniteur_au_point(origine)`.
- **Même `HMONITOR`, même borne, même règle.** Le client leur envoie **une
  seule mesure** (`resize-dom.ts` émet le `Resize` ET le `viewport` du même
  `taille`), donc le geste du second arrivé est un `no-op`.

⚠️ **Si les deux divergeaient**, le superviseur l'emporterait au tour suivant :
un désaccord **borné à une seconde**, jamais une oscillation.

🔴 **CE N'EST PAS LA RÉSURRECTION DU CHEMIN QUE D9 A RETIRÉ.** D8 faisait suivre
**la SORTIE** au viewport par `ChangeDisplaySettingsExW` ; D9 l'a mesuré et
retiré. **Ici aucun mode d'affichage n'est changé et le registre n'est pas
touché** : seuls le recadrage et la fenêtre bougent, à l'intérieur d'une sortie
immobile.

---

## 3. Les rouges, vues rouges

| Mutation | Attendu | Obtenu |
| --- | --- | --- |
| `taille_pour_viewport` ignore sa demande | rouge | **3 tests** rouges |
| `borne_de_la_sortie` ignore la zone de travail | rouge | **1** rouge |
| branche `Vivante` → `Vec::new()` (hier) | rouge | **2** rouges |
| le client cesse de réannoncer | rouge | **2** rouges |

Restaurations depuis des **copies nommées** (`/var/tmp/lot33/copie-*`), jamais
par `git checkout --`.

### 3.1 Un contrôle VACUEUX, trouvé en jouant la mutation

`un_viewport_rejoue_ne_fait_avancer_aucune_machine` assertait
`rejeu.iter().all(matches!(SuivreLeViewport))` — et **`all()` est vrai d'un
vecteur vide**. Le comportement d'hier le laissait donc VERT. Un test qui
assère la PRÉSENCE de l'effet a été ajouté
(`un_viewport_sur_une_session_vivante_demande_de_suivre`), et c'est lui qui
rougit à la mutation 3.

### 3.2 Un attendu à moi, faux, attrapé par le test

Le premier jet attendait `(1723, 1080)` pour `taille_pour_viewport((1723,1303),
(1860,1080))`, **en croyant `borner_a_la_taille_max` un écrêtage axe par axe**.
C'en est un de **MISE À L'ÉCHELLE, à rapport d'aspect PRÉSERVÉ** : il rend
`(1428, 1080)`.

🔴 **Cela change le diagnostic, et c'est inscrit dans le test** : les bandes
noires **ne viennent pas du plafond**, qui respecte l'aspect demandé, mais du
**GEL** de la taille retenue à l'ouverture.

---

## 4. Le corollaire « créer la sortie plus généreusement » — JUGÉ, et écarté

**Non appliqué**, pour trois raisons, dont deux sont des mesures :

1. **D8 a établi qu'une sortie ne naît pas à la taille demandée** — elle naît à
   la dernière taille laissée au registre. En demander plus n'est pas établi en
   donner plus.
2. La relation entre taille demandée, `GetDesc().DesktopCoordinates` (1428) et
   `DXGI_OUTDUPL_DESC` (1860) **n'est pas comprise** : trois nombres pour une
   sortie créée à 1428. Changer l'entrée de cette boîte noire sans la
   comprendre, c'est parier.
3. `TAILLE_MAX_SORTIE` n'est **pas calibrée**, et D6 a mesuré le décodeur du
   navigateur saturé dès huit fenêtres de 720p.

C'est **la seule voie connue pour GRANDIR au-delà de la sortie**, et elle reste
donc ouverte — inscrite en legs.

---

## 5. Ce que ce lot N'ÉTABLIT PAS

- 🔴 **AUCUNE SESSION NE L'A EXERCÉ.** Le rôle `client` est exclusif et le
  propriétaire était connecté tout du long. Rien n'est déployé, l'agent n'a pas
  été redémarré. **Le remède est éprouvé sur l'hôte et attesté dans le binaire,
  jamais vu à l'œuvre.**
- 🔴 **PERSONNE N'A REGARDÉ UNE IMAGE.** Aucun jugement visuel n'est porté, ni
  sur les bandes, ni sur la barre des tâches, ni sur le contenu retrouvé.
- 🔴 **UN VIEWPORT PLUS LARGE QUE LA BORNE GARDE SES BANDES NOIRES.**
  `taille_retenue` borne par un `min` axe par axe. Le `5118x1438` mesuré sort à
  `1428x538`. Un test le fige pour que personne ne croie le cas fermé.
- 🔴 **L'ÉCART desktop/texture (1428 contre 1860 en largeur) N'EST NI CRÉÉ NI
  CORRIGÉ ICI.** Le rapport entre ce que montre l'image et ce que couvre la
  fenêtre reste exactement celui d'avant. C'est le terrain du lot 32T.
- ⚠️ **Un redimensionnement demandé pendant le SOMMEIL d'une fenêtre est à
  nouveau perdu** — il ne l'était plus depuis que `resize` était un `no-op`. Le
  rejeu du client (`RejeuResize`) devrait le rattraper au `Resize` suivant :
  **non mesuré**, dit comme tel dans `commandes.rs`.
- ⚠️ **`verify-all.sh` rend 2 étapes en échec**, PRÉEXISTANTES et étrangères à
  ce lot : un secret en clair dans
  `docs/superpowers/plans/journaux-lot31/instrument/pilote-lot31.mjs`, commité
  en `541003c`.

---

## 6. Legs ouverts

- 🔴 **GRANDIR AU-DELÀ DE LA SORTIE reste impossible.** Seule voie connue :
  créer la sortie à `TAILLE_MAX_SORTIE`. Écarté ci-dessus, faute de comprendre
  la boîte noire du pilote. **Décision du propriétaire.**
- 🔴 **`client/src/main.ts` A FRANCHI SON PLAFOND POUR LA DEUXIÈME FOIS** par
  une addition d'une vingtaine de lignes ; il était à **500 EXACTEMENT** à HEAD.
  Troisième extraction (`viewport-dom.ts`). **Le prochain qui y ajoute quoi que
  ce soit doit extraire d'abord.** Idem `superviseur/placement.rs`, porté à 500
  pile par une correction de commentaire, extrait dans la foulée.
- ⚠️ **Les deux extractions de ce lot ont SUIVI leur addition** au lieu de la
  précéder, contrairement à celle de `windows_source/sortie.rs`. Dit plutôt que
  maquillé : les deux besoins sont apparus à mi-lot, avec la mesure de la barre
  des tâches.


---

## 7. Le déploiement (31 août 2026, 11:48)

**Autorisé explicitement par le propriétaire**, qui a choisi le moment.

### 7.1 Une coupling que le plan de déploiement ne nommait pas

🔴 **LES DEUX MOITIÉS DOIVENT PARTIR ENSEMBLE, ET DÉPLOYER `agent.exe` SEUL
AURAIT ÉTÉ PIRE QUE DE NE RIEN FAIRE.** Le capteur aurait retaillé fenêtre et
recadrage ; le superviseur, dont la table garde l'ancienne taille tant que le
client ne la réannonce pas, l'aurait reposée **chaque seconde**. Le symptôme
aurait été une oscillation à 1 Hz — le « ça marche puis ça revient » que le
cadrage du lot redoutait, produit par le déploiement et non par le code.

**Ordre retenu : le client D'ABORD** (un vieux superviseur ignore simplement le
`viewport` d'une session vivante — sans effet), **puis l'agent**.

### 7.2 Le client

| | |
| --- | --- |
| Témoin | `type:"viewport"` dans le bundle `main-*.js` : **1 avant, 2 après** — l'ancien déploiement est son propre témoin négatif |
| Source → destination | `sha256 332f089df2433dc6…` des **deux** côtés |
| Droits | `chmod -R a+rX` refait — `DynamicUser=yes` rend un UID éphémère, et un incident réel de page blanche l'a déjà coûté |
| Sauvegarde | `/var/tmp/lot33/dist-avant-lot33` (25 fichiers) |

### 7.3 L'agent — déposé PAR LE PACKAGE

`hooks/agent_payload.py::deposer_agent_console`, `NIVUUS_PACKAGES_DIR` pointé
sur le checkout `installer` (le défaut `/opt/nivuus-packages` n'existe pas sur
cette machine, et le hook **lève** en le nommant plutôt que de déposer ailleurs
en silence).

| | |
| --- | --- |
| Payload `console` | `03e2e752…` → **`021556b8947edd80`** |
| Identité à ma fabrication | `cmp` : **identiques à l'octet près** |
| **À DESTINATION** (`C:\nivuus\agent\agent.exe`) | **`021556b8947edd80`** — vérifié par `Get-FileHash` DANS la VM, pas à la source |
| Binaire remplacé | `12040beaae905b3d` (celui du lot 32T), **conservé** en `agent.exe.avant-lot33` |

⚠️ **La taille ne prouve rien et n'a servi à rien** : 20 487 090 → 20 517 753
octets, un écart qu'une compilation quelconque produirait.

### 7.4 Les cinq témoins, sur le binaire EN PLACE — et l'ancien en contrôle négatif

Mesurés dans la VM, sur `C:\nivuus\agent\agent.exe`, c'est-à-dire le chemin
que la tâche `guacamole-agent` lance :

| Chaîne | Neuf | Ancien |
| --- | --- | --- |
| `zone de travail illisible` (**neuve**) | **2** | **0** |
| `viewport suivi` (**neuve**) | **1** | — |
| `sortie virtuelle rendue au pilote` (préexistante) | **1** | — |
| `zone de travail INEXISTANTE` (**témoin négatif**) | **0** | — |
| `redimensionnement ignor` (**retirée par le lot**) | **0** | **1** |

🔵 **L'ancien binaire, mesuré par le MÊME instrument dans la MÊME exécution,
est le contrôle négatif** : il rend l'inverse exact sur les deux chaînes qui
discriminent.

### 7.5 La relance, et la session

`Get-Process agent` **avant** (5 processus, tués), tâche arrêtée, binaire
remplacé, tâche relancée à **11:48:00**.

| | |
| --- | --- |
| Processus | **3**, tous en **`SessionId = 1`** |
| Enrôlement | `agent enrôlé auprès de la plateforme url="ws://192.168.3.1:3445/agent" prefixe=3sxuA9dd56NpVdHi37R86g` |
| `ERROR` depuis la relance | **0** |
| `redimensionnement ignor` au journal | **0** |
| Fenêtres adoptées par moi | **0** — aucune recette jouée, aucune fenêtre laissée |

🔴 **DEUX ZÉROS ONT FAILLI ÊTRE LUS COMME DES MESURES, ET AUCUN N'EN ÉTAIT UN.**
① Un premier relevé visait `C:\nivuus\agent\agent.log`, **qui n'existe pas** :
les six compteurs rendaient `0`, y compris « ERROR : 0 ». Le vrai journal est
`C:\nivuus\agent.log`. ② Puis `enrol` et `identite` rendaient `0` — parce que
le produit écrit **`enrôlé`** et **`identité`**, avec leurs accents : le piège
que `CLAUDE.md` nomme déjà. **Les deux n'ont été vus qu'en LISANT les lignes au
lieu de croire les comptes**, et le second relevé porte désormais un témoin
positif explicite.

### 7.6 Retour en arrière

Un `Copy-Item` : `agent.exe.avant-lot33` → `agent.exe`, plus un `rsync` depuis
`/var/tmp/lot33/dist-avant-lot33`. **Les deux moitiés doivent revenir
ensemble**, pour la raison du § 7.1.

---

## 8. Ce que le propriétaire doit regarder — et à quoi ressemble un ÉCHEC

🔴 **Le risque est qu'il dise « ça marche » sur un symptôme voisin.** Ce lot
touche TROIS choses ; elles se jugent séparément.

### 8.1 Ce qui doit changer

1. **Retailler la PWA doit retailler la fenêtre Windows dedans.** C'est le
   signe le plus net, et le seul qui ne demande aucun œil exercé.
2. **Les bandes noires latérales doivent disparaître tant que la PWA reste
   plus petite que la sortie** — le cas mesuré le plus fréquent (15 des 34).
3. **La barre des tâches doit sortir du bas de l'image**, et les ~48 px
   d'application qu'elle recouvrait doivent redevenir visibles et cliquables.

### 8.2 À quoi ressemblerait un ÉCHEC — à dire, pas à taire

- 🔴 **La fenêtre change de taille puis revient, une fois par seconde** : les
  deux moitiés ne s'accordent pas. C'est le mode d'échec propre à ce lot, et
  le plus important à rapporter.
- 🔴 **L'image montre du bureau, du fond d'écran ou une bordure grise** sur un
  ou deux côtés : la fenêtre a suivi, le recadrage non.
- ⚠️ **Les bandes noires demeurent quand la PWA est TRÈS large** (plein écran
  sur un grand moniteur) : **ce n'est PAS une régression, c'est la limite
  déclarée** — on ne peut pas grandir au-delà de la sortie. Le distinguer du
  premier cas se fait en rétrécissant la PWA : si les bandes disparaissent en
  petit, le remède marche et c'est la borne qu'on touche.
- ⚠️ **La barre des tâches est toujours là mais les bandes ont disparu** (ou
  l'inverse) : une seule des deux moitiés mord.
- ⚠️ **Le hub est vide au premier abord** : attendu, c'est le prix du
  redémarrage, et non un défaut du lot.

### 8.3 Ce qu'un « ça marche » ne prouverait PAS

Ni la latence, ni le curseur (legs du lot 32T, toujours dû), ni l'écart
desktop/texture de 432 px du lot 32T — qui est **inchangé** : l'image couvre
toujours la même fraction de la fenêtre qu'avant.

---

## 9. Le contrôle des secrets qui criait à tort — rétractation et fermeture

🔴 **J'AI RAPPORTÉ « un secret en clair dans `journaux-lot31/instrument/
pilote-lot31.mjs` ». C'ÉTAIT FAUX, ET JE LE RETIRE.** Le dépôt garde ses
réfutations plutôt que de les effacer.

**Le mécanisme de mon erreur** : j'ai relayé la FORMULATION de l'assertion
(« des secrets sont affectés en clair ») **sans ouvrir la ligne 19**. C'est
nommément le patron « réutiliser la sortie d'une commande pour répondre à la
question d'une AUTRE ». Le message d'échec était exact sur ce que le contrôle
avait trouvé ; la conclusion que j'en ai tirée était mienne, et fausse.

**La pièce.** La ligne 19 est un commentaire d'usage, et la « valeur » est
l'ellipse :

```
//   RECETTE_EMAIL=... RECETTE_MOTDEPASSE=... PLATEFORME_URL=http://h:p \
```

```
printf '%s' '...' | sha256sum | cut -c1-16   ->  ab5df625bc76dbd4
```

— exactement l'empreinte du message d'échec, et une exception préexistante du
contrôle la décrivait déjà, pour un autre fichier, comme « un OBJET DE
REMPLACEMENT que le lecteur doit substituer, pas une valeur ».

**La « seconde défaillance » n'existe pas** : `test:sqlite` et `test:postgres`
sont le MÊME fichier de test et la MÊME assertion, joués contre les deux dos de
base. J'avais rapporté « 2 étapes » d'une façon qui laissait croire à deux
trouvailles.

**Fermé au niveau de la VALEUR** (commit `c1bbe6f`), et non du chemin : scanner
l'intérieur des commentaires est correct et n'était pas le défaut ; le défaut
était qu'aucune règle générale ne couvrait l'objet de remplacement, exempté
fichier par fichier — donc destiné à re-crier à chaque nouvel exemple d'usage.
L'exemption par chemin devenue redondante
(`2026-07-27-jalon1-tranche-verticale.md` / `WINDOWS_ADMIN_PASSWORD`) est
**retirée**, et c'est elle qui prouve que la branche mord.

**Vue rouge en trois points**, fichier jetable et suivi, valeur factice
engendrée à l'instant : un littéral réel **dénoncé**, `abc...` **dénoncé**
(pas de sur-acceptation), `...` exempté. Fichier désindexé et détruit.

✅ `verify-all.sh` rend **« Les 10 étapes sont passées »** — il en rendait deux
en échec depuis le lot 31.


---

## 10. Le second envoi : je m'étais rendu aveugle à mon propre échec

**31 août 2026, après le retour du propriétaire** : *« La taskbar ne s'affiche
plus ! Par contre j'ai des bordures noires de chaque côté de la fenêtre du
notepad. »*

### 10.1 Ce qui est ÉTABLI

**La moitié agent MORD.** Trois nombres du journal, sur les mêmes sessions :
le superviseur demande **`largeur=1428 hauteur=1032`** à l'attache (c'était
1428×**1080** avant le lot), NVENC s'initialise à **1428×1032**, la texture
reste **1860×1080**. La barre des tâches est hors cadre — le propriétaire le
confirme, et les nombres disent pourquoi.

### 10.2 Ce qui N'A PAS PU ÊTRE ÉTABLI, ET C'EST MA FAUTE

`viewport suivi` = **0** et `recadrage et fenêtre alignés` = **0**, avec
témoins positifs dans le même relevé (32 attaches, 4 désignations, 9 NVENC).

🔴 **CE ZÉRO EST ININTERPRÉTABLE, ET LE CODE DIT POURQUOI.** Toutes les traces
du chemin neuf étaient placées **après un court-circuit** :
`placement_periodique::suivre_le_viewport` porte trois `return` avant son
`tracing::info!`, le `suivre_le_viewport` du capteur un, et
`table/attribution.rs` **zéro trace**. Un `0` confondait donc « aucun viewport
n'arrive », « la session n'a pas de sortie retenue », et « il arrive et sature
la borne » — ce dernier cas étant **la limite déclarée du remède**, pas un
défaut.

🔴 **ET LA TRACE QUE J'AI REMPLACÉE ÉTAIT PRÉCISÉMENT CELLE QUI AVAIT RENDU CE
LOT POSSIBLE** : `redimensionnement ignoré` sortait à CHAQUE demande, et c'est
d'elle que viennent les 34 mesures du § 1.1. Le remède a supprimé son propre
instrument de diagnostic. **Une trace qui ne peut sortir qu'en cas de succès ne
peut pas diagnostiquer un échec** — piège de méthode, désormais inscrit dans
`CLAUDE.md`.

### 10.3 Une sonde session 1 QUI NE PROUVE RIEN, et pourquoi je l'écarte

Jouée par tâche planifiée `/it`, elle imprime bien `== session = 1` et trouve
**un seul moniteur** (`\\.\DISPLAY1 bounds=1280x800 work=1280x752`), tous les
Notepad dessus, un à `1278x750+1+1` et quatre minimisés.

❌ **Elle est INADMISSIBLE pour la question posée**, et c'est le journal qui la
récuse : à **10:05:46Z l'agent a détruit ses sorties virtuelles**
(`sortie virtuelle détruite id=259`, `id=260`) et **aucune session ne tournait**
depuis. La sonde décrit l'après, pas le symptôme. *Trancher sur une donnée du
relevé, jamais sur la seule date* — la donnée dit « pas de session ».

⚠️ **Et j'ai écrasé ma première sonde session 1** en rejouant le script à la
main depuis WinRM, qui écrit le MÊME fichier depuis la session 0 : le relevé
lu portait `== session = 0`. Le piège des « plusieurs journaux », rejoué sur
un fichier de sonde.

### 10.4 Ce que le second envoi contient

**Trois traces INCONDITIONNELLES**, chacune posée AVANT tout court-circuit :

| Où | Ce qu'elle dit |
| --- | --- |
| `boucle.rs`, bras `DepuisLaShell::Viewport` | le message **arrive** — session, demande, `effets`, `etat` |
| `placement_periodique::suivre_le_viewport` | demande, borne, retenue, **précédente**, et un champ `decision` qui NOMME la branche |
| `windows_source::suivre_le_viewport` | demande, borne, texture, retenue, courante, `change` |

Elles rendent les quatre branches distinguables, et le volume reste borné : le
`ResizeObserver` est lissé à 200 ms et ne bat que pendant un geste.

### 10.5 La capture réseau, et ce qu'elle ne dit pas encore

Lancée **avant** le geste demandé (12:13:10), sur `host 192.168.3.2 and tcp
port 3445`. 🔵 **L'instrument est prouvé VOYANT** : les trames serveur→agent se
lisent en clair (`{"type":"battement-recu","v":5,…}`), donc un zéro y serait
une mesure.

⚠️ **Mais le `Resize` NE PASSE PAS PAR LÀ** — il voyage sur le canal de données
WebRTC, de pair à pair, pas par le signaling. Le témoin négatif « je vois des
`Resize` mais pas de `viewport` » **n'est pas disponible sur cette capture**,
contrairement à ce que le cadrage supposait ; le témoin utilisable est le
battement.

⚠️ **Et au moment d'écrire, le propriétaire n'avait pas encore joué son geste**
(aucune création de sortie, aucune attache depuis 12:13) : le zéro observé
mesure un système **au repos**, et ne conclut rien.

---

## 11. La capture qui tranche, et la réfutation de « c'est la limite déclarée »

### 11.1 Points 1 à 4, sur les valeurs

**① Les tailles demandées VARIENT, elles ne plafonnent pas.** 28 trames
`viewport` relayées vers la VM pendant ses gestes, **8 valeurs distinctes** :
`1724x1304`, `1723x1303`, `2058x851`, `1922x1092`, `1865x1303`, `1785x1303`,
`1652x1206`, `1438x1062`. Et le produit les suivait déjà : **11 tailles
d'encodage distinctes** au journal sur la même période, de `1266x480` à
`1860x1032`.

**② La borne effective de sa session est `1860x1032`** — sorties relevées
`largeur=1860 hauteur=1080` dans la topologie, moins les 48 rangées de la
barre. ⚠️ **Ni 1428 ni 1860 n'est une constante** : un attachement de la même
séance porte `largeur=1428 hauteur=1080`, c'est-à-dire une zone de travail
**égale au moniteur** — Explorer ne pose sa barre secondaire sur une sortie
neuve qu'avec un délai, si bien que la borne de CRÉATION peut être la pleine
hauteur et se resserrer ensuite. Le suivi de viewport la rattrape ; c'est un
mécanisme, pas un nombre.

**③ LE DISCRIMINANT, ET IL DÉSIGNE UN DÉFAUT.** Sous la borne sur les deux
axes, l'ancien code rendait bien le viewport **exactement** (`min` = identité) —
c'est ce qui rendait la lecture « saturation » tentante. Mais **aucune de ses
huit demandes n'était sous la borne sur les deux axes** (hauteurs 1303, 1092,
1206, 1062 contre 1032 ; largeur 2058 contre 1860). Et dès qu'**un seul** axe
dépasse, le bon comportement est de **réduire à l'échelle en préservant la
forme**, jamais d'écrêter un axe : `1723x1303` tient en `1364x1032`, sous la
borne sur les deux axes et au rapport exact. **Dépasser la borne n'oblige à
aucune bande.** C'est donc un défaut, et il est de moi.

**④ LA MARGE SUR LES QUATRE CÔTÉS RESTE INEXPLIQUÉE PAR MON ARITHMÉTIQUE, ET
JE NE LA RÉTRO-AJUSTE PAS.** `object-fit: contain` ne peut produire qu'**une
seule paire** de bandes à la fois. Deux candidats, ni l'un ni l'autre établi :
① la bordure CSS de `#remote` (`border: var(--trait) solid
var(--accent-fenetre)`), qui court sur les quatre côtés mais **ne varie pas
avec le rapport** ; ② la composition des deux — une paire de bandes qui varie
avec le rapport, plus une bordure constante. **À vérifier après déploiement,
pas avant.**

### 11.2 Ce qui resterait visible si le correctif ne mordait pas

- 🔴 **Des bandes dont l'épaisseur VARIE encore avec la forme de la fenêtre** :
  le correctif n'a pas mordu. C'est le critère central, et il est distinctif —
  après correction, l'écart de rapport est borné par l'arrondi pair, donc
  invisible.
- ⚠️ **Une marge FINE et CONSTANTE sur les quatre côtés, insensible au
  rapport** : ce n'est PAS le défaut corrigé, c'est le candidat ① ci-dessus
  (la bordure CSS). À rapporter comme tel, pas comme un échec.
- 🔴 **Du bureau ou du fond d'écran** dans l'image : la fenêtre et le recadrage
  ont divergé — autre défaut, à rapporter distinctement.
- 🔴 **La barre des tâches de retour** : le fit à aspect préservé aurait mangé
  le bornage par la zone de travail. Les deux doivent tenir ensemble.
- ⚠️ **Une image visiblement plus DOUCE qu'avant** : attendu et assumé. Servir
  la forme juste impose de réduire la taille (1723×1303 demandé → 1364×1032
  servi, remonté par le navigateur). C'est le prix du rapport exact, et c'est
  exactement ce que le § 11.3 propose d'acheter autrement.

### 11.3 Le dossier pour la décision du propriétaire : créer la sortie généreusement

🔵 **LA QUESTION A CHANGÉ DE NATURE DEPUIS LE CORRECTIF D'ASPECT, ET C'EST LE
POINT LE PLUS IMPORTANT DE CE DOSSIER.** Tant que la forme servie était fausse,
une sortie plus grande était la seule façon d'atténuer les bandes. **Elle ne
l'est plus** : le rapport est désormais exact quelle que soit la borne. Ce
qu'une sortie plus grande achète n'est donc plus l'absence de bandes — c'est
**la NETTETÉ** : servir 1723×1303 nativement au lieu de 1364×1032 remonté par
le navigateur. La décision est réelle, mais elle n'est plus urgente.

**Ce qu'elle coûte**

| | |
| --- | --- |
| Pixels à encoder | l'encodeur suit le RECADRAGE, pas la sortie : le coût ne monte que si la borne monte ET que le viewport la suit. De `1860x1032` à `1920x1080` : **+8 %** de pixels. Une sortie plus HAUTE (1440) coûterait bien davantage |
| Débit | non mesuré à taille variable ; `BUDGET_BPS` découpe un budget de session en parts, il ne s'adapte pas à la résolution |
| **Plafond d'encodeurs à N fenêtres** | 🔴 **JAMAIS MESURÉ** au-delà de 720p — protocole écrit et **non joué** au lot 31. NVENC borne en **macroblocs par seconde**, pas en nombre de sessions : huit fenêtres à `TAILLE_MAX_SORTIE` sont **2,25×** les macroblocs de huit fenêtres à 720p. C'est le risque le moins connu et le plus structurel |
| Mémoire du pilote | non mesurée ; le vivier de dix sorties est un plafond de NOMBRE, jamais de taille |
| Décodeur du navigateur | D6 a relevé **18,03 % d'images jetées** dès huit fenêtres de 1280×720 (une exécution) |

**⚠️ Ce qui fragilise l'option, et cela reste entier**

D8 a établi qu'**une sortie ne naît pas à la taille demandée** : elle naît à la
dernière taille laissée au registre. **En demander plus ne donne pas
nécessairement plus.** Et la relation entre les trois nombres —
taille demandée, `GetDesc().DesktopCoordinates` (1428 hier, 1860 aujourd'hui),
`DXGI_OUTDUPL_DESC` (1860) — **n'est toujours pas comprise**. Sur cette machine
la même journée, une sortie créée à 1428 a été relevée à 1428 puis à 1860 selon
la session. **Changer l'entrée d'une boîte noire qu'on ne comprend pas est un
pari, et il faut le dire au propriétaire dans ces termes.**

**`borner_a_la_taille_max` — correction d'une prémisse**

❌ « Elle a perdu son dernier appelant » **n'est plus vrai** : c'était le leg 5
de D9, **fermé par les tâches 6 et 7 de D10**. Elle a aujourd'hui **trois**
appelants de production — `creation_sortie::creer_sortie` (taille de création),
`table::attribution::viewport_recu` (chemin de réutilisation), et
`taille_pour_viewport` (les deux moitiés du lot 33). Vérifié par `grep`, pas de
mémoire. L'option ne la rebranche donc pas : elle **déplacerait** ce qu'elle
borne, de « le viewport du jour de l'ouverture » vers « le plafond ».

**Le cas HiDPI, entier lui aussi**

À `devicePixelRatio = 2`, une fenêtre de 1280×720 CSS demande **2560×1440**,
soit quatre fois les pixels. `borner_a_la_taille_max` le ramène à 1920×1080 —
donc la demande EST bornée aujourd'hui, contrairement à ce que le legs de D9
disait avant D10. Ce qui n'est pas borné, c'est le **coût cumulé** à N
fenêtres, faute du plafond d'encodeurs jamais mesuré.

### 11.4 Une correction de plan à consigner, pour ne pas la repayer

⚠️ **Le `Resize` NE PASSE PAS par le signaling.** Il voyage sur le canal de
données **WebRTC**, de pair à pair ; seul le `viewport` transite par le relais
(port 3445). Le témoin négatif « je vois des `Resize` mais pas de `viewport` »
n'existe donc pas sur une capture du signaling — **le témoin utilisable est le
battement** (`{"type":"battement-recu"}`), qui prouve que l'instrument lit les
trames serveur→agent en clair. Détail qui coûte une heure à qui l'ignore.

---

## 12. Le SECOND déploiement (31 août 2026, 12:30) — et la clôture

**Autorisation permanente du propriétaire** (« ne me demande pas pour
déployer »), prix du redémarrage connu et accepté.

**Envoi AGENT SEUL** : `git diff --stat 25d6d78..HEAD -- client/ proto/` est
vide, donc l'ordre client-puis-agent est sans objet ici. Il a été vérifié, non
supposé.

| | |
| --- | --- |
| Payload `console` | `021556b8…` → **`ac19c67185daea25`**, `cmp`-identique au binaire attesté |
| **À DESTINATION** (`C:\nivuus\agent\agent.exe`) | **`ac19c67185daea25`**, par `Get-FileHash` DANS la VM |
| Binaire remplacé | conservé en `agent.exe.avant-lot33b` |
| Processus | **3**, tous en **`SessionId = 1`** |
| Enrôlement | présent (témoin positif) ; **`ERROR` = 0** |
| Laissé sur sa machine | **rien** — `D:\nivuus-lot33` retiré, aucune tâche planifiée, aucune fenêtre ouverte |

**Les témoins, à destination, avec l'ANCIEN binaire en contrôle négatif dans la
même exécution :**

| Chaîne | Neuf | Ancien |
| --- | --- | --- |
| `viewport recu de la page-shell` (neuve) | **1** | **0** |
| `viewport recu par le superviseur` (neuve) | **1** | **0** |
| `Resize recu par le capteur` (neuve) | **1** | **0** |
| `sortie virtuelle rendue au pilote` (préexistante) | **1** | **1** |
| `viewport INEXISTANT` (témoin négatif) | **0** | **0** |

⚠️ **Le correctif d'aspect lui-même n'est PAS attestable par une chaîne** — il
ne change aucun littéral. Son témoin est **la trace `viewport recu par le
superviseur`**, qui porte `demande` et `retenue` côte à côte : le rapport se
lit directement au journal dès le premier geste. C'est ce que le premier envoi
n'avait pas, et c'est pourquoi il a fallu deux allers-retours.

### 12.1 Ce qu'il doit regarder

1. **Les marges qui variaient avec la forme de la fenêtre doivent disparaître.**
   C'est le critère central de ce second envoi.
2. La fenêtre doit continuer de suivre, et la barre des tâches rester hors du
   cadre — **les deux acquis du premier envoi ne doivent pas régresser**.

### 12.2 À quoi ressemblerait un échec

- 🔴 **Des marges qui VARIENT encore avec le rapport** : le correctif n'a pas
  mordu.
- ⚠️ **Une marge fine et CONSTANTE sur les quatre côtés, insensible au
  rapport** : ce n'est pas le défaut corrigé — c'est le candidat « bordure
  CSS » du § 11.1. **À rapporter comme tel, pas comme un échec.**
- 🔴 **Du bureau ou du fond d'écran dans l'image** : fenêtre et recadrage ont
  divergé.
- 🔴 **La barre des tâches de retour** : le fit aurait mangé le bornage par la
  zone de travail.
- ⚠️ **Une image plus DOUCE qu'avant** : **attendu et assumé**. Servir la forme
  juste impose de réduire la taille, que le navigateur remonte. C'est
  exactement ce que le § 11.3 propose d'acheter autrement.

### 12.3 Ce que ce lot N'ÉTABLIT toujours PAS

- 🔴 **Personne n'a regardé une image.** Aucun jugement visuel n'est porté par
  moi ; le seul œil est celui du propriétaire, et il juge après coup.
- 🔴 **Aucune recette navigateur n'a été jouée**, à aucun moment : le rôle
  `client` est exclusif et il est resté connecté.
- 🔴 **La marge sur les quatre côtés reste inexpliquée** (§ 11.1), et je la
  laisse telle plutôt que de l'ajuster après coup.
- 🔴 **L'écart desktop/texture du lot 32T est inchangé** — ni créé ni corrigé.
- ⚠️ **Quatre hypothèses ont été réfutées par la mesure au cours de ce lot**,
  dont deux miennes (« le placement périodique est en bataille permanente » ;
  « le plafond `1.0` garde la barre des tâches hors du cadre »). C'est le
  relevé qui a tranché à chaque fois, jamais le raisonnement.


---

## 13. Le TROISIÈME envoi : le cadre invisible de DWM (31 août 2026, 12:47)

*« Il y a moins de bordure, mais y en a toujours. »* puis, au discriminant :
**« La marge ne varie pas. »**

### 13.1 Le discriminant a fait son travail

🔵 **Le critère « varie / ne varie pas » avait été écrit AVANT la mesure**
(§ 11.2). Une marge insensible au rapport ne peut pas être une erreur de
proportion : la piste de l'aspect résiduel est **éliminée par le juge humain**,
et les traces neuves le confirment indépendamment — écart d'aspect **0,176 % au
pire**, soit ~3 px sur 1700, et **exactement 0,000 %** sur deux des demandes
relevées (`1592x880` et `1696x952`, servies à l'identique).

### 13.2 Les quatre nombres, session 1, session vivante

```
GetWindowRect = 1732x1032+1280+0   <- exactement la `retenue` du journal
DWM frame     = 1718x1025+1287+0
lisere : gauche=7  haut=0  droite=7  bas=7
sortie : 1860x1080+1280+0,  zone de travail 1860x1032
```

**Les DEUX fenêtres servies rendent le même lisère, sur deux sorties
différentes.** `haut = 0` parce que la barre de titre est peinte : **le lisère
n'est pas symétrique**, et le supposer décalerait l'image.

Le recadrage part de l'origine de la sortie et couvre la taille POSÉE ; la
fenêtre visible, elle, est le cadre DWM. D'où, dans l'image : **7 px de bureau
à gauche, 7 à droite, 7 en bas, 0 en haut** — constants, insensibles au
rapport.

### 13.3 Le remède, et le piège qu'il fallait éviter

`poser` gonfle la cible du lisère avant `SetWindowPos` ; `rectangle_de` rend
désormais le **cadre DWM**. 🔴 **Les deux vont ENSEMBLE** : compenser à la pose
sans compenser à la relecture ferait voir à `doit_etre_replacee` un écart
permanent de 7 px, et la fenêtre serait reposée **chaque seconde** — la même
oscillation à 1 Hz que la conception du premier envoi avait évitée. Un test pur
tient cette boucle, **avec son contre-exemple** (le rectangle brut DOIT
différer de la cible, sinon le test ne prouve rien). Le capteur compense de
même, en taille seule (`SWP_NOMOVE`).

**Repli** : DWM refuse → `Lisere::NUL` → comportement d'avant, **aux deux
bouts** ; les deux moitiés dégradent ensemble.

### 13.4 Le débordement est délibéré, et il est chiffré

La fenêtre est posée **plus grande que le recadrage** — 7 px hors de la sortie
à gauche. Relevé sur la topologie réelle :

```
DISPLAY6 [1280..3140] cible 1732x1032 -> posee [1273..3019] : 7 px a gauche sur DISPLAY1
DISPLAY7 [3140..5000] cible 1592x880  -> posee [3133..4739] : 7 px a gauche sur DISPLAY6
```

🔵 **Elle ne mord sur AUCUN recadrage voisin** : le recadrage étant plus étroit
que sa sortie (1732 sur 1860), la lisière tombe dans la marge **non capturée**
du voisin. ⚠️ **Ce ne serait plus vrai si un recadrage occupait la largeur
ENTIÈRE de sa sortie** — et la partie qui déborde est une bordure
**transparente**, qui ne peint rien.

### 13.5 Le déploiement

| | |
| --- | --- |
| **À DESTINATION** | **`b6b984e889366259`**, `Get-FileHash` DANS la VM |
| Témoin NEUF `DwmGetWindowAttribute(DWMWA_EXTENDED_FRAME_BOUNDS)` | **1** neuf / **0** ancien |
| Témoins préexistants (`viewport recu…`, `sortie virtuelle rendue…`) | **1** / **1** des deux côtés |
| Témoin négatif `lisere INEXISTANT` | **0** / **0** |
| Processus | **3**, tous `SessionId = 1`, enrôlé, **`ERROR` = 0** |
| Laissé chez lui | **rien** |

🔵 **Ce correctif-ci EST attestable par une chaîne**, contrairement à celui
d'aspect : le message de contexte de `DwmGetWindowAttribute` n'existe que dans
le binaire neuf.

### 13.6 Ce qu'il doit regarder, et à quoi ressemblerait un échec

**Ce qui doit changer** : la marge **constante** doit disparaître ; l'image doit
toucher les bords de la fenêtre du navigateur.

**Un échec ressemblerait à :**
- 🔴 **La fenêtre qui se replace en boucle, une fois par seconde** — les deux
  moitiés du correctif ne vont pas ensemble. C'est le mode d'échec propre à ce
  troisième envoi.
- 🔴 **Une marge constante toujours là, de même épaisseur** : le lisère n'est
  pas compensé (DWM a refusé, ou le repli a mordu).
- 🔴 **Une image décalée, ou du bureau sur UN seul côté** : le lisère a été
  appliqué de travers — c'est l'asymétrie (`haut = 0`) qui aurait été manquée.
- ⚠️ **Un liseré coloré d'UN pixel tout autour** : ce n'est pas un défaut, c'est
  `--accent-fenetre` peint par `#remote { border: var(--trait) … }`, **une
  fonctionnalité voulue**. Il ne sera pas retiré.
- 🔴 **Les acquis perdus** — fenêtre qui ne suit plus, barre des tâches de
  retour, marges qui recommencent à varier avec la forme.

### 13.7 Ce que ce lot N'ÉTABLIT toujours PAS

- 🔴 **Personne d'autre que le propriétaire n'a regardé une image**, et il juge
  après coup. Aucune recette navigateur n'a été jouée à aucun moment.
- 🔴 **Le lisère de 7 px est celui de CETTE machine, à CE thème et à CE DPI** —
  il est **relevé à chaque pose**, jamais écrit en dur, mais aucune autre
  configuration n'a été mesurée.
- 🔴 **L'écart desktop/texture du lot 32T reste inchangé** — ni créé ni corrigé.
- ⚠️ **Cinq hypothèses ont été réfutées par la mesure au cours de ce lot**,
  dont trois miennes.
