# Sous-projet ① « Divers » — Presse-papier bidirectionnel et couleur d'accent

> Conception. **Aucun code n'est écrit par ce document** ; il tranche, il
> justifie, et il écrit le coût de chaque décision.
>
> Deux phrases du cadrage ouvrent ce chantier, et il n'en existe pas d'autres :
> `docs/superpowers/specs/2026-07-27-refonte-produit-design.md:111-112`,
> « **Divers** : couleur d'accent de la fenêtre envoyée par data channel (plus
> de `getImageData` côté client), presse-papier bidirectionnel ». **Tout le
> reste est tranché ici.**

---

## 0. Ce que ce document corrige d'entrée dans son propre mandat

Le brief qui a commandé cette spec affirme que « le §11 [du cadrage] range
explicitement le presse-papier avancé (images, fichiers) parmi les TODO de
l'ancien produit ». **C'est faux, et il faut le dire avant de s'en servir comme
d'un appui.**

Relevé par `grep -rn "lipboard\|presse-papier" docs/superpowers/specs/2026-07-27-refonte-produit-design.md CLAUDE.md` :

- le §11 du cadrage (`2026-07-27-refonte-produit-design.md:298-305`) **ne
  mentionne pas le presse-papier**, ni de près ni de loin. Il énumère
  multi-moniteurs, enregistrement de session, collaboration, impression, la
  migration de données et le sort de l'ancien code. Rien d'autre ;
- la seule occurrence de « clipboard avancé (images, fichiers) » vit à
  **`CLAUDE.md:8879`**, entrée n°17 de la **« Priorité P3 (Basse - Nice to
  have) »** de la feuille de route de l'**ancien** produit Guacamole.

**Ce que cela change pour ce document** : le périmètre « texte seul » du §3.4
ci-dessous **n'est pas hérité d'une décision du cadrage** — il est pris ICI, et
il doit donc porter sa propre justification, ce qu'il fait. La distinction
compte : une décision héritée ne se rouvre pas, une décision prise ici se
rouvre le jour où sa justification tombe.

---

## 1. Objet, et les deux objets qu'on remplace

Le sous-projet livre deux mécanismes qui n'ont en commun que d'être nommés dans
la même ligne du cadrage, et de traverser le même canal.

### 1.1 Le presse-papier — ce que l'ancien produit faisait

L'ancien client (`web/index.js`) synchronisait dans les deux sens à travers
Guacamole :

- **serveur → navigateur** : `guac.onclipboard = handleServerClipboardChange`
  (`web/index.js:393`), qui reçoit un flux et un type MIME
  (`web/index.js:330`), le convertit en `Blob` et appelle
  `navigator.clipboard.write` avec un `ClipboardItem` — **tous types confondus**,
  images comprises ;
- **navigateur → serveur** : `onFocusHandler` (`web/index.js:349`), câblé sur
  `click` et `keypress` (`web/index.js:391-392`), qui interroge la permission
  `clipboard-read` (`web/index.js:358`), lit **tout** le presse-papier local et
  pousse chaque élément dans un `createClipboardStream`.

Deux propriétés de cet ancien code gouvernent la conception qui suit, et il faut
les nommer plutôt que de les recopier :

1. **Il lit le presse-papier local à chaque clic et à chaque frappe.** C'est un
   sondage permanent d'une ressource privée, gardé par une permission que
   l'utilisateur doit accorder ; s'il la refuse, `onFocusHandler` **lève une
   exception non interceptée** (`web/index.js:360-362`, `throw new Error('Not
   allowed to read clipboard.')`) dans un écouteur d'événement — c'est-à-dire
   que le sens navigateur → serveur meurt **en silence**, sans que rien ne le
   dise à l'utilisateur. C'est exactement le mode de défaillance que ce dépôt
   combat.
2. **Il était mono-fenêtre.** Une session Guacamole = une fenêtre navigateur =
   un presse-papier. Le produit d'aujourd'hui a **N fenêtres navigateur et N
   processus enfants pour UN SEUL presse-papier Windows**, et c'est le cœur du
   §3.2.

### 1.2 La couleur d'accent — ce que l'ancien produit faisait, et il le faisait DEUX fois

C'est un point que le cadrage ne distingue pas et qu'il faut distinguer, faute de
quoi ce chantier refera le travail de ④.

| Mécanisme ancien | Où | Nature | Qui le reprend |
| --- | --- | --- | --- |
| `ColorThief.getColor(image)` sur l'**icône** extraite du raccourci | `src/app.js:6`, `src/app.js:92-98` | **statique, par APPLICATION**, sert le manifeste PWA et la page d'accueil | **④**, sous-bloc **G5** — `2026-08-19-gestion-apps-design.md:762-763` : « le manifeste dynamique par application (icône 256, **couleur d'accent**) » |
| `getPixelColor(canvas, x, y)` — un `getImageData` d'**un seul pixel** du canevas, relu **toutes les 3 s** | `web/index.js:765-770`, boucle `web/index.js:692` | **vive, par FENÊTRE**, sert `document.body.style.backgroundColor` et `meta[name=theme-color]` sous Window Controls Overlay | **CE document** |

**C'est le second que le cadrage vise** — sa parenthèse dit « plus de
`getImageData` côté client », et `getImageData` n'apparaît que là
(`web/index.js:767`, seule occurrence du dépôt). Le premier appartient à ④ et
**ce document ne le rouvre pas.**

⚠️ **Et le nouveau client ne peut pas refaire l'ancien geste, même s'il le
voulait** : il n'a aucun canevas. Il affiche la vidéo dans un
`<video id="remote">` (`client/src/main.ts:16`) alimenté par une piste WebRTC
(`client/src/webrtc.ts:283-286`). Échantillonner un pixel exigerait de dessiner
la vidéo dans un `<canvas>` à chaque relevé — c'est-à-dire d'ajouter une passe
de rendu par relevé pour lire trois octets. **La parenthèse du cadrage n'est
donc pas une préférence de style : le geste ancien n'a plus de support.**

---

## 2. Ce que le code impose avant toute décision

Tout ce qui suit a été relu ligne à ligne dans l'arbre courant.

### 2.1 Le canal de contrôle, et son versionnement

| Fait | Pièce |
| --- | --- |
| `CONTROL_VERSION = 3`, des deux côtés | `proto/src/control.rs:14`, `proto/ts/control.ts:8` |
| Le champ `v` est vérifié par **égalité stricte** côté Rust | `proto/src/control.rs:41-52`, refus en `:46` |
| Idem côté TypeScript, avant toute autre chose | `proto/ts/control.ts:132` |
| `AgentControl` porte 8 variantes | `proto/src/control.rs:111-211` |
| `ClientControl` en porte **2** — `Resize` et `Visibility` | `proto/src/control.rs:87-106` |
| Les deux enums portent `deny_unknown_fields` | `proto/src/control.rs:86`, `:110` |
| Le parseur TypeScript valide `type` contre une **liste écrite à la main**, puis **caste** | `proto/ts/control.ts:106-108`, `:135`, `:138` |
| Le canal de contrôle est **fiable et ordonné** | `client/src/webrtc.ts:269` — `createDataChannel('control', { ordered: true })` |
| Le canal d'entrées **ne l'est pas** | `client/src/webrtc.ts:265-268` — `ordered: false, maxRetransmits: 0` |
| La file de contrôle sortante est bornée à 32 | `agent/src/transport/controle.rs:41` |

### 2.2 Deux points de passage obligés, et un seul des deux est gardé par le compilateur

**Gardé** — ajouter une variante à `AgentControl` **casse la compilation** tant
que son bras n'est pas écrit, parce que `agent/src/transport/controle.rs:98-107`
fait un `match` **exhaustif** sur `&message` pour nommer le type au journal. De
même, ajouter une variante à `ClientControl` casse
`agent/src/transport/evenements.rs:251-258`, qui n'a pas de bras catch-all.

**NON gardé, et c'est le piège que ce dépôt a payé QUATRE fois** —
`agent/src/capteur/pont_media.rs:75-78` :

```rust
Ok(autre) => {
    tracing::warn!(?autre, "trame inattendue sur la connexion média, abandonnée");
    return;
}
```

Ce `return` **tue le fil `lire_le_media`**, donc affame `SourceDistante`, donc
fait tomber la session dans sa fenêtre de reprise — **sans aucune panne
apparente**. Les quatre fois sont inscrites dans le fichier lui-même :
`Sommeil` (D5, `pont_media.rs:38-49`), `Part` (D6, `:53-60`), `Audio` (D7,
`:62-64`), `PleinEcran` (D8, `:68-71`).

> 🔴 **Toute variante de `DepuisCapteur` poussée sur la connexion média par ce
> chantier DOIT recevoir son bras en `pont_media.rs`, et son test à côté des
> quatre existants** (`pont_media.rs:156`, `:184`, `:215`). Ce serait la
> cinquième fois. C'est une clause de réception, pas un conseil.

**NON gardé non plus, et personne ne l'a encore payé — côté TypeScript.**
`TYPES_AGENT` (`proto/ts/control.ts:106-108`) est un tableau écrit à la main,
et **rien ne le confronte à l'union `AgentControl`** (`:101-104`) : vérifié par
`grep -rn "TYPES_AGENT" proto client/src agent/src signaling`, qui ne rend que
sa déclaration et son unique usage en `:135`. Oublier d'y inscrire un type neuf
ne casse **ni la compilation TypeScript ni aucun test** : `parseAgentControl`
lève, `client/src/webrtc.ts:272-276` intercepte, et le message est perdu contre
un `console.warn`. **C'est le jumeau exact de `pont_media.rs`, sur l'autre
rive.** Il entre lui aussi dans la liste des points de passage ci-dessous.

### 2.3 Le patron « un état Windows lu périodiquement devient un message »

Il existe, il est éprouvé, et ce chantier le recopie plutôt que d'en inventer
un. Le modèle est le plein écran de D8, `agent/src/capteur/fenetre.rs:375-388` :

```rust
if plein_ecran::actif() && dernier_style.elapsed() >= plein_ecran::PERIODE_STYLE {
    dernier_style = Instant::now();
    if let Some(style) = plein_ecran::lire_style(self.parametres.hwnd) {
        if let Some(actif) = suivi_bordure.observer(style) {
            let message = DepuisCapteur::PleinEcran { actif };
            if let Fin::Terminer(motif) = deposer(AEcrire::Etat(message), …) { break motif; }
```

Cinq ingrédients, tous réutilisés ici :

1. **une garde d'armement** par variable d'environnement, convention `=0`
   désarme, une simple présence n'arme pas ;
2. **une période propre au mécanisme**, jamais couplée à celle d'un voisin —
   `PERIODE_STYLE` (250 ms) et `PERIODE_REARBITRAGE` (250 ms,
   `agent/src/capteur/sommeil.rs:52`) sont égales et **délibérément
   indépendantes** ;
3. **une lecture Win32 bon marché, jamais à l'image** ;
4. **un détecteur de CHANGEMENT** dont l'état de référence est pris à
   l'attache : `None` signifie « rien à annoncer » ;
5. **`deposer(…)` et jamais un `send` bloquant**, seul point où le fil peut
   attendre en servant les commandes pendant l'attente.

### 2.4 Où vit le focus, et où vit l'injection clavier

| Chose | Où elle vit | Pièce |
| --- | --- | --- |
| La session focalisée, **au plus une dans tout le capteur** | `Etat.focalisee: Option<String>`, sous `OnceLock<Mutex<Etat>>` | `agent/src/capteur/sommeil/registre.rs:38` |
| Le rang du dernier focus, par session | `Etat.derniers_focus: HashMap<String, u64>` | `agent/src/capteur/sommeil/registre.rs:50` |
| L'arbitrage audio **pur** de D7 | `arbitrer(&[FenetreAudio]) -> Vec<(String, bool)>` | `agent/src/capteur/audio.rs:54`, départage en `:85` |
| L'injection clavier/souris | dans l'**ENFANT**, pas le capteur | `agent/src/input.rs:241` (`SendInput`), construit en `agent/src/demarrage.rs:313-316` |
| `SetForegroundWindow` avant chaque touche | un seul site d'appel | `agent/src/input.rs:159`, appelé depuis `agent/src/input.rs:119`, dans le bras `InputMessage::Key` (`:118`) |

⚠️ **La propriété que `agent/src/input.rs:140-147` écrit de `SendInput` — « global
à la session Windows : il n'adresse personne, il alimente la file d'entrée de la
fenêtre active » — vaut MOT POUR MOT du presse-papier Win32**, qui est unique par
*window station*. Le presse-papier est donc le second objet global de ce produit,
et le premier a déjà coûté un chantier (D1 ne savait pas router le clavier, D2 y
a mis `SetForegroundWindow`).

### 2.5 Le design system, tel qu'il est livré au stade S1

| Fait | Pièce |
| --- | --- |
| Un token `--accent` existe, avec trois valeurs (un bloc sombre, deux blocs clairs) | `client/src/design/tokens.css:58`, `:177`, `:202` |
| Il a **un** appelant réel : l'anneau de focus | `client/src/design/base.css:71` |
| `--sur-accent` est **orphelin et en liste d'attente pour S2** | `client/outils/tokens-orphelins.mjs:98` |
| Le contraste est imposé et vérifié : `--accent` est une « encre » mesurée à 4,5:1 contre les trois fonds | `client/src/design/contraste.ts:67`, seuils en `:73-74` |
| `rapportDeContraste` est **exporté et pur** | `client/src/design/contraste.ts:55` |
| Le contrôle « aucune couleur littérale » balaye `client/src/**/*.css` **et `*.ts`** | `client/outils/couleurs-litterales.mjs:208-211` |
| …mais ne cherche que des **notations écrites en clair** | `client/outils/couleurs-litterales.mjs:111` |
| …et **le déclare lui-même** : « un `el.style.background = 'red'` passerait donc » | `client/outils/couleurs-litterales.mjs:60-63` |
| La règle d'accès à une valeur de token est déjà écrite : `getComputedStyle(document.documentElement).getPropertyValue('--…')`, **à un changement de thème, jamais par image** | `docs/superpowers/specs/2026-08-19-design-system-design.md:214-224` |
| `theme-color`, le manifeste et les icônes **appartiennent à ②**, pas à ⑥ | `docs/superpowers/specs/2026-08-19-design-system-design.md:856` |
| La mise en page Window Controls Overlay est livrée par **⑥/S4**, et **non exerçable** tant que ② n'a pas posé de manifeste | `docs/superpowers/specs/2026-08-19-design-system-design.md:539-547`, `:657-662` |

---

## 3. Relevés pris pour ce document, et ce qu'ils tranchent

**Toute la conception du presse-papier repose sur ces mesures.** Elles ont été
prises le 19 août 2026 dans un **Chromium sans interface** piloté par Playwright,
sur `https://example.com/` (contexte sécurisé, `isSecureContext: true`),
`HeadlessChrome/151.0.0.0`. La VM Windows n'a **pas** été employée — un chantier
concurrent y conduisait une recette.

⚠️ **Portée, à ne pas élargir** : un Chromium **sans interface**, une exécution
par point, **aucun autre navigateur**, et un contexte où
`navigator.userActivation.isActive` valait déjà `true`. Ce qui est mesuré est le
**comportement de l'API**, pas le comportement d'un utilisateur réel devant une
fenêtre de dialogue de permission.

### 3.1 R1 — Écrire ne demande aucune permission ; lire, si

Sortie brute de la sonde :

```
perm:clipboard-read   : "prompt"
perm:clipboard-write  : "granted"
writeText             : OK
readText              : THROW: NotAllowedError: Failed to execute 'readText'
                        on 'Clipboard': Read permission denied.
readTextApresEcriture : THROW: NotAllowedError: … Read permission denied.
navigator.userActivation : { isActive: true, hasBeenActive: true }
```

Quatre faits, et chacun décide quelque chose :

1. **`clipboard-write` est `granted` d'office et `writeText` réussit.** Le sens
   **VM → navigateur** ne dépend donc d'aucune permission. C'est ce qui permet au
   sous-bloc **P1** d'être minimal et démontrable sans rien demander à
   l'utilisateur.
2. **`readText()` échoue alors que `userActivation.isActive` vaut `true`.** Ce
   n'est donc **pas** le geste qui manque : c'est la **permission**. Le brief de
   ce document supposait les deux liés ; la mesure les sépare.
3. **La page ne peut pas relire ce qu'elle vient d'écrire.** Toute conception
   qui voudrait confirmer une écriture en relisant est **impossible** — elle
   n'est pas coûteuse, elle n'existe pas.
4. **L'état de la permission a glissé de `prompt` à `denied` au cours de la
   session**, après le refus (relevé lors d'une sonde ultérieure). Un
   `permissions.query` n'est donc **pas** un prédicteur stable : un premier refus
   colle. **Conséquence de conception : ne jamais conditionner un chemin à
   `permissions.query`, seulement à l'issue réelle de l'appel.**

### 3.2 R2 — L'événement `paste` de confiance lit le presse-papier SANS permission

C'est le relevé qui gouverne tout le sens navigateur → VM, et il a été pris avec
sa **rouge** — la valeur opposée a été produite dans la même session, sur le
même montage, en ne changeant **qu'une variable**.

Montage : un écouteur `keydown` sur `window` qui appelle `preventDefault()` sous
condition d'un drapeau, un écouteur `paste` sur `window`, et un `Ctrl+V`
**réel** (`page.keyboard.press`, donc `isTrusted: true`).

| Drapeau | `keydown` reçus | `paste` reçus |
| --- | --- | --- |
| `preventDefault` **armé** | `ControlLeft` puis `KeyV`, `isTrusted: true` | **aucun** — `pastes: []` |
| `preventDefault` **désarmé** | idem | **un**, `isTrusted: true`, `types: ["text/plain"]`, texte `"sonde-guacamole"` |

Et au moment du second relevé, `permissions.query({name:'clipboard-read'})`
rendait **`denied`** et `readText()` levait toujours.

> 🔵 **Donc : l'événement `paste` de confiance délivre le presse-papier système
> à une page dont la permission de lecture est explicitement REFUSÉE.** C'est le
> fait le plus utile de ce document : il donne au sens navigateur → VM un chemin
> **sans permission du tout**.

> 🔴 **Et c'est le client actuel qui le bloque.** `client/src/input.ts:94-95`
> appelle `event.preventDefault()` **sans condition** sur chaque `keydown`, sur
> `window` (`client/src/input.ts:111`). En l'état, **aucun événement `paste` ne
> peut être émis dans la fenêtre de session.**

Deux faits annexes du même relevé, qui ferment les échappatoires :

- `document.execCommand('paste')` rend **`false`** — le chemin hérité est mort ;
- un `ClipboardEvent('paste')` **synthétique** porte `isTrusted: false` et un
  `clipboardData` **vide** (`""`). On ne peut donc pas se fabriquer un collage.

Et une observation qui ne décide rien mais borne le §3.4 : `ClipboardItem.supports`
rend `true` pour `text/plain`, `image/png` **et** `text/html`. **Le périmètre
texte seul est donc une décision de produit, pas une limite d'API.**

### 3.3 Ce que ces relevés N'établissent PAS

- **Rien du comportement avec une interface réelle** : le montage est sans
  interface. Une fenêtre de dialogue de permission n'a jamais été affichée, ni
  acceptée, ni refusée par un humain.
- **Rien de Firefox ni de Safari.** `clipboard-read` n'est même pas un
  descripteur de permission valide partout, et l'événement `paste` y a ses
  propres règles. **Non mesuré.**
- **Rien de `writeText` depuis une fenêtre SANS le focus du document.** Chromium
  est documenté pour le refuser (`NotAllowedError: Document is not focused`), et
  **la sonde a toujours tourné sur une page focalisée** : cette contrainte est
  donc **supposée, pas mesurée**. Elle est traitée comme réelle par la
  conception (§4.2, règle du dépôt différé) parce que s'en protéger est gratuit
  et que s'en passer serait un pari.

  > ⚠️ **MESURÉE PAR LE SOUS-BLOC P3 (21 août 2026), sonde S2, DEUX exécutions
  > aux relevés identiques — et le verdict est plus fin que « vrai » ou
  > « faux ».** Pièces : `journaux-presse-papier-p3/p3-writetext-{1,2}.json`.
  >
  > | cellule | état relevé | `writeText` |
  > | --- | --- | --- |
  > | focus ✔, activation ✘ | `{hasFocus:true, isActive:false}` | `NotAllowedError: … Write permission denied.` |
  > | focus ✔, activation ✔ | `{hasFocus:true, isActive:true}` | **OK** |
  > | focus ✘, activation ✘ | `{hasFocus:false, isActive:false}` | `NotAllowedError: … Document is not focused.` |
  > | focus ✘, activation ✔ | — | 🔴 **INATTEIGNABLE** |
  >
  > 🔴 **LA CELLULE QUI TRANCHE EST INATTEIGNABLE À CE MONTAGE** : le geste de
  > confiance REND le focus à la fenêtre qui le reçoit, et `Page.bringToFront`
  > ne le lui reprend plus (deux moyens essayés, attente sur le FAIT — vingt
  > relectures de `hasFocus` — et jamais sur une durée). **Le §3.3 reste donc
  > SUPPOSÉ au sens strict**, et le dire est un verdict recevable ; en fabriquer
  > un autre ne le serait pas (RP3-5).
  >
  > ✅ **MAIS IL EST CORROBORÉ PAR UNE PIÈCE, et c'est mieux qu'un « non
  > tranché »** : les deux refus portent le **MÊME NOM** et des **MESSAGES
  > DIFFÉRENTS**. Un chemin de refus **propre au focus** existe donc, et il se
  > nomme lui-même. Ce qui reste non mesuré est s'il survit à une activation.
  >
  > 🔴 **POURQUOI UN 2×2 ET PAS UNE SIMPLE OBSERVATION.** P2 avait DÉJÀ mesuré
  > un `NotAllowedError` sur `writeText`, et ce n'était PAS le focus : son
  > annexe versée relève `hasFocus: true` DES DEUX CÔTÉS, et ce qu'elle mesurait
  > était l'ACTIVATION. **Les deux mécanismes lèvent la même exception**, et une
  > sonde qui n'aurait observé que « pas de focus ⟹ THROW » aurait attribué au
  > focus ce qui pouvait être l'activation — dans les deux sens possibles.
  >
  > ⚠️ **UNE PREMIÈRE RÉDACTION DE CETTE SONDE A RENDU LE VERDICT INVERSE — « le
  > §3.3 est RÉFUTÉ » — ET SON PROPRE RELEVÉ LE RÉFUTAIT** : elle jouait le
  > geste EN DERNIER, obtenait `{hasFocus:true, isActive:true}`, c'est-à-dire la
  > cellule précédente sous une autre étiquette. C'est l'erreur d'attribution
  > que la sonde existait pour empêcher, commise par la sonde. Deux remèdes :
  > l'ordre geste → retrait du focus → écriture, et le verdict calculé sur
  > l'ÉTAT OBSERVÉ, une cellule dont l'état ne correspond pas à son étiquette
  > étant requalifiée INATTEIGNABLE.
- ✅ **CETTE LIGNE EST PÉRIMÉE : `paste` sur un `<video>` focalisé A ÉTÉ
  MESURÉ, le 20 août 2026, et le verdict est FAVORABLE aux DEUX exécutions**
  (`docs/superpowers/plans/journaux-presse-papier-p2/p2-paste-video-{1,2}.json`).
  Focus sur `<video id="remote" tabindex="0">`, régime « exception étroite »,
  `Ctrl+V` de confiance : **1 `paste`, `isTrusted: true`,
  `types: ["text/plain"]`, `e.target` = `VIDEO#remote`**, et
  `clipboard-read` = `denied`/`prompt` avec `readText()` levant
  `NotAllowedError` aux deux. Le relevé de juillet ci-dessous reste vrai **comme
  histoire** ; il n'est plus la question ouverte qu'il annonce.
- **Rien de `paste` sur un `<video>` focalisé.** La sonde a tourné avec le focus
  sur le `body` d'une page ordinaire. Le `<video>` est le seul élément qui prenne
  le focus dans la fenêtre de session (`client/src/main.ts:230`,
  `video.focus()`), et **il n'est pas éditable**. C'est le premier contrôle du
  sous-bloc **P2** (§6.2), et il peut échouer.
- **Rien de la VM Windows.** Aucun relevé Win32 n'a été pris ; tout ce que ce
  document dit du presse-papier Windows est **de la documentation d'API, pas une
  mesure**, et le §8 le range comme tel.

---

## 4. Décisions — le presse-papier

### D1 — Le CAPTEUR détient le presse-papier Windows. Un seul propriétaire, et c'est un argument de correction, pas de performance

Le presse-papier Win32 est **unique par window station** — comme la file
d'entrée que `agent/src/input.rs:140-147` décrit. Le produit a N processus
enfants. Trois arrangements étaient possibles :

| Arrangement | Ce qui casse |
| --- | --- |
| **Chaque enfant** parle au presse-papier | N observateurs d'une ressource **globale**, et **N gardes anti-écho qui ne se voient pas** : l'écriture de l'enfant A est une notification pour l'enfant B, qui la renvoie à sa page, qui la réécrit, qui notifie A… **L'oscillation est inter-processus, donc irréparable localement.** |
| Le **superviseur** le détient | il ne parle pas aux enfants : `agent/src/capteur/protocole.rs:34-37` écrit qu'« il n'existe **aucun canal direct superviseur→capteur** » et que tout transite par l'enfant. Il faudrait inventer un canal. |
| **Le capteur** le détient | rien. Il est unique, il a déjà un canal bidirectionnel avec chaque enfant, il tient déjà le registre du focus (`registre.rs:38`), et il a déjà le patron de sondage périodique (§2.3). |

**Décision : le capteur, et lui seul, ouvre le presse-papier Windows.**

C'est le même argument que D4 pour la mutualisation de la capture — **un
observateur unique d'une ressource unique** —, et c'est un argument de
**correction** : un garde anti-écho n'est sain que s'il voit toutes les
écritures, ce qu'un garde par processus ne fait pas.

⚠️ **Le chemin mono-fenêtre existe et n'a pas de capteur.** `agent/src/main.rs`
retourne vers `capteur::executer()` sous `CAPTEUR`, vers `superviseur::executer()`
sous `SUPERVISEUR`, et vers `demarrage::executer(config)` sinon — l'enfant, ou
l'agent mono-fenêtre. **Règle : le propriétaire est le capteur quand il existe,
l'enfant sinon.** Les deux ne coexistent jamais, donc il n'y a jamais deux
gardes. C'est la même forme que `windows_audio`, qui vit dans l'enfant.

### D2 — Le mécanisme de notification Windows est nommé, et il est ÉCARTÉ au profit d'un compteur sondé

Le mécanisme de notification que le brief demandait de nommer est
**`AddClipboardFormatListener(hwnd)`**, qui fait poster **`WM_CLIPBOARDUPDATE`**
à une fenêtre à chaque changement de presse-papier. (Le mécanisme historique,
la chaîne `SetClipboardViewer` / `WM_DRAWCLIPBOARD`, est déconseillé depuis
Vista : un maillon qui meurt casse la chaîne.)

**Il est écarté, et voici pourquoi.** `AddClipboardFormatListener` exige **une
`HWND` et une pompe de messages** qui la serve. Le capteur n'en a pas : ses fils
de fenêtre bouclent sur une lecture d'images et un service de commandes
(`agent/src/capteur/fenetre.rs:286-390`), sans `GetMessage`. L'adopter voudrait
dire créer une fenêtre message-only et un fil de pompe **dans le processus qui
tient N duplications DXGI et N encodeurs** — c'est-à-dire ajouter une boucle de
messages Win32 au processus le plus chargé et le plus fragile du produit.

**Décision : le capteur sonde `GetClipboardSequenceNumber()`**, un compteur
`DWORD` de la window station qui s'incrémente à chaque modification, **sans
ouvrir le presse-papier ni posséder de fenêtre**. Il est lu sur le patron du
§2.3, à `PERIODE_PRESSE_PAPIER`, et le presse-papier n'est **ouvert que lorsque
le compteur a bougé**.

**Ce que ce choix coûte, écrit :**

- **une latence bornée par `PERIODE_PRESSE_PAPIER`** entre la copie côté Windows
  et son arrivée au navigateur. Valeur retenue **250 ms**, par cohérence avec
  `PERIODE_STYLE` et `PERIODE_REARBITRAGE` — et, comme elles,
  **`PERIODE_PRESSE_PAPIER` est une constante propre à ce mécanisme, à ne coupler
  à aucune autre**, exactement ce que `plein_ecran.rs` interdit pour la sienne.
  **Elle n'est pas calibrée**, et rejoint la liste que ce dépôt tient
  (`BPP_MIN`, `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`, `HYSTERESIS`,
  `TAILLE_MAX_SORTIE`…) ;
- **une coalescence** : deux copies séparées de moins d'une période ne
  produisent qu'un message, celui du contenu final. C'est correct — le
  presse-papier est un **état**, pas un flux d'événements, et c'est la même
  doctrine que `DepuisCapteur::Etat` (`agent/src/capteur/protocole.rs:124-127`) ;
- **un faux positif possible** : une application qui réécrit le **même** contenu
  incrémente quand même le compteur. Le garde d'égalité de contenu (D5) l'absorbe.

**Où le sondage vit.** Sur le **fil de fenêtre** du §2.3, comme le plein écran ?
Non — cela ferait N sondages d'une ressource unique. **Le sondage vit sur le tour
de roue du registre** (`agent/src/capteur/sommeil/registre.rs`, fil unique, période
`PERIODE_REARBITRAGE`), qui est déjà l'endroit où le capteur regarde le monde
**une fois** pour toutes les fenêtres. Le message part ensuite vers chaque
fenêtre par le canal `mpsc` déjà existant — exactement le trajet de
`Message::Audio` décrit par le patron d'arbitrage.

### D3 — Qui écrit, qui lit, et ce qui se passe quand deux fenêtres veulent écrire

C'est la question centrale, et elle **n'a pas la même réponse dans les deux
sens**. Une seule règle pour les deux serait une symétrie fausse.

#### Sens **VM → navigateur** : aucune élection. Toutes les fenêtres reçoivent, seule la focalisée écrit localement.

Raison : l'utilisateur qui copie dans Excel puis passe à sa machine locale peut
coller **depuis n'importe laquelle** de ses fenêtres de session — il n'y a aucune
façon de deviner laquelle. Élire une porteuse rendrait le collage local **muet
dans toutes les autres**, sans rien pour le dire.

Le capteur pousse donc l'état à **toutes** les fenêtres. Puis, côté client, une
règle simple ferme le problème du focus document (§3.3, contrainte supposée —
✅ **mesurée et CORROBORÉE par le sous-bloc P3, sans être établie** : voir
l'encadré du §3.3) :

> **La page mémorise toujours le dernier contenu reçu ; elle ne l'écrit dans le
> presse-papier local que si elle a le focus. Sinon elle l'écrit à la prochaine
> reprise de focus.** Une écriture obsolète est ainsi impossible : c'est
> toujours le **dernier reçu** qui est écrit, jamais une file.

**Coût nommé** : une fenêtre qui n'a jamais eu le focus n'a jamais écrit le
presse-papier local — c'est correct (l'utilisateur n'y a jamais collé), et c'est
observable au journal.

#### Sens **navigateur → VM** : pas d'élection non plus, et c'est un fait, pas un choix.

Un collage naît d'un `Ctrl+V`, qui va **à la fenêtre navigateur qui a le focus**.
Le système d'exploitation de l'utilisateur a donc **déjà élu** l'émetteur, et il
n'en existe qu'un à la fois. Il n'y a rien à arbitrer.

**Et quand deux fenêtres veulent écrire quand même ?** (fenêtres sur deux écrans,
deux collages à quelques millisecondes d'intervalle, ou un client qui se conduit
mal.) **Le dernier arrivé gagne, et c'est la sémantique du presse-papier Windows
lui-même** — `SetClipboardData` n'a jamais fait autre chose. Le capteur les
sérialise par construction : les commandes arrivent sur le canal de commandes,
une par une, et sont exécutées sur un fil.

> **Donc : ce qui exigeait un propriétaire unique n'était pas l'écriture — c'est
> l'ÉCOUTE et le garde anti-écho.** L'écriture n'a jamais eu besoin d'être
> arbitrée. C'est la conclusion à retenir de D1 et D3 pris ensemble, et elle
> diffère de l'audio (D7), où l'arbitrage est réel parce que **deux fenêtres
> peuvent physiquement émettre en même temps**.

⚠️ **Ce que cette décision renonce à faire, explicitement** : il n'existe **aucun
presse-papier par fenêtre**. Deux fenêtres de session partagent le presse-papier
Windows, parce que **leurs applications Windows le partagent déjà**. Un
utilisateur qui copie dans la fenêtre A et colle dans la fenêtre B obtient ce
qu'il attend ; c'est un effet du modèle, pas un défaut.

### D4 — Texte seul en v1 (`text/plain`, UTF-8), borné à 64 KiB, et un refus VISIBLE au-delà

`ClipboardItem.supports` dit `true` pour `image/png` et `text/html` (§3.2) :
**la limite n'est pas l'API.** Elle est le canal.

1. **Le canal de contrôle est fiable, ordonné, et PARTAGÉ.** `ordered: true`
   (`client/src/webrtc.ts:269`) sur un flux SCTP unique qui porte aussi
   `Pointer`, `Resize`, `Link`, `Asleep`. Une image de plusieurs mégaoctets y
   **bloquerait en tête de file** le curseur et le redimensionnement — sur le
   canal même dont `agent/src/transport/controle.rs:32-36` explique que ses
   producteurs « émettent des ÉTATS, dont seul le dernier compte ».
2. **La file sortante est bornée à 32 messages**
   (`agent/src/transport/controle.rs:41`), sans borne d'octets. Une charge non
   bornée y ferait une empreinte mémoire non bornée.
3. **Le presse-papier avancé est déjà rangé ailleurs** : `CLAUDE.md:8879`, P3
   n°17 de l'ancien produit — et **pas** au §11 du cadrage (§0).

**Décision : `text/plain` en UTF-8, et rien d'autre.** Borne
`PRESSE_PAPIER_MAX = 64 KiB` d'UTF-8, **non calibrée** et déclarée telle. Elle
tient très largement un document texte ou un fichier source ; à 10 Mb/s elle
retarde un message de curseur d'au plus ~52 ms, et seulement au moment d'une
copie.

> 🔴 **Au-delà de la borne, on REFUSE — on ne tronque pas.** Tronquer un
> presse-papier produit un collage **silencieusement faux** : l'utilisateur ne
> voit pas que la moitié manque, et le colle dans un document. C'est le pire
> résultat possible, et il est pire que de ne rien coller. Le refus est **dit** :
> un bandeau de statut persistant côté client, sur le patron de `micro.ts`
> (`client/src/main.ts:283`, `{ persistant: true }`), et un `warn!` côté agent.

Ce que cela laisse hors périmètre est au §9 : images, fichiers, RTF, HTML.
`TAILLE_MAX` du canal capteur↔enfant vaut 8 MiB
(`agent/src/capteur/protocole.rs:23`) : la borne de 64 KiB est très en deçà, ce
n'est donc pas elle qui contraint.

### D5 — La boucle est fermée par DEUX gardes, dont l'un est exact

La boucle redoutée : l'agent écrit le presse-papier → Windows incrémente le
compteur → l'agent croit à une copie neuve → il la renvoie au navigateur → le
navigateur la réécrit → … Elle est réelle et elle ne s'arrête pas d'elle-même.

**Garde n°1, exact et principal — le numéro de séquence.** Juste après sa propre
écriture, le propriétaire relit `GetClipboardSequenceNumber()` et le mémorise. Au
sondage suivant, si le compteur observé **est égal** à celui mémorisé, le
changement est le nôtre : rien n'est émis. Le garde est exact parce que le
compteur est **le même objet** que celui qui déclenche.

**Garde n°2, redondant et secondaire — l'égalité de contenu.** Le propriétaire
mémorise aussi le dernier texte **émis** et le dernier texte **reçu** ; un
contenu identique n'est jamais réémis. Il couvre le cas où le n°1 échouerait
(une écriture tierce intercalée entre notre `SetClipboardData` et notre relecture
du compteur), et il absorbe le faux positif de D2 (même contenu réécrit).

C'est **exactement** le garde que l'ancien produit avait déjà, et il faut le
créditer : `web/index.js:349` mémorise `currentCopiedItems` et compare avant de
renvoyer. La conception ici ne l'invente pas, elle lui ajoute le n°1 — que
l'ancien produit n'avait pas, parce qu'il n'écrivait jamais le presse-papier
Windows lui-même.

**Garde n°3, côté client.** La page ne réémet jamais vers l'agent un contenu
qu'elle vient de recevoir de lui. Sans lui, un `paste` de l'utilisateur juste
après une réception renverrait le texte à son émetteur — sans boucle infinie
(l'agent l'absorberait par le n°2), mais avec un aller-retour inutile.

> ⚠️ **Les deux premiers gardes vivent dans le PROPRIÉTAIRE (D1), donc dans un
> processus unique. C'est ce qui les rend sains, et c'est la raison d'être de
> D1.**

### D6 — Le collage passe par le canal de contrôle, PAS par le canal d'entrées, et l'agent injecte lui-même

C'est la décision la moins évidente du document, et elle vient d'une contrainte
d'ordre.

Un collage est **deux choses qui doivent arriver dans cet ordre** : (a) le
presse-papier Windows contient le bon texte, puis (b) l'application reçoit
`Ctrl+V`. Le chemin naïf — le client envoie le texte sur le canal de contrôle et
laisse la touche `V` partir normalement sur le canal d'entrées — **ne garantit
pas cet ordre** : ce sont deux flux SCTP distincts, dont l'un est
`ordered: false, maxRetransmits: 0` (`client/src/webrtc.ts:265-268`). Le mode de
défaillance est **« l'application colle le contenu PRÉCÉDENT »** : silencieux,
plausible, et faux.

**Décision.** Sur un raccourci de collage (`Ctrl+V`, `Shift+Insert`), le client :

1. **n'appelle pas `preventDefault()`** pour ce `keydown`, afin que le
   navigateur émette l'événement `paste` (R2) ;
2. **retient** les scancodes de cette touche : ils ne partent **pas** sur le
   canal d'entrées ;
3. sur l'événement `paste`, lit `e.clipboardData.getData('text/plain')` et émet
   **un seul** message sur le canal de contrôle : `ClientControl::Clipboard
   { text, paste: true }`.

Et l'agent, à la réception, exécute **dans l'ordre, sur un seul chemin** :
l'enfant commande au capteur d'écrire le presse-papier, **attend son `Fait`**,
puis injecte `Ctrl+V` par son `InputInjector` (`agent/src/input.rs`), qui pose
déjà `SetForegroundWindow` avant toute touche (`agent/src/input.rs:119`).

**Ce que cela coûte, écrit :**

- **un aller-retour de tube** entre l'enfant et le capteur avant l'injection. Le
  patron existe déjà et il est exercé : `set_awake` commande le capteur depuis la
  boucle de transport, et D4 a mesuré une attache en 34 µs. Pour un collage —
  geste rare, non nerveux — c'est sans objet ;
- **le clavier de la fenêtre est modifié**, et c'est le seul endroit de ce
  chantier qui touche `client/src/input.ts`. La condition est étroite
  (`event.ctrlKey && event.code === 'KeyV'`, plus `Shift+Insert`) et **testable
  purement** (§5) ;
- **`Ctrl+V` est injecté, pas transmis.** Une application qui lit l'entrée brute
  (un jeu) verra une frappe synthétique. C'est ce que l'utilisateur a demandé, et
  `SendInput` était déjà la seule voie ;
- ⚠️ **si le presse-papier ne peut pas être écrit, la touche `V` est PERDUE, pas
  reportée.** L'agent journalise et le client affiche un bandeau. Rendre la
  touche après coup produirait un collage du contenu périmé — le mode de
  défaillance silencieux que toute cette décision existe pour éviter.

**Le sens copie (`Ctrl+C`) ne demande RIEN au client.** La frappe part
normalement, Windows copie, le compteur de séquence bouge, D2 fait le reste. Il
n'y a pas d'événement `copy` à écouter : le navigateur n'a rien copié.

### D7 — `CONTROL_VERSION` ne monte PAS, et la preuve tient en quatre lectures

**Décision : `CONTROL_VERSION` reste à 3.**

*Preuve, côté vieux client + agent neuf.* `parseAgentControl` vérifie `v`
(`proto/ts/control.ts:132`) **avant** le type ; `v` vaut toujours 3, donc on
passe. Puis le type inconnu échoue contre `TYPES_AGENT`
(`proto/ts/control.ts:135`) et **lève**. Cette exception est interceptée **par
message** en `client/src/webrtc.ts:272-276`, qui écrit un `console.warn` et
**rend la main** : les messages suivants sont traités normalement. **La session
survit.**

*Preuve, côté client neuf + agent vieux.* `serde_json::from_str::<ClientControl>`
échoue sur une variante inconnue ; `agent/src/transport/evenements.rs:236`
journalise `"message de contrôle invalide"` et **continue**. La session survit
aussi.

*Preuve que monter serait PIRE.* Les deux vérifications de `v` sont des
**égalités strictes** (`proto/src/control.rs:46`, `proto/ts/control.ts:132`).
Passer à 4 ferait rejeter **tous** les messages entre un client et un agent de
versions différentes — y compris `Ready`, `SessionEnd` et `Pointer`. Une
incompatibilité **totale** remplacerait une dégradation **par message**.

*Précédent déjà écrit dans le code.* `proto/src/control.rs:118-138` : le champ
`mic` a été ajouté sans bump, et le raisonnement y est déjà consigné.

**Le coût, nommé** : un vieux client écrit une ligne de `console.warn` par
message de presse-papier reçu. Il est borné parce que ces messages ne partent
**qu'au changement** — même doctrine que `Sommeil`, `Part`, `Audio`,
`PleinEcran`.

**Et une annonce de capacité l'accompagne**, calquée sur `mic` : le champ
`clipboard: bool` s'ajoute à `Capabilities` (`proto/src/control.rs:182-186`),
avec `#[serde(default)]` — **obligatoire, pas décoratif**, `AgentControl`
portant `deny_unknown_fields` : un champ **manquant** est une erreur de
désérialisation en Rust (`proto/src/control.rs:126-129`). Côté TypeScript, il
est **optionnel** (`clipboard?: boolean`), pour que « son absence vaut `false` »
s'obtienne gratuitement (`proto/ts/control.ts:37-46`).

⚠️ **`Capabilities` arrive AVANT `Ready`**, et le code le dit
(`proto/src/control.rs:172-181`) : un client qui gaterait son initialisation sur
`Ready` perdrait ce message. Le client de ce chantier ne le fera pas — il traite
les types indépendamment, comme `client/src/main.ts:215`.

### D8 — L'armement, et ce qu'on fait d'un refus de permission

**Variable d'environnement `PRESSE_PAPIER=0` désarme**, une simple présence
n'arme pas — convention d'`AUDIO`, `PLEIN_ECRAN`, `SUPERVISEUR`, `CAPTEUR`, et
pour la même raison : tester `is_ok()` armerait le mécanisme en écrivant
`PRESSE_PAPIER=0` pour le couper. Lue par `OnceLock` dans le **propriétaire**
(D1). Trace au démarrage, une fois.

⚠️ **Elle doit être transmise par `scripts/run-agent.sh`, dans la tâche qui
l'introduit.** Ce piège a été payé en D1 (`SUPERVISEUR`), en D2
(`MULTIFENETRE_REPRISE`) et en D7 (`AUDIO`) : un agent démarre sans la variable
et **ne le signale pas**.

**Le refus de permission.** La conception n'en dépend pour rien (R1 : l'écriture
est `granted` ; R2 : le collage passe par `paste`, sans permission). Il reste
deux issues d'échec possibles, et **aucune n'est silencieuse** :

| Issue | Ce que le produit fait |
| --- | --- |
| `writeText()` rejette (`NotAllowedError`, document non focalisé, ou permission retirée) | le contenu **reste mémorisé** ; nouvelle tentative à la prochaine reprise de focus ; bandeau persistant après **deux** échecs consécutifs, pour ne pas crier sur un simple défaut de focus |
| aucun événement `paste` ne parvient (§3.3, contrôle P2-①) | bandeau persistant disant **comment** coller — et le sous-bloc P2 ne peut alors pas être reçu |

Le message dit **comment rétablir**, jamais seulement que quelque chose manque —
règle déjà appliquée au micro (`client/src/main.ts:280-283`).

**Ce qu'on ne fait PAS : appeler `readText()`.** Ni au focus, ni au clic, ni
jamais. C'est le geste de l'ancien produit (`web/index.js:349-364`), il exige la
permission (R1), il lit une ressource privée **en dehors de toute intention de
collage**, et R2 montre qu'il est inutile. **Le nouveau produit ne demande
aucune permission de presse-papier.** C'est le meilleur résultat de ce
chantier, et il est gratuit.

---

## 5. Décisions — la couleur d'accent

### D9 — La source est l'ICÔNE de la fenêtre, lue par `hwnd`. Pas le thème Windows, pas l'image capturée.

Trois sources existaient. Ce qu'elles disent n'est pas la même chose :

| Source | Ce qu'elle rend | Pourquoi elle est retenue ou écartée |
| --- | --- | --- |
| Le thème d'accent Windows (`DwmGetColorizationColor`, `UISettings`) | **une** couleur pour toute la session | ❌ **Écartée** : identique pour Excel et pour Firefox. Le cadrage dit « couleur d'accent **de la fenêtre** » — celle-ci n'est pas de la fenêtre. |
| Un échantillon de l'**image capturée** | ce que l'ancien produit faisait, suit le thème du document | ❌ **Écartée en v1** — voir le coût ci-dessous. |
| L'**icône** de la fenêtre, `WM_GETICON` / `GetClassLongPtrW(GCLP_HICON)` | une couleur par fenêtre, stable | ✅ **Retenue.** |

**Pourquoi l'icône.** Elle est **par `HWND`**, donc réellement par fenêtre :
Steam et le jeu qu'il lance — le cas fondateur du modèle multi-fenêtres — ont
des icônes différentes. Elle est **stable** : aucune oscillation, aucun
scintillement, aucune relecture par image. Et surtout, **le calcul de la couleur
dominante à partir de pixels RGBA est PUR** : aucun `#[cfg]`, testable sur
l'hôte Linux, exactement ce que la doctrine de ce dépôt récompense.

**Pourquoi PAS l'image capturée, chiffré.** Y accéder demande un aller-retour
GPU→CPU sur une texture D3D11, donc de toucher `agent/src/capture.rs` (**492
lignes, marge 8**) et/ou `agent/src/windows_source.rs` (**630 lignes, dette
gelée, `#[cfg(windows)]`, aucun test**) — relevé par la commande le 19 août
2026 (§7). Ce sont les deux fichiers que ce dépôt protège le plus. Payer cela
pour trois octets toutes les cinq secondes n'est pas un arbitrage défendable.

> ⚠️ **Ce que l'icône coûte, et c'est une régression réelle sur un axe** :
> **elle ne suit pas le thème du document.** Un Firefox en thème sombre garde
> l'icône orange de Firefox, là où le pixel échantillonné de l'ancien produit
> aurait viré au sombre. C'est le seul point où ce chantier fait **moins bien**
> que ce qu'il remplace, et c'est assumé. La voie de rattrapage est nommée —
> l'échantillonnage de l'image — et son prix est écrit ci-dessus.

> ⚠️ **Danger Win32 à traiter, pas à découvrir** : `WM_GETICON` est un
> `SendMessage` **synchrone vers une autre application**. Une application figée
> ne répond pas, et l'appel **bloque indéfiniment le fil qui l'a émis** — ici, le
> tour de roue du capteur, donc **toutes** les fenêtres. **`SendMessageTimeout`
> avec `SMTO_ABORTIFHUNG` est obligatoire**, et un délai dépassé se traite comme
> « pas d'icône », jamais comme une erreur fatale. `GetClassLongPtrW(GCLP_HICON)`
> est en revanche une lecture non bloquante, et sert de repli.

**Réutilisation avec ④, nommée sans être supposée.** ④/G5 calcule lui aussi une
couleur depuis une icône
(`2026-08-19-gestion-apps-design.md:762-763`). Le module pur de ce chantier —
« tranche RGBA → couleur dominante » — est le point de réunion naturel. **Ce
document le propose, il ne l'impose pas** : ④ a son propre calendrier, et
supposer qu'il l'adoptera serait affirmer au-delà du relevé.

**Quand elle change.** Relue sur le même tour de roue que le presse-papier, à
`PERIODE_ACCENT` (**5 s**, **non calibrée**), et **émise au changement
seulement** — doctrine de `Sommeil`/`Part`/`Audio`/`PleinEcran`. Une icône change
rarement (l'icône de progression d'un téléchargement est le cas réel).

### D10 — La couleur reçue est VALIDÉE par le client, puis posée sur un token — jamais employée brute

C'est la résolution de la tension avec le design system, et elle a été
construite **contre une mesure**, pas contre une intuition.

**La tension, exactement.** Le contrôle §7.2 du design system
(`client/outils/couleurs-litterales.mjs`) interdit les couleurs littérales, et
il balaye bien les `.ts` (`:208-211`). **Mais il est syntaxique.** Le fichier le
déclare lui-même en `:60-63` : « un `el.style.background = 'red'` passerait
donc ». Une couleur venue de l'agent arrive dans une **variable**, à l'exécution :
`style.setProperty('--x', couleurDeLAgent)` **passe vert**. Et le contrôle §7.1
(contraste) parse `tokens.css` — il ne voit **aucune** couleur calculée à
l'exécution.

> 🔴 **Donc : aucun des sept contrôles du design system ne peut voir une couleur
> venue de l'agent. Deux d'entre eux passeraient au vert sur un accent
> illisible.** Le rempart doit être ailleurs, et il doit être là où on peut le
> voir rouge.

**Décision, en quatre points.**

1. **Un token neuf, `--accent-fenetre`, déclaré dans les TROIS blocs de
   thème**, avec pour valeur par défaut celle de `--accent` du bloc
   (`tokens.css:58`, `:177`, `:202`). Les trois déclarations sont
   **obligatoires** : le contrôle §7.4 (`client/outils/blocs-de-theme.mjs`)
   exige l'égalité des ensembles de noms entre les trois blocs. Cela donne au
   token une valeur saine **avant** tout message, **si** l'agent n'en envoie
   jamais, et **si** la validation refuse celle qu'il envoie.
2. **La valeur reçue passe par une fonction PURE de conformation** avant d'être
   posée, qui **réemploie `rapportDeContraste`** — déjà exporté et pur
   (`client/src/design/contraste.ts:55`) — pour mesurer la couleur reçue contre
   les trois fonds du thème **courant**, lus par
   `getComputedStyle(document.documentElement).getPropertyValue('--fond-…')`.
   C'est le point de lecture **déjà normé** par le design system
   (`2026-08-19-design-system-design.md:214-224`), et son contrat — « à un
   changement de thème, jamais par image » — est respecté : on lit à un
   changement de thème ou à l'arrivée d'une couleur, soit au plus une fois toutes
   les 5 s.
3. **Si le seuil de 3:1 (WCAG 1.4.11, `contraste.ts:74`) n'est pas tenu, la
   couleur est REFUSÉE** et `--accent-fenetre` garde la valeur du thème. **Pas de
   correction automatique** — éclaircir ou assombrir la couleur d'une application
   produirait une teinte que personne n'a choisie, et le §8 du design system
   écrit déjà que le **choix** des teintes n'est vérifié par rien.
4. **`--sur-accent` n'est PAS employé par ce chantier.** Il est en liste
   d'attente pour S2 (`client/outils/tokens-orphelins.mjs:98`), et lui donner un
   appelant ferait **échouer** le contrôle §7.6 tant que sa ligne n'est pas
   retirée. Ce chantier ne pose pas de bouton primaire : il n'en a pas besoin.

**Ce que le client en fait à chaud.** Il pose `--accent-fenetre` sur `:root`.
C'est tout, et il faut être franc sur la conséquence :

> ⚠️ **L'effet VISIBLE de ce mécanisme est nul jusqu'à ce que ② livre un
> manifeste.** Le chrome de fenêtre PWA (Window Controls Overlay) est livré par
> ⑥/S4 et **ne peut pas être exercé** sans `display_override:
> ["window-controls-overlay"]` (`2026-08-19-design-system-design.md:539-547`,
> `:657-662`) ; `theme-color` et le manifeste **appartiennent à ②**
> (`:856`). **Ce chantier livre la valeur et sa garantie, pas son emploi
> visible** — exactement la même limitation déclarée, pour exactement la même
> raison, que la partie WCO de S4. Le §6.4 en tire un critère qui **reste
> falsifiable malgré cela**.

⚠️ **Un token neuf sans appelant est un orphelin**, et le contrôle §7.6
(`client/outils/tokens-orphelins.mjs`) exige alors une ligne en liste d'attente,
à **retirer** le jour où l'appelant naît. Le sous-bloc A1 doit **soit** livrer
son appelant, **soit** poser la ligne d'attente et nommer le sous-bloc qui la
retirera. Les deux sont acceptables ; **ne rien faire fait échouer le contrôle**.

---

## 6. Découpage en sous-blocs

Quatre sous-blocs. Le premier est minimal, démontrable, et **ne demande aucune
permission ni aucune modification du clavier**.

### P1 — La VM copie, le navigateur colle

**Livre** : le propriétaire (D1), le sondage du numéro de séquence (D2), la
lecture du texte, le trajet complet capteur → enfant → client, et
`navigator.clipboard.writeText`. **Une seule fenêtre.** Aucun garde anti-écho
n'est nécessaire : à ce stade **l'agent n'écrit jamais** le presse-papier, la
boucle ne peut pas exister.

> ⚠️ **PRÉMISSE JUSTE, CONCLUSION TROMPEUSE — annoté le 20 août 2026, revue
> transverse de fin de P1.** P1 n'écrit effectivement jamais le presse-papier,
> et la boucle d'écho ne peut effectivement pas exister. **Mais P1 LIVRE
> POURTANT le garde n°2 de D5** (`agent/src/presse_papier.rs`, comparaison au
> dernier contenu émis), et pour une raison que cette phrase ne pouvait pas
> anticiper : la sonde P0 a **MESURÉ** que `GetClipboardSequenceNumber` **bouge
> sur une réécriture identique** (`q2="bouge"`, deux exécutions). Le garde
> n'absorbe donc pas un écho — il absorbe un **faux positif du compteur**. Un
> lecteur qui repartirait de cette ligne conclurait que P1 n'embarque aucun
> garde, et le retirerait.

Chemin : `GetClipboardSequenceNumber` → `DepuisCapteur::PressePapier { texte }`
poussé sur la connexion média → **bras dans `pont_media.rs`** → `Recu::PressePapier`
→ champ consommé sur `SourceDistante` → branche `a1…` de `tick.rs` →
`AgentControl::Clipboard { text }` → **entrée dans `TYPES_AGENT`** → branche dans
`client/src/main.ts` → `writeText`.

| # | Critère | Ce qui le rend ROUGE, et pourquoi cet état est atteignable |
| --- | --- | --- |
| ① | Copier du texte dans le Bloc-notes de la VM le rend collable dans une application **locale** | ne pas écrire le bras de `pont_media.rs` : le premier message tue le fil et la session tombe dans sa fenêtre de reprise. **Cet état est atteignable en retirant le bras — c'est la ROUGE à jouer avant le vert**, et c'est le cinquième rappel d'un défaut payé quatre fois |
| ② | Un texte **inchangé** recopié ne produit **aucun** message | 🔴 rouge = désarmer le garde n°2 de D5 : le compteur de séquence bouge quand même (fait déclaré non mesuré, §8) et un message part. **Si aucun message ne part même sans le garde, le critère est NON MESURABLE** et doit le dire |
| ③ | Un texte au-dessus de `PRESSE_PAPIER_MAX` est **refusé et dit**, jamais tronqué | copier 100 KiB ; le bandeau doit paraître et le presse-papier local rester **inchangé**. Rouge = un presse-papier local qui contient 64 KiB du texte |
| ④ | `PRESSE_PAPIER=0` désarme, et **la trace le dit** | ⚠️ et le contrôle qui vaut est de vérifier que la variable **atteint le processus** — pas seulement qu'elle est écrite dans le script |

⚠️ **Un contrôle qui ne peut pas échouer serait ici trivial à écrire** : « la
trace `presse-papier` apparaît ». Elle apparaîtrait aussi sur un mécanisme qui ne
lit rien. **Le critère ① se juge sur le CONTENU collé localement**, pas sur une
ligne de journal — c'est la leçon de F1 (D7), payée trois fois.

> ⚠️ **CE QUE LA RECETTE DE P1 A RÉELLEMENT PU JUGER — annoté le 20 août 2026.**
> Deux lignes de ce tableau ont vieilli, et une exigence n'a pas pu être tenue :
>
> - **① « collable dans une application locale » n'a PAS pu être jugé.** Le
>   témoin de mesurabilité (E11 du plan), joué avant tout critère, établit que
>   **le niveau 2 est NON MESURABLE sur ce montage** : aucun serveur X n'est
>   joignable, `xclip` et `wl-paste` sont absents, `xsel` refuse en « Can't open
>   display ». Ce que la recette a jugé est le **niveau 1** — le texte
>   réellement passé à `writeText`, la résolution de sa promesse, et la
>   relecture par la page elle-même. **Ce n'est pas un échec du produit, c'est
>   une mesure non prise**, et ce n'est PAS le niveau 2 : un presse-papier
>   interne au processus Chromium suffirait à le satisfaire.
> - **② « fait déclaré non mesuré, §8 » : IL EST MESURÉ.** La sonde P0 rend
>   `q2="bouge"` sur **deux exécutions** — le compteur bouge bien sur une
>   réécriture identique. **Le critère est donc MESURABLE**, et il a été tenu :
>   1 message après la copie neuve, **1** après la recopie identique, 2 après
>   une copie différente.
> - **③ et ④ ont été tenus tels qu'écrits**, deux exécutions chacun.
>
> Détail et pièces : `docs/superpowers/plans/2026-08-19-presse-papier-p1-resultats.md`.

### P2 — Le navigateur colle dans la VM

**Livre** : R2 mis en production. L'exception étroite dans `client/src/input.ts`,
l'écouteur `paste`, `ClientControl::Clipboard`, l'écriture du presse-papier
Windows par le propriétaire, l'injection de `Ctrl+V` par l'enfant (D6), **et les
trois gardes anti-écho de D5 dès leur naissance** — parce que c'est P2 qui rend
la boucle possible, et qu'un sous-bloc ne livre pas un défaut qu'il crée.

| # | Critère | Ce qui le rend ROUGE |
| --- | --- | --- |
| ① | **L'événement `paste` parvient bien quand le focus est sur le `<video>`** | ✅ **MESURÉ LE 20 AOÛT 2026, VERDICT FAVORABLE, DEUX EXÉCUTIONS** — la clause « il n'est PAS acquis » est périmée. La rouge est jouée et versée : le régime « produit » de la sonde, qui recopie le `preventDefault()` inconditionnel de `client/src/input.ts`, rend **ZÉRO `paste` sur ses huit cellules**, aux deux exécutions. **Ne pas la rejouer sur la VM : la citer.** ~~c'est le préalable, et il n'est PAS acquis : R2 a été mesuré avec le focus sur un `body`~~ |
| ② | Coller dans le Bloc-notes de la VM depuis le presse-papier local **fonctionne**, permission `clipboard-read` **refusée** | rouge = accorder la permission et voir si cela change quelque chose : **cela ne doit rien changer.** Si cela change quelque chose, le chemin employé n'est pas celui qu'on croit |
| ③ | Le contenu collé est **le dernier copié**, jamais le précédent | rouge du chemin naïf : envoyer la touche `V` sur le canal d'entrées **au lieu de** l'injection de D6, et coller deux textes différents à la suite. **Cette rouge est le seul contrôle de l'ordre**, et elle est la justification entière de D6 — si elle ne se déclenche pas, D6 est du coût pour rien et doit être rouverte |
| ④ | ❌ **REFORMULÉ — la rouge ci-dessous est VACUEUSE, et cette spec avait prévu le cas qui se produit.** Énoncé retenu : après **k** collages, **aucun** message `clipboard` ne revient vers la fenêtre | 🔴 **ROUGE REMPLACÉE : `PRESSE_PAPIER_GARDE=0` ⟹ EXACTEMENT k messages.** ~~désarmer le garde n°1 et compter les messages sur 30 s ; le compte doit croître sans borne~~ — **il reste à UN**, et la démonstration tient au code : le `Sondeur` relit notre texte, l'annonce une fois, puis pose lui-même `dernier_emis` ET `reference`, si bien qu'`observer` sort dès sa première ligne au tour suivant ; et **rien ne relance**, le client n'émettant vers l'agent que sur un `paste`, donc sur un geste humain. **Désarmer le seul n°1 rendrait donc ZÉRO message aussi.** D'où une variable qui désarme les DEUX gardes. 🔵 **Conséquence qui contredit D5** : dans l'architecture livrée, **aucune oscillation auto-entretenue n'est possible** — ce que les gardes suppriment est un aller-retour PAR COLLAGE, pas une divergence |
| ⑤ | Un raccourci **qui n'est pas un collage** garde son `preventDefault` | rouge = frapper `Ctrl+W`, `Ctrl+T`, `Ctrl+N` : la fenêtre navigateur ne doit ni se fermer, ni ouvrir d'onglet. **C'est le risque le plus concret de P2** — élargir la condition de D6 rendrait le navigateur au clavier |

### P3 — Les N fenêtres

**Livre** : la règle « toutes reçoivent, seule la focalisée écrit localement, les
autres écrivent à la reprise du focus » (D3), et sa mesure.

> ❌ **« LIVRE LA RÈGLE D3 » EST FAUX, ET LE PLAN DE P3 L'A RELEVÉ AVANT
> D'ÉCRIRE UNE LIGNE (E1, 21 août 2026).** Elle était DÉJÀ livrée : le fan-out
> par P1 (`agent/src/capteur/sommeil/presse_papier.rs::distribuer` itère sur
> TOUTES les clés de `canaux`, sans aucun filtre, avec son test) et le dépôt
> différé par P1 aussi (`client/src/presse-papier.ts`,
> `presse-papier-dom.ts`, quatre tests d'hôte).
>
> **Ce que P3 livre est la MESURE, plus DEUX TROUS que cette spec ne nomme
> pas** : les deux moitiés du legs n°3 de P1 (l'agent n'émettait pas l'état
> courant à l'inscription, ET `client/src/main.ts` perdait en silence un
> message arrivé avant l'attache), et l'attribution session ↔ fenêtre Windows,
> qui n'était observable NULLE PART — aucune trace du dépôt n'associait une
> `session` au `hwnd` ni au PID de l'APPLICATION Windows. Sans elle, ② et ③ ne
> sont pas ATTRIBUABLES, et un relevé non attribuable n'est pas un verdict.
>
> ⚠️ **Ce document est un relevé DATÉ : il est annoté, jamais réécrit.**

| # | Critère | Ce qui le rend ROUGE |
| --- | --- | --- |
| ① | À **trois** fenêtres, une copie dans la VM parvient aux **trois** | ❌ **CETTE ROUGE DÉSIGNE UN MÉCANISME QUI N'EXISTE PAS** (E2 du plan de P3) : il n'y a **aucune élection de porteuse** dans le presse-papier — c'est le mécanisme de l'AUDIO (`capteur/sommeil/porteurs.rs`), et `presse_papier::distribuer` n'en a jamais eu. La rouge n'est pas jouable telle qu'écrite. **REMPLACÉE** : muter `distribuer` pour n'envoyer qu'à la **PREMIÈRE clé**, le compte tombant de 3 à **1**. ⚠️ Une mutation qui n'enverrait à PERSONNE serait MOINS BONNE : elle rendrait **0**, et zéro est aussi ce que rend un mécanisme entièrement mort — **1 ne peut venir que d'une distribution qui fonctionne et qu'on a restreinte** |
| ② | Un collage depuis la fenêtre **B** met le texte de B dans la VM, pas celui de A | rouge = deux textes distincts dans deux fenêtres |
| ③ | Deux collages **quasi simultanés** ne produisent ni interblocage ni contenu mêlé ; **le dernier gagne** | rouge = un contenu **mixte**, ou une commande sans réponse dans les 12 s de la borne du canal |
| ④ | Une fenêtre **sans focus** n'écrit pas le presse-papier local, et l'écrit **à la reprise du focus** | 🔴 **NON MESURABLE en conditions de produit, et c'est MESURÉ** — voir l'encadré ci-dessous |

> 🔴 **④ N'EST PAS MESURABLE EN CONDITIONS DE PRODUIT, ET LA SONDE S1 DE P3 LE
> DIT — DEUX EXÉCUTIONS, RELEVÉS IDENTIQUES** (`p3-focus-{1,2}.json`). Trois
> fenêtres ouvertes par `window.open` — **le geste du produit**,
> `client/src/shell-page.ts` — rapportent TOUTES `document.hasFocus() === true`,
> aux trois basculements. RP3-2 est réalisé, et le témoin a échoué sur sa
> deuxième issue, écrite d'avance.
>
> ⚠️ **MAIS LA CAUSE N'EST PAS LE `--headless`, et une première rédaction de la
> sonde l'aurait attribué à lui.** Une SECONDE ARME, dans la même exécution et
> avec le même code de mesure, ouvre par `Target.createTarget` : `bringToFront`
> y retire parfaitement le focus. **L'attribution est au MODE D'OUVERTURE**, et
> le verdict qui commande la recette est celui de l'arme du produit. Xvfb et
> xdotool sont relevés ABSENTS de l'hôte ce jour-là (consentement donné en D8,
> jamais suivi d'effet), et les mesures qui en sortiraient **ne se compareraient
> à aucune campagne antérieure**.
>
> 🔵 **ET « DU COÛT POUR RIEN » SUPPOSE UNE FENÊTRE — c'est E4 du plan de P3, et
> c'est sa contribution de conception.** À N, le test de focus fait autre chose
> que se protéger d'un refus : il **ÉLIT l'unique écrivain local**. Sans lui, N
> appels concurrents à `writeText` partiraient pour une seule copie, le dernier
> gagnant arbitrairement — **régime que rien ne mesure**. La règle RESTE donc,
> quel que soit le verdict du §3.3, et son retrait serait une **décision du
> propriétaire du dépôt**, jamais une conséquence mécanique d'une sonde.
>
> ⚠️ **Corollaire du relevé de S1, et il n'est pas confortable** : si toutes les
> fenêtres du produit rapportent le focus, alors **toutes écrivent**, et la
> recette rencontrera ce régime PAR ACCIDENT. Le pilote de P3 le relève
> explicitement (`n_ecrivains_concurrents`) plutôt que de le taire.

⚠️ **Trois fenêtres, pas huit.** Le blocage par pollution du registre a plafonné
D9 à trois fenêtres, et D10 l'a levé sur mesure. **Ce chantier ne re-mesure pas
ce plafond** : il prend le nombre que la VM rend le jour de la recette et
**l'écrit**. Un critère qui exigerait huit fenêtres serait un critère sur le
multi-fenêtres, pas sur le presse-papier.

### A1 — La couleur d'accent

**Livre** : la lecture de l'icône par `hwnd` (D9), le calcul **pur** de la
couleur dominante, `AgentControl::Accent`, la conformation **pure** côté client
(D10), et le token `--accent-fenetre` dans les trois blocs.

| # | Critère | Ce qui le rend ROUGE |
| --- | --- | --- |
| ① | Deux fenêtres d'**applications différentes** annoncent des couleurs **différentes** | rouge = lire le thème Windows au lieu de l'icône : les deux valeurs deviennent identiques. ⚠️ **Le critère est NON MESURABLE si les deux icônes ont la même dominante** : choisir deux applications visiblement contrastées, et **le dire dans le relevé** |
| ② | La couleur atterrit bien sur `:root` | `getComputedStyle(document.documentElement).getPropertyValue('--accent-fenetre')` doit rendre la valeur annoncée. Rouge = poser le token sans le déclarer dans les trois blocs : §7.4 échoue |
| ③ | Une couleur **illisible** est **refusée**, et le thème reprend la main | 🔴 **test unitaire pur, et il DOIT être vu rouge** : injecter un gris à 1,2:1 contre `--fond-0` et vérifier que la fonction rend le repli. C'est **le seul rempart** — §7.1 et §7.2 ne voient rien de ce chemin (D10), empiriquement confirmé |
| ④ | Aucun message tant que l'icône ne change pas | rouge = émettre à chaque période : 12 messages par minute et par fenêtre |
| ⑤ | Les **sept** contrôles du design system restent verts | `npm run design:verifier` **et** `npm test`. ⚠️ Le §7.6 (orphelins) échouera si `--accent-fenetre` n'a ni appelant ni ligne d'attente : **c'est un résultat correct du contrôle**, pas un faux positif |

⚠️ **A1 est livré sans son emploi visible** (D10). Les critères ci-dessus portent
sur la **valeur** et sa **garantie**, jamais sur un rendu — parce qu'aucun rendu
n'existe avant que ② pose un manifeste. **C'est déclaré, pas dissimulé**, et
c'est le même statut que la partie WCO de S4.

---

## 7. Ce qui est PUR, ce qui est `#[cfg(windows)]`, et le plafond de 500 lignes

### 7.1 La séparation, décidée ici

| Module | Nature | Pourquoi |
| --- | --- | --- |
| `agent/src/presse_papier.rs` | **PUR** — bornage UTF-8, normalisation des fins de ligne (`\r\n` ⇄ `\n`), garde d'égalité de contenu, décision « émettre ou non » | c'est la **logique décisionnelle**, celle que la doctrine du capteur réserve au code hors `cfg` (`agent/src/capteur/pont_media.rs:4-10`), et elle se voit rouge sur l'hôte |
| `agent/src/presse_papier/win32.rs` | `#[cfg(windows)]` | `GetClipboardSequenceNumber`, `OpenClipboard`/`GetClipboardData`/`SetClipboardData`/`CloseClipboard`. **Aucune décision**, seulement des appels |
| `agent/src/accent.rs` | **PUR** — tranche RGBA → couleur dominante | testable sur l'hôte, et c'est le point de réunion proposé à ④ (D9) |
| `agent/src/accent/win32.rs` | `#[cfg(windows)]` | `SendMessageTimeout(WM_GETICON)`, `GetClassLongPtrW`, `GetIconInfo`, `GetDIBits` |
| `client/src/presse-papier.ts` | **PUR, sans DOM** | la machine à états « reçu / focalisé / écrit », le garde n°3, le dépôt différé. Injection de dépendances, patron d'`audio.ts` et `status.ts` |
| `client/src/accent.ts` | **PUR, sans DOM** | la conformation de D10 ; ne prend que des chaînes et rend une chaîne |

⚠️ **La normalisation des fins de ligne n'est pas un détail** : Windows emploie
`\r\n`, le presse-papier des navigateurs et le web `\n`. Un aller-retour non
normalisé double les lignes ou les colle. C'est **pur**, donc testable, et c'est
la première chose qu'un test doit voir rouge.

⚠️ **`agent/src/presse_papier.rs` est un nom AUTONOME** : il ne préfixe aucun
module de premier niveau existant. Par la convention écrite en tête de
`CLAUDE.md` (§ « Convention de module enfant »), il vit **à la racine nue**,
`mod presse_papier;` ordinaire — comme `geometry`, `sortie_dxgi` et
`survie_verdict`. Son enfant `#[cfg(windows)]` se déclare par un simple `mod
win32;` **à l'intérieur** de lui : il n'a jamais besoin de sortir de l'arbre de
son parent, donc il est **hors de portée** de la règle du `#[path]`. Même
raisonnement pour `accent`.

### 7.2 Le plafond de 500 lignes, budgété d'avance

Relevé **par la commande**, le 19 août 2026, depuis la racine :

```
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>420'
```

```
  70215 total
   1536 agent/src/encode.rs
    630 agent/src/windows_source.rs
    500 agent/src/encode/arret.rs
    495 agent/src/transport.rs
    494 client/verify-webrtc.mjs
    492 agent/src/capture.rs
    491 agent/src/demarrage.rs
    481 agent/src/transport/socket.rs
    477 agent/src/transport/piste_video.rs
    474 agent/src/capteur/distante/tests.rs
    472 agent/src/congestion/controleur.rs
    471 agent/src/transport/tick/tests/audio.rs
    471 agent/src/transport/piste_audio.rs
    470 proto/src/control.rs
    468 agent/src/transport/adaptation.rs
    465 agent/src/diagnostics/multifenetre/reprise/passes.rs
    463 agent/src/moniteurs_virtuels/pilote.rs
    460 agent/src/micro.rs
    459 agent/src/geometry.rs
```

⚠️ **Trois chiffres publiés dans `CLAUDE.md` ont dérivé**, et le brief de ce
document reprenait deux d'entre eux :
`agent/src/transport.rs` **495** (le brief disait 495 — exact) ;
`agent/src/demarrage.rs` **491** (exact) ;
mais `client/verify-webrtc.mjs` vaut **494**, quand `CLAUDE.md` publie **497**.
Ce document ne corrige pas `CLAUDE.md` — périmètre concurrent — mais il **ne
recopie pas** le chiffre périmé.

**Les fichiers que ce chantier touche, et leur marge :**

| Fichier | Lignes | Marge | Budget prévu |
| --- | --- | --- | --- |
| `proto/src/control.rs` | **470** | 30 | +1 variante `AgentControl`, +1 `ClientControl`, +1 champ `Capabilities`, +2 constructeurs, +tests ⟹ **franchira 500**. 🔴 **L'extraction est décidée ICI** : `proto/src/control/tests.rs` par `#[path]`, `#[cfg(test)]` — le bloc de tests fait **199 lignes** (`:272-470`), donc l'extraction rend **271** et clôt la question pour longtemps |
| `agent/src/transport/tick.rs` | **403** | 97 | +1 branche `a1…` |
| `agent/src/capteur/protocole.rs` | 419 | 81 | +1 variante par sens, +tests |
| `agent/src/capteur/pont_media.rs` | 263 | 237 | +1 bras, +1 test |
| `agent/src/capteur/fenetre/commandes.rs` | 234 | 266 | +1 bras étage 0, +1 dans le bras miroir `:221-227` |
| `agent/src/capteur/sommeil/registre.rs` | 346 | 154 | le sondage sur le tour de roue |
| `agent/src/capteur/distante.rs` | 409 | 91 | +2 champs, +2 méthodes |
| `agent/src/source.rs` | 393 | 107 | +2 méthodes de trait, à défaut inerte |
| `agent/src/input.rs` | 250 | 250 | l'injection de `Ctrl+V` |
| `client/src/main.ts` | **451** | 49 | +2 branches `onControl` + câblage ⟹ **serrée**. 🔴 **Toute la logique va dans `presse-papier.ts` et `accent.ts`**, `main.ts` ne reçoit que l'appel |
| `client/src/input.ts` | 123 | 377 | l'exception étroite de D6 |
| `proto/ts/control.ts` | 139 | 361 | +2 interfaces, +`TYPES_AGENT`, +1 encodeur |

⚠️ **Aucun fichier de la dette gelée n'est touché** : ni `encode.rs` (1536), ni
`windows_source.rs` (630), ni `capture.rs` (492, marge 8), ni
`encode/arret.rs` (500, marge 0). C'est un effet **direct** de D9 — l'icône
plutôt que l'image capturée — et c'est la moitié de sa justification.

⚠️ **Règle du dépôt, à appliquer et non à découvrir** : l'extraction de
`proto/src/control.rs` se joue **AVANT** l'addition qui la rend nécessaire, pas
après. D9 a payé deux compressions pour l'avoir oublié ; D10 a joué trois
extractions préalables et n'en a payé aucune.

---

## 8. Ce que ce chantier N'ÉTABLIRA PAS

- **Aucun taux.** Comme tous les sous-blocs de ce dépôt, une exécution par
  critère au mieux. Aucun énoncé ne portera de fréquence de succès.
- ~~**Rien du presse-papier Windows n'est MESURÉ à ce jour.**
  `GetClipboardSequenceNumber`, `AddClipboardFormatListener`, le comportement du
  compteur sur une réécriture identique, et le fait qu'une écriture par nos soins
  fasse bien bouger le compteur : **tout cela est de la documentation d'API**, pas
  un relevé. La VM était tenue par un chantier concurrent.~~ **Le premier geste de
  P1 est de le mesurer**, et le critère ② de P1 dit explicitement quoi faire si le
  compteur ne se comporte pas comme annoncé.
  > ✅ **FAIT, et c'est la seule ligne de ce §8 que le chantier a déjà réfutée
  > (20 août 2026, sonde P0, DEUX exécutions).** `GetClipboardSequenceNumber`
  > est **stable au repos**, **bouge à chaque copie** (5 mouvements, 5 lectures,
  > **0 échec d'ouverture**), **bouge sur une réécriture identique**
  > (`q2="bouge"`) et **bouge sur notre propre écriture** (`q3="bouge"`).
  > `AddClipboardFormatListener` n'a, lui, **pas** été mesuré : D2 l'écarte, et
  > la sonde ne l'a pas éprouvé — cette moitié-là de la ligne tient toujours.
  > Pièces : `journaux-presse-papier-p1/p0-sonde-{1,2}.log`.
  > ⚠️ **L'instrument de cette sonde a rendu un FAUX verdict éliminatoire à sa
  > première exécution** (trois zéros sur une VM saine, parce que rien n'avait
  > été copié depuis le démarrage de la station de fenêtres) : **un verdict
  > négatif exige que la chose mesurée soit ABSENTE, pas seulement nulle.**
  > Journal du défaut conservé : `p0-sonde-0-instrument-defectueux.log`.
- **Rien d'un navigateur autre que Chromium**, et rien d'un Chromium **avec
  interface**. Les relevés du §3 sont sans interface, une exécution chacun. Une
  fenêtre de dialogue de permission n'a jamais été affichée à un humain.
- **`writeText` depuis une fenêtre non focalisée n'est pas mesuré** (§3.3) ; le
  critère ④ de P3 le mesurera, et pourra réfuter la règle du dépôt différé.
- ✅ ~~**`paste` sur un `<video>` focalisé n'est pas mesuré** — c'est le préalable
  éliminatoire de P2, et il peut faire échouer le sous-bloc entier.~~ **MESURÉ
  le 20 août 2026, verdict FAVORABLE, deux exécutions, avec sa rouge et son
  témoin de mesurabilité** — voir le §3.3 et le §10 (R1).
- **Aucun jugement visuel n'est porté sur la couleur d'accent.** C'est la lacune
  exacte que ce dépôt traîne depuis `BPP_MIN` (chantier C volet 1), et A1 ne la
  ferme pas : le contraste sera **mesuré**, la **beauté** de la teinte ne le sera
  pas — le design system l'écrit déjà de ses propres tokens (§8, « une centaine
  d'autres bleus passeraient les mêmes seuils »).
- **Aucune des constantes de ce chantier ne sera calibrée** :
  `PERIODE_PRESSE_PAPIER` (250 ms), `PERIODE_ACCENT` (5 s),
  `PRESSE_PAPIER_MAX` (64 KiB). Elles rejoignent la liste tenue par ce dépôt.
- **La couleur d'accent n'aura aucun effet visible** tant que ② n'a pas livré de
  manifeste (D10). Ce chantier livre la valeur, pas son rendu.
- **Rien du presse-papier au-delà de trois fenêtres**, et le nombre réellement
  atteint sera **relevé**, pas supposé.
- **Rien de la latence de bout en bout**, qu'aucun sous-bloc du chantier D n'a
  jamais mesurée et que celui-ci ne mesurera pas davantage.
- **Le presse-papier reste PARTAGÉ entre les fenêtres d'une même session** (D3) —
  c'est une propriété du modèle, et aucune mesure ne dira si les utilisateurs
  l'attendaient.

---

## 9. Hors périmètre v1, explicitement

- **Images, fichiers, RTF, HTML dans le presse-papier.** `ClipboardItem.supports`
  les accepte (§3.2) : c'est une décision de produit, prise ici (§0, D4), et
  révisable le jour où le canal ne serait plus le goulot. L'ancien produit les
  portait (`web/index.js:330-345`) ; `CLAUDE.md:8879` les rangeait déjà en P3.
- **`readText()` et la permission `clipboard-read`.** Le nouveau produit ne
  demande **aucune** permission de presse-papier (D8). Y revenir serait le repli
  si le contrôle ① de P2 échouait — et ce serait un recul, pas une évolution.
- **Le presse-papier entre deux sessions d'utilisateurs différents.** Le
  presse-papier Windows est global à la window station ; ce document ne touche
  pas à la question de savoir si deux utilisateurs partagent une VM. C'est un
  sujet de ⑤, pas de ①.
- **L'historique de presse-papier**, la synchronisation à froid, le collage
  différé, le collage sélectif de format.
- **La couleur d'accent par thème de document** (un Firefox sombre) : c'est
  l'échantillonnage de l'image capturée, écarté par D9 avec son prix écrit.
- **Le manifeste PWA, `theme-color`, les icônes servies** : ils appartiennent à
  ② (`2026-08-19-design-system-design.md:856`), et la couleur statique par
  application appartient à ④/G5 (`2026-08-19-gestion-apps-design.md:762-763`).
- **L'application VISUELLE de `--accent-fenetre`** : ⑥/S4.

---

## 10. Risques, et ce qui rendrait ce chantier non livrable

| # | Risque | Gravité | Ce qui le lève, ou le borne |
| --- | --- | --- | --- |
| R1 | ~~🔴 **`paste` ne parvient pas sur un `<video>` focalisé**~~ | ~~éliminatoire pour P2~~ | ✅ **LEVÉ le 20 août 2026** — verdict FAVORABLE, deux exécutions, journaux versés sous `journaux-presse-papier-p2/`. La sonde mesure une matrice de 24 cellules dans une seule session et porte les deux témoins qui manquaient à la sonde P0 de P1 : une **ROUGE d'instrument** (le régime « produit » rend zéro `paste`) et un **témoin de mesurabilité** (`body` rend un `paste` dans la même session). Le repli `readText()` n'est PAS emprunté, et **le produit livré ne demande aucune permission de presse-papier** |
| R2 | 🔴 **Le bras de `pont_media.rs` est oublié** | tue la session au premier message | c'est le **critère ① de P1**, joué en rouge avant le vert. Cinquième rappel d'un défaut payé quatre fois |
| R3 | 🔴 **Le type est oublié dans `TYPES_AGENT`** | message perdu contre un `console.warn`, **aucun test ne le voit** | §2.2. Le contrôle est un test qui confronte `TYPES_AGENT` à l'union — **qu'aucun test ne fait aujourd'hui** |
| R4 | ⚠️ **`WM_GETICON` bloque sur une application figée** | **gèle le tour de roue du capteur, donc TOUTES les fenêtres** | `SendMessageTimeout` + `SMTO_ABORTIFHUNG` obligatoire (D9). Repli non bloquant : `GetClassLongPtrW` |
| R5 | ⚠️ **L'élargissement de la condition de D6 rend le navigateur au clavier** | `Ctrl+W` fermerait la fenêtre de session | critère ⑤ de P2, joué explicitement |
| R6 | ⚠️ **Le garde anti-écho a un trou** | oscillation permanente, trafic sans fin | deux gardes (D5), dont l'un exact ; critère ④ de P2, avec sa rouge |
| R7 | ⚠️ **`OpenClipboard` échoue parce qu'une autre application le tient** | copie ou collage perdu | c'est un cas **normal** sous Windows, pas une panne : réessai borné au sondage suivant, puis abandon **journalisé**. Jamais une boucle d'attente |
| R8 | ⚠️ **Un accent illisible atteint `:root`** | texte invisible, et **aucun des sept contrôles ne le verrait** | D10 : validation pure, refus au lieu de correction, test vu rouge (A1-③) |
| R9 | ⚠️ **`PRESSE_PAPIER` n'est pas transmise par `scripts/run-agent.sh`** | l'agent démarre sans elle **et ne le dit pas** | payé en D1, D2 et D7 ; à faire **dans la tâche qui introduit la variable**, jamais après |
| R10 | ⚠️ **`proto/src/control.rs` franchit 500** | dette neuve | extraction des tests **avant** l'addition (§7.2), pas après |
| R11 | ⚠️ **Le presse-papier fuit entre deux utilisateurs d'une même VM** | confidentialité | hors périmètre v1 (§9), **nommé et non traité** — la question appartient à ⑤ et elle est réelle |

---

## 11. Annexe — la liste des points de passage, à cocher

Un message neuf traverse **onze** endroits. Ceux marqués 🔴 **ne se signalent
par aucune erreur de compilation** : les oublier produit une panne silencieuse.

**Sens capteur → navigateur** (`PressePapier`, `Accent`) :

1. `agent/src/capteur/protocole.rs` — variante de `DepuisCapteur`
2. 🔴 `agent/src/capteur/pont_media.rs` — le bras, **et son test**
3. `agent/src/capteur/distante.rs` — `Recu`, le champ, la méthode
4. `agent/src/source.rs` — la méthode de trait, à défaut inerte
5. `agent/src/transport/tick.rs` — la branche `a1…`
6. `agent/src/transport/controle.rs:98-107` — le nom au journal *(gardé par le compilateur)*
7. `proto/src/control.rs` — la variante d'`AgentControl` + son constructeur
8. `proto/ts/control.ts` — l'interface, **l'union**, et 🔴 **`TYPES_AGENT`**
9. `client/src/main.ts` — la branche `onControl`

**Sens navigateur → capteur** (`Clipboard`) :

10. `proto/src/control.rs` — la variante de `ClientControl` ;
    `agent/src/transport/evenements.rs:251-258` *(gardé par le compilateur)*
11. `agent/src/capteur/fenetre/commandes.rs` — le bras **étage 0** (avant la
    source, comme `Visibilite`/`AudioMort`/`AudioVivant`, parce qu'il ne touche
    ni encodeur ni duplication) **et** le bras miroir de `:221-227`
    *(gardé par le compilateur)*

> Cette liste est le livrable le plus réutilisable de ce document : **elle vaut
> pour tout message futur**, pas seulement pour celui-ci.
