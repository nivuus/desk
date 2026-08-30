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

## 4. La cause, établie par lecture : le critère est évalué TROP TÔT

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

## 5. ③ Ce que je propose, et l'ARBITRAGE que je demande

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
