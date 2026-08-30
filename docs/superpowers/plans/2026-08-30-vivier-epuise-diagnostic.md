# Lot 32F — « plus aucune sortie virtuelle disponible » : le diagnostic, et l'arbitrage que je ne prends pas seul

Date : 30 août 2026. Branche `package-nivuus`.

🔴 **CE DOCUMENT NE LIVRE AUCUN REMÈDE.** Il établit la cause, réfute la piste
qui m'était donnée, et **demande un arbitrage**. Après la manche précédente —
où j'ai déployé un remède que je n'ai jamais pu voir tirer — je ne recommence
pas sans que la conception soit tranchée.

---

## 1. La rouge est reproduite

Parcours sur la production, binaire `D34213D8…` :

```
VERDICT ROUGE : ouvertes=14 refus=30 tenues=0
motifs de refus : ["plus aucune sortie virtuelle disponible"]
```

**30 refus, tous du même motif, zéro fenêtre tenue.** C'est bien le refus
dominant, et il est entièrement à nous : aucun rapport avec Apollo.

---

## 2. ① Ce que le critère fait VRAIMENT

`agent/src/superviseur/fenetres.rs::merite_une_fenetre` — **lu, pas supposé** :

```rust
if !d.visible || d.masquee_dwm || d.a_un_proprietaire { return false; }
if d.titre.is_empty() { return false; }
!d.tool_window || d.app_window
```

🔵 **Il est déjà pur, déjà testé sur l'hôte (8 tests), et il porte déjà quatre
des cinq critères Win32 usuels** — visibilité, propriétaire, fenêtre-outil avec
sa dérogation `WS_EX_APPWINDOW`, **et l'occultation DWM**, celle qu'on oublie.
Il n'a ni classe, ni taille, ni processus.

**Le cadrage qui m'était donné supposait qu'il manquait un critère. Ce n'est
pas ce que la mesure montre.**

---

## 3. ② Le relevé — et il RÉFUTE la piste de l'exclusion

Instrument : `journaux-lot32f/recensement.ps1`, en **session 1** (il imprime sa
session), calculant **le même prédicat que le produit** et relevant en plus la
classe, la taille et le processus.

| Échantillonnage | durée | échantillons | fenêtres DISTINCTES qui passent |
| --- | --- | --- | --- |
| 2 s | 60 s, au repos | 28 | **2** |
| 2 s | 55 s, **pendant** un parcours (30 refus) | 26 | **2** |
| **150 ms** | 55 s, **pendant** un parcours | **130** | **3** |

Ce qui passe, à 150 ms :

```
F|vues=130/130|proc=powershell|classe=SDL_app|2560x1440|titre=Steam : mode Big Picture
F|vues=116/130|proc=powershell|classe=Notepad|3423x971|titre=Untitled - Notepad
```

🔴 **`DesktopWindowXamlSource` N'APPARAÎT JAMAIS**, à aucune cadence — alors
que la page-shell a bel et bien reçu un **refus portant ce titre**. Une règle
d'exclusion fondée sur lui aurait exclu **une chose que le relevé ne montre
pas**, c'est-à-dire une règle qu'on n'aurait jamais pu voir rouge.

⚠️ **Et mon premier relevé à 2 s était trop grossier** : il ne peut pas voir
une fenêtre qui vit moins de 2 s. Le passage à 150 ms n'a pourtant rien
changé au verdict — ce n'est donc pas une affaire de finesse
d'échantillonnage.

---

## 4. ~~La cause, établie par lecture : le critère est évalué TROP TÔT~~ — **RÉFUTÉ PAR MA PROPRE MESURE**

> 🔴 **ANNOTATION DU 30 AOÛT 2026 (lot 32G). CE QUI SUIT EST FAUX, ET C'EST MA
> MESURE QUI L'A DÉFAIT.** J'avais déduit — par lecture seule, sans mesure —
> que le crochet évaluait le prédicat avant que DWM n'occulte la fenêtre. Une
> sonde à **15 ms** (1641 échantillons, `journaux-lot32g/decantation.ps1`),
> pendant un parcours produisant le refus, mesure pour **chaque** fenêtre le
> temps qu'elle passe le prédicat :
>
> ```
> passant_ms=-1  Open With Dummy Window Class For Interim Dialog  "Pick an app"
> passant_ms=-1  Open With Dummy Window Class For Interim Dialog  "Pick an app"
> passant_ms=-1  SDL_app        "Steam : mode Big Picture"
> passant_ms=-1  App            "Forza Horizon 6"
> passant_ms=-1  ConsoleWindowClass  "Administrator: …cmd.exe"
> passant_ms=-1  Notepad        "Untitled - Notepad"
> ```
>
> **`-1` signifie « n'a JAMAIS cessé de passer ».** Les six fenêtres sont
> persistantes ; **aucune ne se fait occulter après coup.** Il n'y a donc rien
> à décanter, et **la conception du § 5 tombe avec.** Voir § 8.
>
> ⚠️ **La leçon est celle de `placement.rs:4-7`, et je viens de la repayer :**
> une cause « établie par lecture » n'est pas établie. J'ai écrit *établie*
> là où il fallait *déduite*.

### (le raisonnement réfuté, conservé)


`agent/src/superviseur/hook.rs` : le crochet est armé sur **`EVENT_OBJECT_SHOW`**
(`:123`) et appelle `decrire` **immédiatement** (`:124`), qui lit à cet instant
`IsWindowVisible`, `GW_OWNER`, `WS_EX_*` et `DWMWA_CLOAKED`.

🔴 **Or ces attributs sont posés APRÈS l'affichage, pas avant.** Une fenêtre
technique est montrée, puis occultée par DWM, puis reçoit son propriétaire et
son style `WS_EX_TOOLWINDOW`. **À l'instant précis de `SHOW`, elle satisfait
les cinq critères.** Le produit lui alloue alors un processus, une
`PeerConnection` et une sortie virtuelle — et le vivier de dix s'épuise.

🔵 **C'est ce qui explique le paradoxe du § 3** : mon échantillonnage ne peut
jamais voir ces fenêtres dans leur état passant, parce qu'à 150 ms elles sont
déjà occultées. **Le crochet, lui, les voit toutes — parce qu'il regarde au
seul instant où elles passent.**

**Le critère n'est donc pas incomplet : il est consulté au mauvais moment.**

---

## 5. ~~Ce que je propose~~ — **CADUC** (voir § 4 et § 8)

**La règle qui décrit ce qu'est une fenêtre d'application est déjà écrite.**
Ce qu'il faut lui ajouter n'est pas un critère de plus, c'est une
**décantation** : réévaluer le même prédicat après un court délai, et n'admettre
la fenêtre que si elle passe **encore**. Une fenêtre qui ne passe plus après
décantation n'était pas une fenêtre d'application.

**Ce que cela vaut** : aucune liste noire, aucun nom d'application, rien qui
vieillisse mal. La règle reste celle du cadrage, et le remède est temporel.

🔴 **L'ARBITRAGE, ET JE NE LE PRENDS PAS SEUL.** Une décantation retarde
**l'annonce de TOUTE fenêtre**, y compris les vraies applications : l'utilisateur
clique, et son application apparaît un délai plus tard. C'est un changement de
comportement perçu, sur le chemin le plus visible du produit.

**Trois questions que je te renvoie :**

1. **Quel délai est acceptable** pour qu'une application légitime paraisse ?
   Je n'ai aucune mesure de ce qu'un humain tolère ici, et ce dépôt note
   qu'**aucun jugement d'usage n'a jamais été porté**. Je refuse d'inventer une
   constante de plus.
2. **Décanter TOUTES les fenêtres, ou seulement re-vérifier avant d'allouer
   la sortie ?** La seconde forme n'ajoute aucun délai perçu pour une
   application qui passe — elle ne coûte qu'aux fenêtres qui allaient être
   refusées de toute façon. **C'est celle que je recommande**, et elle demande
   de relire le prédicat au moment de `creer_sortie`, pas au moment du hook.
3. **Le sort des fenêtres d'Apollo** : `SDL_app` « Steam : mode Big Picture »,
   2560×1440, **présente en permanence** (130/130), est une fenêtre
   d'application parfaitement formée. Elle consommera une sortie et sera
   diffusée. **Est-ce voulu ?** C'est le legs « il capture les fenêtres
   d'Apollo », et aucune règle honnête ne l'écarte : elle *est* une
   application.

---

## 6. Ce que ce lot N'établit PAS

- **aucun remède n'est écrit, ni testé, ni déployé** — à dessein ;
- **que la décantation suffirait** : c'est une déduction de lecture, pas une
  mesure. Le contrôle qui vaudrait est de relire le prédicat après délai sur
  les fenêtres qui échouent aujourd'hui, et de compter combien cessent de
  passer ;
- **la durée de vie réelle de ces fenêtres** : mon instrument ne les voit
  jamais passer, donc il ne peut pas la mesurer. Il faudrait instrumenter le
  crochet lui-même.

---

## 7. L'état rendu

Tâche de sonde supprimée, fichiers retirés, Notepad accumulés fermés, agent
relancé (**session 1**, `D34213D8…`, 3 processus), Apollo **Running** en
`ensure_active`. VM en exécution. **Rien n'a été déployé par ce lot.**


---

## 8. LA CAUSE RÉELLE, mesurée : les sorties FUITENT en régime

Ce ne sont ni les fenêtres techniques, ni un critère incomplet, ni une
évaluation trop précoce. **Le vivier se remplit de sorties que personne ne
tient.**

| Relevé | Valeur |
| --- | --- |
| Fenêtres passant le prédicat (sonde 15 ms, 1641 échantillons) | **6** |
| Moniteurs PnP **actifs** | **9** |
| dont **SudoVDA** actifs | **8** |
| Vivier du pilote | **10** |

**Huit sorties virtuelles vivantes pour six fenêtres**, et le vivier est
pratiquement plein. C'est ce qui produit `plus aucune sortie virtuelle
disponible`, et aussi le refus
`création d'une sortie 1280x720@60 (IOCTL 0x00222000)` — le **pilote** lui-même
refuse de créer.

🔵 **Le produit le dit déjà, et personne ne l'écoutait.** La purge du démarrage
émet une `ERROR` explicite :

```
purge terminée SANS retrouver le compte attendu — topologie non conforme à ce
que la purge a retiré   retirees=5 avant=7 apres=1 attendu=2
```

et, au relevé suivant, `retirees=7 avant=9 apres=1 attendu=2`.

🔵 **Et la purge, elle, fonctionne** : forcée à la main
(`MULTIFENETRE_VDD_PURGE=1`), elle fait passer les moniteurs SudoVDA actifs de
**8 à 0**. **La fuite n'est donc pas irrécupérable — elle est simplement
jamais récupérée entre deux démarrages de l'agent.**

**La cible se déplace donc**, et il faut le dire : ce n'est plus « quelles
fenêtres méritent une sortie », c'est **« pourquoi une sortie attribuée n'est
pas rendue »**. Le chemin de restitution existe
(`creation_sortie::rendre_la_sortie`), et le compte ne tombe pas juste.

⚠️ **Ce que ma propre campagne y a mis, et je ne l'écarte pas** : j'ai tué
l'agent par `Stop-Process -Force` des dizaines de fois aujourd'hui, et
`CLAUDE.md` prévient qu'une sortie virtuelle survit à un arrêt brutal
(le `Drop` ne court pas sur un `TerminateProcess`). **Une part de ces huit
sorties est mon sillage.** Ce qui n'est PAS mon sillage : la purge du
démarrage court à chaque relance et rapporte **elle-même** qu'elle ne
réconcilie pas — c'est un constat du produit sur lui-même, pas de moi.

---

## 9. Ce que je n'ai pas fait, et pourquoi

🔴 **Aucun remède, pour la seconde fois de suite, et c'est délibéré.** J'ai
réfuté deux cadrages successifs — le mien inclus — par la mesure. Écrire un
troisième remède sur une troisième hypothèse non mesurée serait exactement ce
que la manche précédente a coûté.

**Ce qu'il faut mesurer avant d'écrire quoi que ce soit** : compter, sur une
exécution propre, les créations et les restitutions de sortie, et voir
lesquelles ne s'apparient pas. Les deux traces existent déjà
(`sortie virtuelle créée id=…` et `sortie virtuelle rendue au pilote
sortie_pilote=…`) : **le comptage est à portée, sans une ligne de code neuve.**

**Ce qui reste vrai des manches précédentes** : le critère de fenêtre est bon
(§ 2), et la piste de l'exclusion est réfutée (§ 3). Ces deux résultats
tiennent.

---

## 10. L'état rendu

Sondes supprimées (`lot32f-rec`, `lot32g-dec`, `lot32g-purge`) et leurs
fichiers retirés. **Sorties virtuelles purgées : 8 → 0.** Notepad accumulés
fermés. Agent relancé, **session 1**, `D34213D8…`, 3 processus. Apollo
**Running** en `ensure_active`. VM en exécution. **Rien n'a été déployé.**

---

## 11. Le diagnostic de la « fuite » — et **il n'y a pas de fuite en régime**

### 11.1 ① Le comptage, sans une ligne de code neuve

Journal entier (67 194 lignes), traces appariées `sortie virtuelle créée id=` /
`sortie virtuelle détruite id=` :

| | |
| --- | --- |
| créées | **458** |
| détruites (pilote) | **312** |
| **jamais détruites** | **146** |
| rendues par le superviseur (`rendre_la_sortie`) | 149 |
| 🔴 `sortie orpheline NON rendue` | **0** |
| 🔴 `sortie virtuelle NON détruite/retirée` | **0** |

🔵 **Aucune destruction n'a JAMAIS échoué.** Ce n'est donc pas une destruction
qui rate : c'est une destruction **qui n'est jamais tentée**.

### 11.2 ④ La séparation des deux comptes — et elle change tout

**Vie courante de l'agent, sans aucun arrêt brutal depuis son démarrage :**

| | |
| --- | --- |
| créées | **10** |
| détruites | **0** |
| refus de viewport | **0** |
| fenêtres fermées | **0** |
| **sessions réellement servies** | **10** (`w-5` … `w-14`, chacune sur sa propre `\\.\DISPLAYn`) |
| processus agent | **13** = superviseur + capteur + pont + **10 enfants** |
| moniteurs SudoVDA actifs | **10** |

🔴 **DIX SORTIES CRÉÉES, DIX SORTIES EN SERVICE. La fuite en régime ordinaire
est de ZÉRO.** Rien n'est détruit parce que **rien ne s'est fermé** — c'est le
comportement correct.

**Les 146 sont donc réparties sur les vies ANTÉRIEURES de l'agent**, et elles
s'expliquent par un seul mécanisme : **l'arrêt brutal**. `CLAUDE.md` le porte
déjà — le `Drop` ne court pas sur un `TerminateProcess`, et **le chemin
d'extinction propre du superviseur n'a jamais été exercé depuis D1**.

⚠️ **Et j'en suis la cause immédiate** : j'ai arrêté l'agent par
`Stop-Process -Force` des dizaines de fois aujourd'hui. **Ces 146 sont mon
sillage**, et je ne les mets au compte de personne d'autre.

### 11.3 🔴 Ce que cela fait au diagnostic : le vivier n'est pas fuité, il est PLEIN

`plus aucune sortie virtuelle disponible` n'est pas le symptôme d'une fuite.
**C'est le vivier de dix, entièrement consommé par dix fenêtres légitimement
adoptées.** Le pilote refuse la onzième — `numeros.rs:33` le dit :
« le pilote refuse la 11ᵉ sortie simultanée (mesuré à la tâche 6) », et
`PLAFOND_NUMEROS = 16` n'est qu'une marge pour les retraits en échec.

**Le vrai levier est donc COMBIEN de fenêtres le produit adopte** — c'est-à-dire
l'**axe B**, la règle d'appartenance que le propriétaire vient de trancher. Le
diagnostic et sa décision se rejoignent.

### 11.4 ② D'où vient le « compte attendu », et pourquoi il ne tombe pas juste

`purge.rs:122` : `let attendu = avant.len().saturating_sub(retirees);`

**C'est une arithmétique sur le NOMBRE DE SORTIES DXGI**, pas sur nos GUID :
« autant qu'avant, moins ce que j'ai retiré ». Son objet, écrit à `:118-121`,
est légitime : sans ce verdict, « un handle ou un IOCTL cassé produirait
silencieusement `retirees=0` et un `Ok(())` ».

🔴 **Mais l'égalité qu'il suppose est FAUSSE dès qu'un tiers touche à la
topologie.** Les deux relevés mesurés :

```
retirees=5  avant=7  apres=1  attendu=2
retirees=7  avant=9  apres=1  attendu=2
```

**`apres` est INFÉRIEUR à `attendu`, pas supérieur** : il a disparu **plus** de
sorties que nous n'en avons retirées. C'est exactement ce que produit la sortie
virtuelle **temporaire** qu'Apollo crée pour sonder ses encodeurs et détruit
aussitôt — mesurée au lot 32C.

🔵 **L'`ERROR` est donc une FAUSSE ALERTE dans ce cas**, et la purge, elle, a
fait son travail : forcée à la main, elle a fait passer les moniteurs SudoVDA
actifs de **8 à 0**. **Le contrôle est bon dans son intention et faux dans son
arithmétique** ; il ne peut pas distinguer « je n'ai pas retiré ce que je
croyais » de « quelqu'un d'autre a retiré autre chose pendant ce temps ».

### 11.5 ③ Sur quoi la purge énumère — et sa prudence est BIEN réglée

`purge.rs:93-94` : `for numero in 1..=PLAFOND_NUMEROS { let guid = guid_pour(numero); … }`

Elle **régénère nos propres GUID depuis un gabarit déterministe** et tente un
retrait sur chacun. **Elle ne peut donc retirer QUE les nôtres.**

🔵 **Contrairement à ce que je pouvais craindre, ce n'est pas une qualité mal
réglée : c'est la bonne conception.** Les sorties d'Apollo ne portent pas notre
gabarit ; elles sont hors d'atteinte **par construction**, pas par prudence
paramétrable. **Cette purge ne peut pas casser Moonlight**, et aucun réglage
n'est à trouver ici.

⚠️ **Sa seule limite réelle** : elle ne retire que ce que **notre gabarit**
couvre. Une sortie que nous aurions créée hors gabarit lui échapperait — mais
`guid_pour` est le seul chemin de création, donc le cas n'existe pas
aujourd'hui.

---

## 12. Les hypothèses de remède — **dites comme hypothèses**

Aucune n'est écrite, aucune n'est mesurée.

| # | Hypothèse | Ce qui la CONFIRMERAIT |
| --- | --- | --- |
| **H1** | Le refus vient du **nombre de fenêtres adoptées**, pas d'une fuite. La règle d'appartenance (axe B) suffit. | Filtre armé, dans les mêmes conditions : **moins de dix** fenêtres adoptées **et zéro** `plus aucune sortie virtuelle disponible`. Témoin : les applications du catalogue toujours servies. |
| **H2** | Le **chemin d'extinction propre** n'existe pas en pratique ; tout arrêt réel laisse N sorties. | Arrêter l'agent **par son chemin propre** et compter : `sortie virtuelle détruite` = nombre de sorties vivantes, puis **0** moniteur SudoVDA. Aujourd'hui invérifiable — ce chemin n'a jamais été exercé depuis D1. |
| **H3** | Le verdict de la purge est **arithmétiquement faux** en présence d'un tiers, et son `ERROR` masquera une vraie panne le jour où elle surviendra. | Faire créer/détruire une sortie par un tiers pendant la purge et retrouver l'`ERROR` sur une purge par ailleurs parfaite — **déjà observé deux fois**, jamais provoqué délibérément. |
| **H4** | Le vivier de **dix** est simplement trop petit pour l'usage visé. | Mesurer combien de fenêtres un usage réel adopte. ⚠️ **Ce n'est pas un remède logiciel** : le refus vient du **pilote**, `numeros.rs:33`. |

🔴 **H1 et H2 ne sont pas concurrentes** : H1 traite le refus vu par
l'utilisateur, H2 traite l'état laissé derrière. **Les confondre ferait
mesurer l'un et croire l'autre corrigé.**

⚠️ **H2 recouvre un legs déjà ouvert de `CLAUDE.md`** (« le chemin d'extinction
propre du superviseur n'a jamais été exercé »), et ce diagnostic lui donne
enfin un chiffre : **146 sorties non détruites** sur ce journal.

---

## 13. Ce que ce lot n'a PAS fait

- **aucun remède, aucune modification du produit** — sondes et lecture seules,
  pour la troisième manche consécutive et à dessein ;
- **l'axe B n'est pas mesuré** : la question ② du cadrage (les applications du
  catalogue sont-elles bien nos descendantes ?) a été **commencée et non
  finie** — le relevé de filiation n'a rendu que les agents, jamais un
  `notepad.exe`, faute d'avoir couru pendant que le lancement aboutissait.
  **Rien n'en est conclu.** 🔵 En revanche la lecture est acquise et elle
  compte : `apps/lancement.rs` documente que le processus lancé **n'entre dans
  aucun job object**, délibérément — un job `KILL_ON_JOB_CLOSE` tuerait les
  applications de l'utilisateur au premier redéploiement. **Le mécanisme
  suggéré est donc déjà considéré et rejeté par le produit, avec sa raison.**
- **l'axe A est sans cible mesurée** (§ 4) : les six fenêtres relevées ne
  cessent jamais de passer le prédicat.
