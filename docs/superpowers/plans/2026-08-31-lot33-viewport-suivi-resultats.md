# Lot 33 — le recadrage et la fenêtre suivent le viewport, et la barre des tâches sort du cadre

**31 août 2026.** Branche `package-nivuus`. Commits `ff8f436` (extraction) et
`25d6d78` (correction).

Symptômes rapportés par le propriétaire, qui testait en direct : **« des
bordures noires de chaque côté »** et **« ça ne resize pas la fenêtre Notepad
quand je resize la PWA »**.

🔴 **RIEN N'EST DÉPLOYÉ PAR CE LOT.** L'agent n'a pas été redémarré, aucune
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
