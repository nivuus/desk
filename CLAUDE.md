# CLAUDE.md

> **⚠️ IMPORTANT**: Ce fichier contient toutes les informations essentielles pour la continuation du projet. **Toute information importante pour la suite du développement DOIT être stockée dans ce fichier.** Cela inclut les configurations critiques, et toute connaissance acquise pendant le développement.

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Guacamole is a Node.js-based remote desktop web application that provides browser-based access to Windows applications via Apache Guacamole protocol (RDP). The system creates isolated sessions for running remote Windows applications with a custom filesystem bridging client and server.

## 📏 Conventions de code

### Taille maximale d'un fichier : 500 lignes

**Un fichier de code source ne doit pas dépasser 500 lignes.** Au-delà, le
fichier porte plus d'une responsabilité : il faut le découper avant d'y ajouter
quoi que ce soit.

**Portée** — la règle s'applique au code source écrit à la main :
`agent/src/`, `client/src/`, `signaling/`, `proto/`, `src/`, `web/`, `scripts/`.

**Exemptions explicites** :

- `docs/` — les plans, specs et recettes sont des journaux d'exécution, longs
  par nature et non maintenus comme du code.
- `CLAUDE.md` — ce fichier est un index de connaissances, pas du code.
- Fichiers générés ou vendorisés : `dist/`, `target/`, `node_modules/`,
  `*-lock.json`, `Cargo.lock`, `testdata/`.

**Règle d'application** : geler la dette, pas la purger. Aucun **nouveau**
fichier ne naît au-dessus de 500 lignes, et un fichier déjà au-dessus ne doit
pas grossir davantage — toute addition substantielle s'accompagne d'une
extraction. Le découpage rétroactif des fichiers ci-dessous se fait au moment
où l'on travaille dedans, pas en chantier séparé.

**Dette existante** (code source uniquement) :

| Fichier | Lignes | Pourquoi elle reste |
| --- | --- | --- |
| `agent/src/encode.rs` | 1536 | `#[cfg(windows)]`, aucun test |
| `agent/src/windows_source.rs` | ~~638~~ **628** (6 août 2026, D9) | `#[cfg(windows)]`, aucun test |

> ✅ **`wasapi.rs` SORT de cette table (3 août 2026, sous-bloc D7, tâche 1).**
> Il faisait 543 lignes ; la tâche 1 en a extrait la machinerie COM du *process
> loopback* (`EtatActivation`, `ResultatActivation`, `GestionnaireCompletion`,
> `probe_process_loopback`, `pour_processus`) vers
> **`agent/src/wasapi/process_loopback.rs`**. **Relevé par la commande le
> 4 août 2026** : `wasapi.rs` **352** lignes, `wasapi/process_loopback.rs`
> **402**. Les deux sont sous le plafond, et `process_loopback.rs` reste
> `#[cfg(windows)]` et sans test — ce n'est donc pas une dette purgée, seulement
> déplacée sous la ligne des 500. La table n'a donc plus que **deux** entrées.

> ⚠️ **`windows_source.rs` est passé de 631 à 638 lignes le 3 août 2026
> (sous-bloc D6, commits `1523c36` et `5a9267d`) : +7 sur de la dette GELÉE.**
> Ces trois chiffres sont **relevés par la commande ci-dessous ce jour-là**, pas
> recopiés. **La croissance est déclarée avec sa justification et sa
> condition** :
>
> - **justification** — l'addition est **100 % commentaire**. Elle n'ajoute
>   **rien** à la surface non testée que la règle des 500 lignes existe pour
>   contenir, et elle inscrit la réfutation d'une justification portante devenue
>   fausse (« l'agent étant mono-session, des statiques ne mélangent pas
>   plusieurs sessions » — faux depuis D4) au lieu de la retirer. **Raccourcir
>   une réfutation pour atteindre un compte de lignes échangerait une vérité
>   contre un nombre** — geste que cet encadré interdit déjà pour `arret.rs` ;
> - **condition, puisqu'il n'y a eu AUCUNE extraction** : **la prochaine
>   addition à ce fichier, de quelque nature qu'elle soit, exige une
>   extraction.** Son point de chute est nommé — **`agent/src/windows_source/telemetrie.rs`** —
>   et c'est aussi le remède de fond : le jour où `TICKS`, `CAPTURED` et
>   `PRODUCED` deviendront per-session (consignation n°1 de D7), ils sortiront
>   du fichier et lui rendront cette marge.
>
> ✅ **CETTE CONDITION EST LEVÉE, et par le remède annoncé lui-même (6 août
> 2026, sous-bloc D9, tâche 11).** Les trois statiques `TICKS`/`CAPTURED`/
> `PRODUCED` sont devenues un champ **par session** de `WindowsSource` et sont
> sorties vers **`agent/src/windows_source/telemetrie.rs`** (**72** lignes,
> **pur, aucun `cfg`, deux tests d'hôte**) — exactement le point de chute
> nommé trois ans de lecture plus haut. `windows_source.rs` retombe de **638 à
> 628** (relevé par la commande le 6 août 2026), et **il a récupéré plus que
> les 7 lignes que D6 lui avait prises**. La dette de taille reste, elle,
> entière : 628 est toujours au-dessus de 500, toujours `#[cfg(windows)]`,
> toujours sans test.

> `encode.rs` est passé de 1502 à 1536 lignes le 31 juillet 2026 (correctif de
> libération des encodeurs). **Cette croissance de +34 est régulière au regard
> de la règle ci-dessus** : l'addition s'est accompagnée de son extraction —
> l'essentiel de la logique neuve vit dans `agent/src/encode/arret.rs`, et seul
> le câblage est resté dans `encode.rs`.
>
> ⚠️ **`arret.rs` est à 500 lignes exactement : sa marge est NULLE.** Il en
> faisait 457 à sa création ; la revue finale de branche l'a porté à 500 en y
> inscrivant la portée présente et la borne du pire cas de `Drop`, et a dû
> resserrer sa propre rédaction pour ne pas franchir le plafond. **Toute
> addition future à ce fichier appelle une extraction, jamais une compression
> supplémentaire** — la compression y a déjà été jouée, et elle ne l'est qu'une
> fois.

> ⚠️ **`agent/src/capture.rs` est à 496 lignes : sa marge est de 4 lignes**
> (2 août 2026, fin du sous-bloc D3 — voir le §8 de
> `plans/2026-08-02-multifenetres-plafond-concurrence-resultats.md`). Il a
> atteint **exactement 500** en cours de sous-bloc D2 ; une extraction vers
> `capture/ouverture.rs` l'a fait retomber à **453**, puis une ronde de
> correction lui a **rendu 32 lignes** (485 à la fin de D2), et il a repris
> **11 lignes** depuis. **Toute addition future à ce fichier appelle une
> extraction** — les modules enfants `capture/reprise.rs`,
> `capture/ouverture.rs` et `capture/types.rs` existent déjà et sont le bon
> endroit. **Leçon du chiffre, vérifiée deux fois plutôt qu'une : la marge
> regagnée par une extraction se reperd à la ronde suivante si on la traite
> comme acquise.**
>
> ✅ **Le chiffre a bougé, et pour une fois dans le bon sens : `capture.rs` vaut
> ~~496~~ **492**, marge ~~4~~ **8** (relevé par la commande le 6 août 2026,
> D9).** Aucune extraction : `DesktopCapture::cible()`, **orphelin non
> revendiqué**, est parti avec le changement de mode de sortie (tâche 3). **Une
> marge peut aussi se regagner en retirant du code mort — mais c'est un effet de
> bord, pas une méthode**, et l'injonction ci-dessus tient sans changement.**

Ces trois modules ne se compilent que sur la VM et ne sont couverts par aucun
test : les découper se ferait sans filet automatisé. La dette est assumée
jusqu'à ce qu'ils gagnent des tests — voir
`docs/superpowers/specs/2026-07-30-dette-taille-fichiers-design.md` §1.

Les quatre fichiers que la suite de tests couvrait ont été résorbés le
30 juillet 2026 : voir `docs/superpowers/plans/2026-07-30-dette-taille-fichiers.md`.

> ⚠️ **Les nombres de ce tableau et des deux encadrés ci-dessus ont été relevés
> le 2 août 2026, et ils DÉRIVENT** : rien ne les met à jour hors un chantier
> qui touche le fichier concerné. Deux d'entre eux étaient faux au moment de ce
> relevé — `windows_source.rs` était annoncé à 740 pour **648** réels, et
> `capture.rs` à 485 pour **496**. **Ne jamais s'y fier pour décider si un
> fichier peut encore grossir : relancer la commande ci-dessous**, qui est la
> seule source de vérité, et **corriger le tableau dans le même mouvement**.
>
> ✅ **Relancée le 2 août 2026 en fin de recette D4 : les trois lignes du tableau
> sont EXACTES** (1536 / 648 / 543), et `capture.rs` (496), `arret.rs` (500) et
> `superviseur/table.rs` (489) le sont aussi. **Deux chiffres voisins ne l'étaient
> plus** — `superviseur/boucle.rs` est à **491** et `demarrage.rs` à **472**,
> quand la conception de D4 les annonçait à 485 et 468.
>
> ⚠️ **Marge étroite NEUVE, qu'aucun document ne signalait :
> `agent/src/capteur/distante.rs` est à 487 lignes, soit une marge de 13.**
> C'est un fichier né avec D4 (voir « Sous-bloc D4 ») : **toute addition
> substantielle y appelle une extraction, pas une compression.**
>
> ❌ **CE CHIFFRE EST FAUX, et il l'était déjà quand il a été écrit — relevé du
> 3 août 2026 (sous-bloc D5) : `agent/src/capteur/distante.rs` fait 235 lignes**,
> ses tests vivant depuis D4 dans `agent/src/capteur/distante/tests.rs` (385).
> Le 487 est le compte d'AVANT cette extraction, jamais repris ensuite. **Il n'y
> a donc jamais eu de marge de 13 sur ce fichier** — et c'est exactement la
> dérive contre laquelle l'encadré ci-dessous prévient : un nombre recopié
> survit à la réalité qu'il décrivait. Les trois autres occurrences de ce 487
> dans ce fichier sont annotées de la même façon.
>
> ⚠️ **Le 235 a vieilli à son tour** : le sous-bloc D6 a porté
> `agent/src/capteur/distante.rs` à **288** (relevé par la commande le 3 août
> 2026, vague de correction finale de branche). Marge 212 — le fichier reste
> très en dessous du plafond, mais **le nombre qui le disait n'était déjà plus
> le bon.**
>
> ✅ **Relance du 2 août 2026, après la SECONDE recette de D4 : les trois lignes
> du tableau sont toujours exactes** (1536 / 648 / 543), et **aucun autre fichier
> de code source ne dépasse 500 lignes**. La tâche 10 a fait grossir
> `agent/src/capteur/` sans franchir aucun plafond : `distante.rs` **487**
> (inchangé, le remède du canal n'est pas passé par lui), `serveur.rs` **433**,
> `fenetre.rs` **329**, `tube.rs` **298**. Les marges étroites relevées à cette
> date, **par la commande et non recopiées** : `encode/arret.rs` **500** (marge
> 0), `capture.rs` **496** (4), `superviseur/boucle.rs` **491** (9),
> `superviseur/table.rs` **489** (11), `capteur/distante.rs` **487** (13),
> `transport/socket.rs` **481** (19), `transport/piste_video.rs` **477** (23),
> `demarrage.rs` **472** (28).
>
> ❌ **Les deux `487` de ce paragraphe sont faux** (voir l'annotation ci-dessus) :
> `capteur/distante.rs` fait **235** lignes, et la marge de 13 n'a jamais
> existé. Le reste de la liste n'est pas réfuté, mais **il a vieilli**. Relevé
> du 3 août 2026, fin du sous-bloc D5, **par la commande** : `encode/arret.rs`
> **500** (marge 0), `capture.rs` **496** (4), `superviseur/boucle.rs` **493**
> (7), `capteur/serveur.rs` **490** (10), `superviseur/table.rs` **489** (11),
> `transport/socket.rs` **481** (19), `transport/piste_video.rs` **477** (23),
> `transport/adaptation.rs` **472** (28), `demarrage.rs` **472** (28). Et
> `windows_source.rs` a **maigri** de 648 à **631** : le remède de D5 y remplace
> une reconstruction d'encodeur par une destruction préalable.
>
> ⚠️ **Seconde marge étroite NEUVE, relevée après la vague de correction de la
> revue finale de branche : `agent/src/capteur/serveur.rs` est à 490 lignes,
> soit une marge de 10.** Il était à **433** avant cette vague : **+57 en une
> seule ronde**, dont l'essentiel est le commentaire que la revue avait
> explicitement exigé pour justifier `TAMPON`. La croissance est donc légitime,
> et c'est précisément pourquoi elle mérite l'encadré : **la leçon déjà payée
> deux fois par ce dépôt — « la marge regagnée par une extraction se reperd à la
> ronde suivante si on la traite comme acquise » — s'applique littéralement ici.**
> Toute addition future à ce fichier appelle une extraction, jamais une
> compression du commentaire de `TAMPON`, qui doit rester auprès de la constante
> qu'il justifie.
>
> ✅ **CETTE INJONCTION A ÉTÉ TENUE À LA LETTRE, et par une tâche DÉDIÉE placée
> AVANT celle qui devait y ajouter du code (6 août 2026, D9, tâche 6).**
> `agent/src/capteur/serveur/instances.rs` (**82** lignes) est extrait, et **le
> commentaire de `TAMPON` part AVEC sa constante** — comparé mot pour mot par la
> revue. `serveur.rs` retombe de ~~490~~ à **435** : **la marge passe de 10 à
> 65**, et la tâche 9 y a ensuite ajouté son bras `AudioMort` sans franchir
> quoi que ce soit. **C'est la première fois dans ce dépôt qu'une marge étroite
> est traitée AVANT l'addition plutôt qu'après**, et c'est ce qui a évité la
> compression que les deux autres fichiers de ce même sous-bloc ont subie
> (`sommeil.rs`, `capteur/fenetre.rs` — voir la section D9).
>
> ✅ **Relance du 3 août 2026, fin du sous-bloc D6, PAR LA COMMANDE** : les trois
> lignes du tableau sont exactes après correction — **1536 / 638 / 543** —, le
> `638` étant le seul mouvement (voir l'encadré du tableau), et **aucun autre
> fichier de code source ne dépasse 500 lignes**. Marges étroites de ce relevé :
> `encode/arret.rs` **500** (marge 0), `capture.rs` **496** (4),
> `superviseur/boucle.rs` **493** (7), `capteur/serveur.rs` **490** (10),
> `superviseur/table.rs` **489** (11), `transport/socket.rs` **481** (19),
> `transport/piste_video.rs` **477** (23), `demarrage.rs` **472** (28),
> `transport/adaptation.rs` **468** (32).
>
> ⚠️ **Un chiffre de l'annotation ci-dessus a vieilli, et il faut le dire** :
> `transport/adaptation.rs` y est donné à **472** (relevé D5) ; il vaut **468**.
> D6 l'a fait franchir 500 (562 en accueillant le câblage des parts) puis
> **extrait** vers `agent/src/transport/part.rs` (**138 à l'extraction ; 274
> aujourd'hui**, voir plus bas), d'où 456 puis 468 après
> deux rondes de documentation. **La marge regagnée par une extraction se
> reperd** : +12 en deux rondes, sur le fichier même qui venait d'être découpé.
> C'est la troisième fois que ce dépôt paie cette leçon.
>
> ✅ **Les fichiers nés de D6 sont tous très en dessous du plafond** :
> `capteur/repartiteur.rs` **147**, `capteur/sommeil/parts.rs` **348**,
> `transport/part.rs` **274**, `capteur/pont_media.rs` ~~188~~ **263**
> (relevé du 5 août 2026 ; les trois autres sont inchangés). ⚠️ **Ce `188` est le
> quatrième chiffre périmé trouvé dans CE paragraphe** — celui-là même que
> l'encadré ci-dessous nomme « le SOMMAIRE, le seul endroit qu'un chantier
> suivant lira ».
>
> ⚠️ **Et sa dérive s'est faite en DEUX temps, dont un que personne n'a
> enregistré** : **188 (D6) → 225 (D7, +37, absent de TOUS les tableaux de D7)
> → 263 (D8, +38, le bras `PleinEcran` du `match` catch-all et son test)**.
> Chiffres relevés par la commande et par `git show 7c44f51:…`. **Un fichier
> peut donc dériver sans figurer dans aucun relevé de la branche qui le fait
> dériver** — le seul remède reste la commande.
>
> ❌ **DEUX DE CES QUATRE CHIFFRES ONT ÉTÉ FAUX, et ils l'étaient au moment même
> où la vague de correction finale de branche corrigeait les MÊMES nombres dans
> le document de résultats.** Ils portaient **119** et **138** ; la commande rend
> **147** et **274**, et la ligne ci-dessus est corrigée. `parts.rs` **348** et
> `pont_media.rs` **188** étaient, eux, exacts.
>
> ⚠️ **C'est le naufrage du « 487 » rejoué à l'identique, et il faut voir
> comment.** Le tableau du §« Ce que le code livre » de la section D6 a bien été
> corrigé dans le même mouvement — mais **pas ce paragraphe-ci**, qui est le
> SOMMAIRE, le seul endroit qu'un chantier suivant lira pour savoir de quelle
> marge il dispose. *Corriger une affirmation exige de la CHERCHER, pas de la
> corriger là où on nous l'a montrée* : la règle est écrite trois fois plus haut
> dans ce fichier, et elle a quand même été payée une quatrième.
>
> ✅ **Relevé complet des fichiers que la branche D6 a fait bouger, PAR LA
> COMMANDE, à la vague de correction finale** — aucun n'approche 500 :
>
> | Fichier | Lignes | Remarque |
> | --- | --- | --- |
> | `agent/src/transport/adaptation.rs` | **468** | inchangé depuis le relevé ci-dessus |
> | `agent/src/capteur/sommeil.rs` | ~~432~~ **269** (6 août 2026, D9) | **le fichier qui a le plus grossi de la branche** — il portait le registre entier (vivier partagé, canaux, focus, tour de roue). ⚠️ **D9 a extrait ce registre vers `sommeil/registre.rs` (331)**, sur exigence de revue, après l'avoir d'abord ramené à 499 **par compression** — geste que ce fichier interdit |
> | `agent/src/capteur/distante/tests.rs` | ~~409~~ **474** (D8, inchangé sous D9) | |
> | `agent/src/capteur/fenetre.rs` | ~~407~~ **485** (6 août 2026, D9 — marge 15) | |
> | `agent/src/capteur/sommeil/parts.rs` | ~~348~~ **349** (D9) | |
> | `agent/src/capteur/protocole.rs` | **347** | |
> | `agent/src/source.rs` | **334** | |
> | `agent/src/congestion/reconfiguration.rs` | **306** | |
> | `agent/src/capteur/distante.rs` | **288** | |
> | `agent/src/transport/tick.rs` | **275** | |
> | `agent/src/capteur/vivier.rs` | **275** | inchangé au 6 août 2026 |
> | `agent/src/transport/part.rs` | ~~274~~ **301** (D9 — `PART_SONDAGE=0`) | |
> | `agent/src/capteur/repartiteur.rs` | **147** | |
>
> ⚠️ **Quatre chiffres de CE tableau ont vieilli à leur tour, et pour la même
> raison que celui de `capteur/fenetre.rs` ci-dessous : du travail ultérieur les
> a fait bouger sans que la table ne soit reprise.** Relevé par la commande le
> 4 août 2026, fin du sous-bloc D7 — **ce sont des fichiers que D7 a modifiés**
> (tâches 6, 7 et 10 : protocole de fenêtre, source distante, tick de
> transport) :
>
> | Fichier | Publié ci-dessus | Réel (4 août 2026) | Cause du mouvement |
> | --- | --- | --- | --- |
> | `agent/src/capteur/protocole.rs` | 347 | ~~361~~ ~~369~~ **381** (6 août) | D7 tâche 6 — le message `Audio` ; D8 y ajoute `PleinEcran`, +8 ; **D9 y ajoute `AudioMort`, +12** |
> | `agent/src/capteur/distante.rs` | 288 | ~~332~~ ~~375~~ **400** (6 août) | D7 tâche 7 — `est_endormie` et l'état audio ; puis la vague F2 (354) ; puis D8, +21 ; **puis D9, +25** (`signaler_audio_mort`, `rattachement_survenu`) |
> | `agent/src/source.rs` | 334 | ~~348~~ ~~362~~ **387** (6 août) | D7 — le trait `VideoSource`/audio gagne une méthode ; D8 y ajoute +14 ; **D9 y ajoute deux méthodes de trait, +25** |
> | `agent/src/transport/tick.rs` | 275 | ~~297~~ ~~308~~ **343** (6 août) | D7 tâche 10 — le fil de fenêtre relaie l'ordre `Audio` ; D8 y relaie `Fullscreen`, +20/-9 ; **D9 y ajoute la branche a1sexies, +35** |
>
> Aucun des quatre n'approche 500 (marge la plus étroite : 168 sur
> `distante.rs`), donc aucune entrée de dette n'est à tort présente ou absente.
> Mais **ce sont précisément des chiffres que ce fichier a payé cher ailleurs
> pour avoir laissés dériver** — voir le naufrage du « 487 » sur ce même
> `distante.rs` un peu plus haut dans ce document.
>
> ⚠️ **`superviseur/boucle.rs` dérive aussi d'une ligne** : publié à **493**
> plus haut (relevés D5 et D6), il vaut **492** aujourd'hui. Non touché par D7 ;
> écart trop mince pour identifier une cause, corrigé ici puisque cette même
> table était de toute façon rouverte.
>
> ⚠️ **Deux autres chiffres de ce fichier ont vieilli au passage, et ils sont
> corrigés ici plutôt que là où ils dorment** : `capteur/fenetre.rs` est donné à
> **329** dans le relevé du 2 août (D4) — il vaut **407** ; et
> `capteur/tube.rs`, donné à **298** dans la même phrase, vaut **263**. Ce
> dernier n'a pas été touché par D6 : **il dérivait déjà**, ce qui est
> exactement la raison pour laquelle ces nombres ne se recopient jamais.
>
> ✅ **Relance du 4 août 2026, fin du sous-bloc D7, PAR LA COMMANDE.** Le tableau
> de dette n'a plus que **DEUX** lignes — `wasapi.rs` en est sorti, voir
> l'encadré posé juste au-dessus de la table — et **aucun fichier de code
> source ne dépasse 500 lignes**. Les deux entrées restantes sont inchangées :
> `encode.rs` **1536**, `windows_source.rs` **638**. Fichiers que D7 a fait
> bouger, tous mesurés par la commande, aucun proche du plafond :
>
> | Fichier | Lignes | Remarque |
> | --- | --- | --- |
> | `agent/src/wasapi/process_loopback.rs` | **402** | neuf — la machinerie COM extraite de `wasapi.rs` |
> | `agent/src/wasapi.rs` | **352** | sort de la dette gelée (543 → 352) |
> | `agent/src/windows_audio.rs` | ~~399~~ **479** (marge 21) | préexistant (chantier A), modifié — porte la trace `compteurs audio` que la recette d'entrée de D8 relit |
> | `agent/src/superviseur/table.rs` | ~~468~~ ~~493~~ **494** (marge **6**) | ALLÉGÉ par D7 — 489 → 468 : la tâche 9 de D7 retire `audio_libre` et le champ `audio`. ⚠️ **REPRIS ET DÉPASSÉ par D8** : +25/-0 (`rafraichir_taille_sortie`), marge 7 ; ⚠️ **et D9 lui prend encore UNE ligne** (un commentaire) : **marge 6 au 6 août 2026, la plus serrée du dépôt après `arret.rs`** |
> | `agent/src/congestion/controleur.rs` | **472** (marge 28) | modifié — `audio_bps` suit désormais l'arbitrage au lieu d'être posé inconditionnellement |
> | `agent/src/capteur/fenetre.rs` | ~~437~~ ~~470~~ **485** (marge 15) | ALOURDI — 407 → 437 : le span `tracing` porteur de `session` (consignation n°2 de D6) et l'arbitrage du son. ⚠️ D8 y ajoute +33 (lecture du style, `SuiviBordure`, `PERIODE_STYLE`) : 470 au 5 août. ⚠️ **D9 l'a fait franchir 500 (508) puis EXTRAIT** `fenetre/trace.rs` (**43**) : **485 au 6 août 2026** |
> | `agent/src/demarrage.rs` | **457** (marge 43) | 472 → 457 malgré l'ajout du câblage audio, extrait vers `demarrage/audio.rs` |
> | `agent/src/demarrage/audio.rs` | **77** | neuf |
> | `agent/src/capteur/audio.rs` | ~~152~~ **228** (6 août 2026, D9) | neuf — la règle pure de l'arbitrage, sans aucun `cfg`, testée sur l'hôte. **D9 y ajoute le champ `inapte` et quatre tests** |
> | `agent/src/capteur/sommeil.rs` | ~~315~~ ~~327~~ **269** (6 août 2026, D9) | ALLÉGÉ — 432 → 315 : montée à 500 en cours de tâche puis ses tests extraits vers `sommeil/tests.rs`. ⚠️ **D9 l'a fait franchir 500 (à 499 après COMPRESSION, geste que ce fichier interdit), puis la revue a exigé l'EXTRACTION** de `sommeil/registre.rs` (**331**) : 269 |
> | `agent/src/capteur/sommeil/tests.rs` | **203** | neuf |
>
> ✅ **Relevé de la VAGUE DE CORRECTION de la revue finale de branche (4 août
> 2026), PAR LA COMMANDE.** Deux chiffres du tableau ci-dessus ont bougé dans
> cette vague même, et sont **barrés à leur place** plutôt que corrigés
> ailleurs. Les autres fichiers qu'elle touche :
>
> | Fichier | Lignes | Remarque |
> | --- | --- | --- |
> | `agent/src/capteur/distante/tests.rs` | ~~462~~ **474** (marge 26) | +29 (433 → 462) : le test du rattachement muet (F2). ⚠️ **D8 y ajoute +12** (le message `PleinEcran`) : **474 au 5 août 2026** |
> | `agent/src/capteur/distante.rs` | ~~354~~ ~~375~~ **400** | +22 : le rattachement remet l'enfant au silence (F2). ⚠️ D8 y ajoute +21 (375 au 5 août) ; ⚠️ **D9 y ajoute +25** : **400 au 6 août 2026** |
> | `agent/src/audio.rs` | **273** | +88 (185 → 273) : `LECTURES_ECHOUEES_MAX` et `temporisation_de_reprise`, PURS et éprouvés sur l'hôte, avec leurs deux tests (F3) |
> | `agent/src/transport/piste_audio.rs` | **218** | +24 : le budget audio se conditionne à l'existence d'une source (F4), et `capture_morte` au journal (F3) |
>
> ❌ **Les deux deltas ci-dessus (`+53` sur `tests.rs`, `+87` sur `audio.rs`)
> étaient FAUX, relevés le 4 août 2026 par la commande** (`git diff --numstat`
> entre le début et la fin de la vague) : `distante/tests.rs` a gagné **+29**
> (433 → 462, pas 409 → 462) et `audio.rs` **+88** (185 → 273). Le `409` de
> départ n'a jamais été mesuré pour cette vague : c'est le chiffre **D6** de la
> ligne 221 de ce même fichier, recopié au lieu d'être relevé — le même geste
> que le naufrage du « 487 » de `distante.rs`, documenté quatre fois plus haut
> dans ce fichier, et que le 119/138 de `repartiteur.rs`/`part.rs` rejouait déjà
> « à l'identique » (voir ci-dessus, section D6). **Corrigé dans le tableau
> ci-dessus** ; l'absolu de 462 restait juste.
>
> ⚠️ **`windows_audio.rs` est passé de 399 à 479 : sa marge est de 21**, et
> **il n'y a eu AUCUNE extraction** — la croissance est presque entièrement du
> commentaire (la réfutation de F1) et la tolérance aux erreurs de lecture (F3),
> dont la partie *pure* a bien été portée dans `audio.rs`, où elle est testable.
> **Toute addition future à ce fichier appelle une extraction, jamais une
> compression** ; son point de chute est nommé — `agent/src/windows_audio/`, en
> commençant par le corps du fil de capture. Et le successeur immédiat est
> connu : le signal enfant→capteur que F3 laisse hors périmètre passera par ici.
>
> Les fichiers du tableau du 3 août (D6) **absents de la liste ci-dessus ne
> sont PAS tous des fichiers que D7 a laissés intacts** — quatre d'entre eux
> (`protocole.rs`, `distante.rs`, `source.rs`, `transport/tick.rs`) ont bien
> bougé sous D7 et sont corrigés à l'endroit où ce tableau du 3 août les publie
> (voir l'encadré juste au-dessus de ce même tableau), pas répétés ici. Pour
> tout fichier ne figurant dans **aucun** des deux tableaux, relancer la
> commande avant de s'y fier.

> ✅ **Relance du 5 août 2026, fin du sous-bloc D8, PAR LA COMMANDE.** Le tableau
> de dette a toujours **DEUX** lignes, inchangées — `encode.rs` **1536**,
> `windows_source.rs` **638** — et **aucun fichier de code source ne dépasse
> 500 lignes**. D8 n'a touché ni l'un ni l'autre : la tâche 9 a délibérément
> évité d'ajouter un champ à `windows_source.rs` (le nom `\\.\DISPLAYn` est
> dérivé du `hwnd` par `MonitorFromWindow`), et le code neuf du changement de
> mode vit dans un module petit-fils.
>
> ⚠️ **MARGE ÉTROITE NEUVE, et c'est la plus serrée du dépôt après
> `encode/arret.rs` : `agent/src/superviseur/table.rs` est à ~~493~~ **494**
> lignes, marge ~~7~~ **6** (relevé du 6 août 2026 : D9 y ajoute une ligne de
> commentaire, et c'est désormais la marge la plus serrée du dépôt après
> `encode/arret.rs`).** Il était à 468 à la fin de D7 ; la tâche 9 lui a ajouté **+25/-0**
> (`rafraichir_taille_sortie`), **aucune compression, aucune extraction**.
> **Toute addition future à ce fichier appelle une extraction** — ses tests de
> rétention vivent déjà à part (`superviseur/table/tests_retention.rs`, **322**),
> c'est le bon point de chute pour la suite.
>
> Fichiers que D8 a fait bouger, **tous mesurés par la commande** :
>
> | Fichier | Lignes | Remarque |
> | --- | --- | --- |
> | ~~`agent/src/windows_source/redimensionnement/mode_sortie.rs`~~ **CE FICHIER N'EXISTE PLUS** (supprimé le 6 août 2026, D9 tâche 3 — voir la section D9) | ~~427~~ ~~446~~ ~~458~~ | neuf — `ChangeDisplaySettingsExW` et `borner_a_la_taille_max`, module petit-fils **pour ne pas ajouter une ligne `mod` à `windows_source.rs`** (jugé sain, sauvé par l'honnêteté de son commentaire ; **premier module à remonter d'un cran le jour du découpage**) |
> | `agent/src/diagnostics/multifenetre/mode_sortie.rs` | ~~421~~ **383** (6 août 2026, D9) | neuf — la sonde P1, deux fois écrite (la première ne pouvait pas échouer). ⚠️ **D9 l'a portée à 496 puis EXTRAITE en cinq enfants** : `eliminatoire.rs` **307**, `voisines.rs` **240**, `temoin.rs` **200**, `combinaisons.rs` **157**, `persistance.rs` **107** |
> | `client/src/fullscreen.test.ts` | **327** | neuf |
> | `agent/src/superviseur/table/tests_retention.rs` | **322** | +41 |
> | `agent/src/windows_source/sortie.rs` | ~~316~~ **313** (6 août 2026, D9) | +128 sous D8 ; D9 y laisse `borner_a_la_taille_max` **sans aucun appelant** |
> | `agent/src/windows_source/redimensionnement.rs` | ~~292~~ ~~327~~ ~~345~~ **252** (6 août 2026, D9) | +75/-15 sous D8, +53 aux deux rondes de la revue finale ; **D9 lui retire 93 lignes** — le garde du désarmement et tout l'appel au changement de mode partent avec lui |
> | `agent/src/capteur/pont_media.rs` | ~~188~~ **263** | +38 sous D8 — le bras `PleinEcran` du `match` catch-all, **et son test** (trou du plan, voir la section D8). ⚠️ Le 188 est un chiffre **D6** : D7 l'avait déjà porté à **225** sans qu'aucun de ses tableaux ne le dise |
> | `agent/src/capteur/plein_ecran.rs` | ~~195~~ ~~263~~ **212** (6 août 2026, D9) | neuf — le prédicat pur, aucun `cfg`, ~~8~~ **9** tests d'hôte. **+68 à la revue finale de D8** (`changement_de_mode_arme`), **puis −51 en D9** : le garde disparaît avec le mécanisme, et le fichier devient le **point de référence du constat de mesure** que cinq commentaires du dépôt citent |
> | `client/src/fullscreen.ts` | **174** | +64/-4 |
> | `agent/src/capteur/fenetre/commandes.rs` | ~~192~~ **214** (6 août 2026, D9) | +14/-3 sous D8 ; D9 y ajoute le bras `AudioMort` et y corrige un commentaire périmé trouvé par la **revue transverse** |
> | `proto/ts/control.ts` | **129** | +8/-2 |
> | `agent/src/superviseur/boucle/placement_periodique.rs` | **76** | +18/-2 |
>
> ⚠️ **NEUF chiffres publiés plus haut dans ce fichier ont vieilli, et ils sont
> corrigés À LEUR PLACE** (barrés dans les tableaux D6 et D7 ci-dessus), pas
> répétés ici : `superviseur/table.rs` 468 → **493**, `capteur/fenetre.rs`
> 437 → **470**, `capteur/distante/tests.rs` 462 → **474**,
> `capteur/distante.rs` 354 → **375**, `capteur/protocole.rs` 361 → **369**,
> `source.rs` 348 → **362**, `transport/tick.rs` 297 → **308**,
> `capteur/pont_media.rs` 188 → **263** (à **deux** endroits, dont le
> §« Ce que le code livre » de D6). **Sept d'entre eux sont imputables à D8
> seul** ; le huitième, `capteur/pont_media.rs`, a dérivé **en deux temps** —
> D7 l'avait porté de 188 à **225 sans qu'aucun tableau de D7 ne l'enregistre**,
> puis D8 à 263. Et son 188 dormait dans le **SOMMAIRE** de D6, l'endroit exact
> dont ce fichier écrit trois fois qu'il est le seul qu'on lise.
>
> ✅ **Chiffres voisins RELEVÉS et EXACTS ce jour-là**, à ne pas re-vérifier :
> `encode/arret.rs` **500** (marge 0), `capture.rs` ~~496~~ **492** (8),
> `superviseur/boucle.rs` **492** (8), `capteur/serveur.rs` ~~490~~ **435** (65),
> `transport/socket.rs` **481** (19), `windows_audio.rs` **479** (21),
> `transport/piste_video.rs` **477** (23), `congestion/controleur.rs` **472**
> (28), `transport/adaptation.rs` **468** (32), `demarrage.rs` ~~457~~ **464** (36),
> `wasapi.rs` **352**, `wasapi/process_loopback.rs` **402**,
> `capteur/tube.rs` **263**, `capteur/vivier.rs` **275**,
> `transport/part.rs` ~~274~~ **301**, `capteur/repartiteur.rs` **147**,
> `capteur/sommeil.rs` ~~327~~ **269**, `capteur/sommeil/parts.rs` ~~348~~ **349**,
> `capteur/audio.rs` ~~152~~ **228**, `audio.rs` **273**,
> `transport/piste_audio.rs` **218**, `demarrage/audio.rs` **77**.

> ✅ **Relance du 6 août 2026, fin du sous-bloc D9, PAR LA COMMANDE, APRÈS les
> dernières éditions de la ronde** (y compris celles de la revue transverse — une
> table relevée en début de ronde serait fausse à la fin de la même ronde, erreur
> que D8 a commise en croyant bien faire). **Le tableau de dette a toujours DEUX
> lignes, et l'une d'elles a MAIGRI** : `encode.rs` **1536** (inchangé),
> `windows_source.rs` **628** (~~638~~). **Aucun autre fichier de code source ne
> dépasse 500 lignes.**
>
> ⚠️ **MARGE LA PLUS SERRÉE DU DÉPÔT APRÈS `encode/arret.rs`, et elle s'est
> encore resserrée : `agent/src/superviseur/table.rs` est à 494 lignes, marge 6**
> (493 à la fin de D8). D9 n'y a mis qu'une ligne de commentaire, et cela a suffi.
> **Toute addition future à ce fichier appelle une extraction** — ses tests de
> rétention vivent déjà à part (`superviseur/table/tests_retention.rs`, **326**).
>
> ⚠️ **MARGE ÉTROITE NEUVE, que nul tableau ne signalait :
> `agent/src/transport/tick/tests.rs` est à 489 lignes, marge 11.** Fichier né
> avant D9, porté là par la tâche 9 (le test du verrou `audio_mort_signale`).
> Consigné comme mineur différé par la revue de cette tâche, repris ici parce que
> c'est le seul endroit qu'on lit pour connaître sa marge.
>
> **Fichiers que D9 a fait bouger, tous mesurés par la commande :**
>
> | Fichier | Lignes | Remarque |
> | --- | --- | --- |
> | ❌ `agent/src/windows_source/redimensionnement/mode_sortie.rs` | **SUPPRIMÉ** (~~458~~) | tâche 3 — le changement de mode de sortie est retiré **sur mesure**, pas désarmé |
> | `agent/src/capteur/sommeil.rs` | ~~327~~ **269** | il a franchi 500, a été ramené à **499 PAR COMPRESSION** — geste que ce fichier interdit nommément —, et la revue a exigé l'**extraction** : `sommeil/registre.rs` |
> | `agent/src/capteur/sommeil/registre.rs` | **331** | neuf — le registre lui-même, transposition vérifiée caractère pour caractère |
> | `agent/src/capteur/serveur.rs` | ~~490~~ **435** (marge 65) | tâche 6 — `serveur/instances.rs` extrait AVANT que la tâche 9 n'y ajoute quoi que ce soit. **La seule marge de 10 du dépôt est rendue** |
> | `agent/src/capteur/serveur/instances.rs` | **82** | neuf — le commentaire de `TAMPON` part **avec sa constante**, comme la règle l'exige |
> | `agent/src/capteur/fenetre.rs` | ~~470~~ **485** (marge 15) | tâche 11 : franchi 500 (508), resserré à 496, puis **extrait** `fenetre/trace.rs` sur reclassement de la revue |
> | `agent/src/capteur/fenetre/trace.rs` | **43** | neuf — le lecteur de `SOURCE_TRACE`, côté CAPTEUR cette fois |
> | `agent/src/windows_source/telemetrie.rs` | **72** | neuf — **pur, aucun `cfg`**, deux tests d'hôte. C'est lui qui rend sa marge à `windows_source.rs` |
> | `agent/src/capteur/audio.rs` | ~~152~~ **228** | le champ `inapte`, quatre tests neufs, et la réfutation du réarmement (revue transverse) |
> | `agent/src/capteur/sommeil/porteurs.rs` | **217** | +25 : la remise à zéro du compteur de réarmements, et sa réfutation (revue transverse) |
> | `agent/src/capteur/distante.rs` | ~~375~~ **400** | `signaler_audio_mort`, `rattachement_survenu` |
> | `agent/src/source.rs` | ~~362~~ **387** | deux méthodes de trait, toutes deux à défaut inerte |
> | `agent/src/transport/tick.rs` | ~~308~~ **343** | la branche a1sexies |
> | `agent/src/transport/tick/tests.rs` | **489** (marge **11**) | +118 |
> | `agent/src/capteur/protocole.rs` | ~~369~~ **381** | `VersCapteur::AudioMort` |
> | `agent/src/transport/part.rs` | ~~274~~ **301** | `PART_SONDAGE=0` |
> | `agent/src/windows_source/redimensionnement.rs` | ~~345~~ **252** | −93 : tout l'appel au changement de mode part |
> | `agent/src/capteur/plein_ecran.rs` | ~~263~~ **212** | −51 : le garde disparaît ; l'en-tête devient le **constat de mesure** que cinq commentaires citent |
> | `agent/src/windows_source/sortie.rs` | ~~316~~ **313** | `borner_a_la_taille_max` y reste, **sans aucun appelant** |
> | `agent/src/capteur/fenetre/commandes.rs` | ~~192~~ **214** | le bras `AudioMort`, et un commentaire périmé corrigé par la revue transverse |
> | `agent/src/superviseur/table.rs` | ~~493~~ **494** (marge **6**) | une ligne de commentaire, et c'est la marge la plus serrée du dépôt après `arret.rs` |
> | `agent/src/superviseur/table/tests_retention.rs` | ~~322~~ **326** | +4 |
> | `agent/src/diagnostics/multifenetre/mode_sortie.rs` | ~~421~~ **383** | la phase P : franchi 496, puis extrait en **cinq** enfants |
> | `agent/src/diagnostics/multifenetre/mode_sortie/eliminatoire.rs` | **307** | neuf |
> | `agent/src/diagnostics/multifenetre/mode_sortie/voisines.rs` | **240** | neuf |
> | `agent/src/diagnostics/multifenetre/mode_sortie/temoin.rs` | **200** | neuf |
> | `agent/src/diagnostics/multifenetre/mode_sortie/combinaisons.rs` | **157** | neuf |
> | `agent/src/diagnostics/multifenetre/mode_sortie/persistance.rs` | **107** | neuf |
> | `agent/src/survie_verdict.rs` | **61** | neuf, **pur** — ⚠️ posé à la RACINE du crate alors que le dépôt a deux précédents (`capture_reprise`, `windows_source_sortie`) qui gardent le fichier chez le parent et n'y hissent que la déclaration par `#[path]`. Déviation relevée, non corrigée |
> | `client/src/main.ts` | **352** | legs 7, 8 et 10 |
> | `client/src/resize.ts` + `resize.test.ts` | **45** + **39** | neufs, **purs, sans DOM** |
> | `agent/src/demarrage.rs` | ~~457~~ **464** | le champ `session` sur `contrôle reçu`, et le lecteur mort de `SOURCE_TRACE` retiré |
> | `agent/src/capture.rs` | ~~496~~ **492** (marge 8) | ALLÉGÉ — `DesktopCapture::cible()`, orphelin, part avec le changement de mode |
>
> ✅ **Chiffres voisins RELEVÉS et EXACTS ce jour-là**, à ne pas re-vérifier :
> `encode/arret.rs` **500** (marge 0), `superviseur/boucle.rs` **492** (8),
> `transport/socket.rs` **481** (19), `windows_audio.rs` **479** (21),
> `transport/piste_video.rs` **477** (23), `capteur/distante/tests.rs` **474**
> (26), `congestion/controleur.rs` **472** (28), `transport/adaptation.rs`
> **468** (32), `proto/src/control.rs` **419**, `wasapi/process_loopback.rs`
> **402**, `capteur/sommeil/parts.rs` **349**, `wasapi.rs` **352**,
> `capteur/sommeil/tests.rs` **378**, `capteur/vivier.rs` **275**,
> `capteur/tube.rs` **263**, `capteur/pont_media.rs` **263**,
> `audio.rs` **273**, `transport/piste_audio.rs` **229**,
> `capteur/repartiteur.rs` **147**, `demarrage/audio.rs` **77**.

**Vérifier l'état** :

```bash
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
```

## Development Commands

### Running the Application

```bash
# With Docker (recommended)
docker-compose up --build

# Local development (requires guacd daemon)
npm install
node index.js
```

The server listens on port 3445 by default.

### Environment Variables

Required environment variables (configured in docker-compose.yml or index.js):

- `WINDOWS_HOSTNAME` - Target Windows machine IP/hostname
- `WINDOWS_USERNAME` - Standard user account for RDP connections
- `WINDOWS_PASSWORD` - Password for standard user
- `WINDOWS_ADMIN_USERNAME` - Administrator account for WinRM commands
- `WINDOWS_ADMIN_PASSWORD` - Administrator password

### Dependencies

The project uses:

- `guacd` daemon (Apache Guacamole server)
- Node.js 20.x
- FUSE for filesystem operations (requires privileged container)

## Architecture

### Core Components

**index.js** - Entry point that initializes Express server and coordinates three main modules:

- `src/app.js` - Windows application discovery via WinRM
- `src/session.js` - Guacamole session and connection management
- `src/asset.js` - Static asset serving and image processing

### Session Flow

1. User navigates to `/:app` (e.g., `/excel`)
2. System generates UUID session identifier
3. Creates:
   - WebSocket tunnel for Guacamole protocol
   - Separate WebSocket for filesystem bridge
   - FUSE mount at `/mnt/ftp-{uuid}`
4. Spawns `guacd` daemon instance bound to port 4822
5. Establishes RDP connection to Windows with RemoteApp configuration
6. Browser connects via guacamole-common-js client library

### Filesystem Architecture

The system implements a bidirectional filesystem bridge between browser and Windows:

**Server side** (`src/file.js`):

- Creates FUSE mount using `fuse-native`
- Translates FUSE operations (read, write, getattr, etc.) to WebSocket messages
- Caches file data to reduce round-trips
- Mount point exposed to Windows via RDP drive sharing

**Client side** (`web/index.js`, lines 406-618):

- Uses File System Access API (`showDirectoryPicker`)
- Receives filesystem operation requests via WebSocket
- Performs local file operations and returns results
- Enables Windows applications to read/write files directly to user's browser filesystem

### Application Discovery

`src/app.js` discovers available Windows applications by:

1. Querying Windows desktop shortcuts via WinRM/PowerShell
2. Extracting icons and converting to base64 PNG
3. Determining file associations from Windows registry
4. Generating metadata (name, color theme, MIME handlers)
5. Refreshing application list hourly

### Frontend Build Process

`src/asset.js` uses Gulp to transpile and bundle:

- `web/index.js` → `dist/app.js` (main client application)
- `web/home.js` → `dist/home.js` (home page)
- Uses Babel for ES6+ → ES5 transpilation
- Uses Browserify to bundle CommonJS modules for browser

### Guacamole Integration

Connection established via `guacamole-lite` wrapper:

- Encryption: AES-256-CBC with session UUID as key
- Protocol: RDP with RemoteApp mode
- Audio: Bidirectional audio support
- Clipboard: Synchronized clipboard via WebSocket streaming
- Input: Mouse, touch, and keyboard event forwarding

### PWA Features

The application supports Progressive Web App installation:

- Dynamic manifest generation per application (`/:app/manifest.json`)
- Service worker registration (`web/sw.js`)
- File handler associations for opening files in installed apps
- Window controls overlay for native-like window chrome
- Theme color extraction from application canvas

## Key Technical Details

### RDP Configuration

Session settings in `src/session.js` (lines 126-159):

- Client name set to UUID to ensure unique sessions
- Drive sharing enabled, pointing to FUSE mount
- Display update resize method for dynamic resolution
- Font smoothing and touch enabled
- Upload/download disabled (filesystem bridge used instead)
- High DPI support (500 DPI default)

### Security Considerations

- Credentials are stored in environment variables and hardcoded in index.js
- AES-256-CBC encryption for Guacamole protocol
- WebSocket connections should be served over WSS in production
- Container requires privileged mode for FUSE operations

### Client-Side State Management

`web/index.js` manages:

- Display scaling and resize handling
- Clipboard synchronization (bidirectional)
- Title bar dragging for PWA window mode
- Theme color extraction from canvas pixels
- Audio context initialization on user interaction

### Cleanup and Resource Management

`src/cleanup.js` (referenced but not examined) handles:

- Graceful guacd process termination
- FUSE unmounting via `fusermount -u`
- WebSocket connection cleanup
- Session instance removal from registry

## Common Development Patterns

### Adding New Remote Application Support

Applications are auto-discovered from Windows desktop shortcuts. To add support:

1. Place `.lnk` shortcut on Windows user desktop
2. Application will appear in list after next refresh cycle
3. File associations automatically extracted from Windows registry

### Modifying Frontend Client

1. Edit source in `web/index.js`
2. Gulp watches and rebuilds to `dist/app.js` automatically
3. Changes require browser refresh (no hot reload)

### Testing WinRM Connection

The repository includes test files:

- `test_winrm_nodejs.js` - NodeJS WinRM testing
- `test_winrm_fixed.js` - Fixed WinRM implementation tests

## Known Constraints

- Only supports single guacd instance on port 4822
- Requires Windows machine with:
  - RDP RemoteApp capability
  - WinRM enabled (port 5985)
  - Desktop shortcuts for discoverable applications
- FUSE operations require Linux host with kernel support
- File System Access API requires modern Chromium-based browser
- No horizontal scaling due to in-memory session storage

---

## 🔧 Problèmes Résolus (Session du 21 Oct 2025)

### 1. Icônes Manquantes - PowerShell Extraction Failed

**Symptôme**: Aucune icône ne s'affichait, logs montrant "PowerShell extraction returned FAILED"

**Cause Racine**: Script PowerShell complexe utilisant `Add-Type -TypeDefinition` avec code C# pour `ExtractIconEx` ne fonctionnait pas correctement

**Solution Implémentée** (`src/iconExtractor.js` lignes 165-178):

```javascript
const script = `
if (-not (Test-Path 'C:\\temp')) { New-Item -ItemType Directory -Path 'C:\\temp' | Out-Null }
Add-Type -AssemblyName System.Drawing
$icon = [System.Drawing.Icon]::ExtractAssociatedIcon('${windowsPath}')
if ($icon) {
    $bitmap = $icon.ToBitmap()
    $bitmap.Save('${tempFile}', [System.Drawing.Imaging.ImageFormat]::Png)
    $bitmap.Dispose()
    $icon.Dispose()
    echo 'SUCCESS'
} else {
    echo 'FAILED'
}
`.replace(/\n/g, "; ");
```

**Compromis**: Les icônes sont maintenant extraites en basse résolution (32x32 ou 48x48) au lieu de haute résolution (256x256+)

**Compensation**: Taille d'affichage réduite à 48px sur la page d'accueil (`assets/index.html` lignes 64-65) pour masquer la pixelisation

**Statut**: ✅ RÉSOLU - Les icônes s'affichent correctement

---

### 2. Sessions RDP Se Fermant Immédiatement (Après 2 Secondes)

**Symptôme**: Les sessions s'ouvraient puis se fermaient automatiquement après exactement 2 secondes

**Cause Racine**:

1. Le WebSocket filesystem tentait de se connecter au client browser
2. Si la connexion échouait ou était lente, un heartbeat check toutes les 2 secondes détectait le problème
3. Le handler d'erreur du WebSocket appelait `window.close()` immédiatement
4. Les handlers du tunnel Guacamole appelaient aussi `window.close()` sur toute erreur

**Solutions Multiples Implémentées** (`web/index.js`):

**A) Désactivation fermeture sur erreur filesystem** (lignes 426-433):

```javascript
ws.onclose = function () {
  console.log("Filesystem WebSocket closed (filesystem features disabled)");
  // NE PLUS fermer la fenêtre
};

ws.onerror = function (error) {
  console.log(
    "Filesystem WebSocket error:",
    error,
    "(filesystem features disabled)"
  );
  // NE PLUS fermer la fenêtre
};
```

**B) Heartbeat interval augmenté** (ligne 451):

```javascript
// Changé de 2000ms à 30000ms
setInterval(() => {
  if (ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ heartbeat: true }));
  }
}, 30000);
```

**C) Tunnel handlers moins agressifs** (lignes 117-122):

```javascript
if (state === Guacamole.Tunnel.State.CLOSED) {
  console.log("Tunnel closed - will close window in 1 second");
  setTimeout(function () {
    window.close();
  }, 1000); // Délai pour éviter fermeture prématurée
}
```

**Impact**: Les sessions restent maintenant ouvertes même si le filesystem WebSocket échoue. Les features de partage de fichiers sont simplement désactivées silencieusement.

**Statut**: ✅ RÉSOLU - Les sessions restent ouvertes correctement

---

### 3. Fenêtre Ne Se Fermant Pas Après Déconnexion

**Symptôme**: Après avoir fermé l'application RDP côté Windows, la fenêtre du navigateur restait ouverte indéfiniment

**Cause**: Tous les handlers de fermeture automatique avaient été désactivés pour résoudre le problème #2

**Solution** (`web/index.js` lignes 117-122):

- Ajout d'un timer de 1 seconde avant fermeture
- Fermeture uniquement quand le tunnel Guacamole passe à l'état `CLOSED`
- Ignore les états `UNSTABLE` pour permettre la reconnexion

**Statut**: ✅ RÉSOLU - La fenêtre se ferme correctement après déconnexion

---

### 4. Design Obsolète de la Page d'Accueil

**Problème**: Design basique avec fond bleu foncé, cards simples sans animations

**Améliorations Implémentées** (`assets/index.html` lignes 26-134):

1. **Fond moderne**: Dégradé violet/bleu (`linear-gradient(135deg, #667eea 0%, #764ba2 100%)`)
2. **Effet Glassmorphism**: Cards blanches semi-transparentes avec backdrop-filter blur
3. **Grid CSS Responsive**: Auto-adaptatif selon la largeur d'écran
4. **Animations**: Effet de levée au survol, scale sur les boutons
5. **Typographie**: Police système moderne (-apple-system, Segoe UI, etc.)
6. **Ombres douces**: `box-shadow: 0 8px 32px rgba(0, 0, 0, 0.1)`
7. **Icônes optimisées**: Taille fixe 48x48px avec `object-fit: contain`

**Statut**: ✅ RÉSOLU - Design moderne et professionnel

---

### 5. Bouton Install/Uninstall Non Fonctionnel

**Symptôme**: `navigator.getInstalledRelatedApps()` retourne toujours un tableau vide

**Cause**: Cette API ne fonctionne que pour les apps cross-origin avec `related_applications` dans le manifest, pas pour les PWA du même domaine

**Solution** (`web/home.js` lignes 3-23):

- Tracking manuel via `localStorage`
- Fonctions helper: `isAppInstalled()`, `markAppInstalled()`, `markAppUninstalled()`
- Bouton affiche "Install" ou "Uninstall" selon l'état stocké
- Instructions affichées pour guider l'utilisateur vers l'installation native du navigateur

**Limitation Connue**: Si l'utilisateur installe via le menu du navigateur directement, le bouton ne changera pas automatiquement (nécessite clic sur Install puis Uninstall pour sync)

**Statut**: ⚠️ PARTIELLEMENT RÉSOLU - Fonctionne mais nécessite recompilation JavaScript (voir section Build)

---

## 🐛 Problèmes Connus Non Résolus

### 1. Crash "double free or corruption"

**Log Type**: `guacd[63]: ERROR: double free or corruption (out)`

**Contexte**:

- Se produit sporadiquement après ~30-60 secondes
- Souvent corrélé avec un accès au filesystem FUSE
- Log précédent: `getattr / { mode: 16877, size: 1000000000 }`

**Cause Probable**:

- Race condition entre fermeture du WebSocket filesystem et opération FUSE en cours
- Guacd tente d'accéder à la mémoire du drive FUSE pendant qu'il se démonte
- Double libération de mémoire dans le code natif de `fuse-native` ou `guacd`

**Impact**:

- Ferme la session RDP prématurément
- Message d'erreur: "RDP server closed/refused connection: Manually disconnected"
- L'utilisateur perd son travail si non sauvegardé

**Mitigations Actuelles**:

- Filesystem WebSocket ne ferme plus la fenêtre en cas d'erreur
- Caching des opérations filesystem pour réduire les appels

**TODO**:

1. Ajouter une gestion propre de shutdown du filesystem
2. Attendre que toutes les opérations FUSE soient terminées avant démontage
3. Investiguer les logs de guacd avec niveau DEBUG
4. Considérer alternative à FUSE (WebDAV? SFTP?)

**Workaround Utilisateur**: Si le crash se produit, simplement relancer l'application

---

### 2. Qualité Basse Résolution des Icônes

**Problème**:

- Icônes extraites en 32x32 ou 48x48 seulement
- Upscalées à 512x512 par Sharp (algorithme Lanczos3)
- Résultat pixelisé visible si affiché en grand

**Cause**:

- `System.Drawing.Icon.ExtractAssociatedIcon()` retourne seulement les petites icônes
- Les méthodes haute résolution (icotool, wrestool, sharp-ico) échouent avec les .exe modernes

**Mitigation Actuelle**:

- Affichage à 48px sur la page d'accueil (1:1 ou 1.5x upscale seulement)
- Acceptable pour les icônes système

**Solutions Possibles**:

1. Réimplémenter extraction via PowerShell avec `[System.IconExtractor]` et P/Invoke vers Shell32.dll
2. Utiliser un outil Windows natif (ResourceHacker, RCEdit) appelé via WinRM
3. Extraire directement depuis les ressources PE avec lecteur binaire custom
4. Pre-générer icônes haute résolution côté Windows et les stocker

**Priorité**: MOYENNE - Fonctionnel mais pas idéal

---

### 3. Détection PWA Installées Incomplète

**Problème**:

- Le système ne détecte pas automatiquement si l'utilisateur a installé l'app via le menu du navigateur
- Nécessite que l'utilisateur clique sur "Install" depuis la page d'accueil

**Cause**:

- `navigator.getInstalledRelatedApps()` ne fonctionne pas pour PWA du même domaine
- Pas d'événement JavaScript natif quand une PWA est installée depuis le menu browser
- LocalStorage utilisé comme workaround mais peut se désynchroniser

**Impact**:

- Confusion utilisateur: app installée mais bouton dit "Install"
- Pas de problème fonctionnel, juste UX sous-optimal

**Solutions Possibles**:

1. Ajouter `related_applications` dans manifest avec URL externe (mais nécessite domaine différent)
2. Utiliser Service Worker pour détecter l'installation (événement `appinstalled`)
3. Vérifier `window.matchMedia('(display-mode: standalone)')` régulièrement
4. Accepter la limitation et documenter le comportement

**Priorité**: BASSE - Cosmétique seulement

---

### 4. Recompilation JavaScript Manuelle

**Problème**:

- Modifications de `web/home.js` et `web/index.js` ne sont PAS recompilées automatiquement
- Nodemon surveille seulement `index.js` et `src/*.js`
- Build Gulp s'exécute uniquement au démarrage de l'application

**Impact**:

- Les changements JavaScript client nécessitent redémarrage Docker complet
- Ralentit le développement frontend
- Risque d'oublier de recompiler et tester ancien code

**Workaround Actuel**:

```bash
docker-compose restart web
```

**Solutions Possibles**:

1. Ajouter un watcher Gulp séparé pour `web/*.js`
2. Modifier `nodemon.json` pour inclure `web/` dans les fichiers surveillés
3. Migrer vers Webpack avec hot module replacement
4. Script de développement séparé avec `gulp watch`

**Configuration Actuelle** (`src/asset.js` lignes 19-37):

```javascript
gulp
  .src(__dirname + "/../web/index.js")
  .pipe(babel({ presets: ["@babel/env"] }))
  .pipe(browserify({ insertGlobals: true, debug: true }))
  .pipe(gulp.dest(__dirname + "/../dist"));
```

**TODO**: Ajouter système de watch pour développement plus fluide

**Priorité**: MOYENNE - Impact développement

---

## 📦 Système de Build et Compilation

### Pipeline de Compilation

**Source**: `src/asset.js` lignes 19-37

**Processus**:

```
web/index.js  → Babel (preset: @babel/env) → Browserify → dist/index.js
web/home.js   → Babel (preset: @babel/env) → Browserify → dist/home.js
```

### Déclenchement

- ✅ **Automatique**: Au démarrage de l'application Node.js
- ❌ **Pas de watch**: Modifications ne déclenchent pas rebuild
- ❌ **Nodemon ignore web/**: Surveille uniquement `index.js` et `src/`

### Forcer la Recompilation

```bash
# Méthode 1: Redémarrer le conteneur Docker
docker-compose restart web

# Méthode 2: Redémarrer complètement
docker-compose down
docker-compose up -d

# Méthode 3: Touch index.js pour déclencher nodemon (NE FONCTIONNE PAS pour web/*)
touch index.js
```

### Vérifier la Compilation

```bash
# Voir date de dernière modification
ls -lah dist/home.js dist/index.js

# Vérifier taille des fichiers
du -h dist/*.js
```

---

## 🔐 Configuration Critique

### Ports Exposés

| Port | Service      | Description                           |
| ---- | ------------ | ------------------------------------- |
| 3445 | Express HTTP | Serveur web principal                 |
| 4822 | guacd        | Daemon Guacamole (RDP proxy)          |
| 5985 | WinRM        | Communication PowerShell vers Windows |
| 3389 | RDP          | Port RDP Windows (implicite)          |

### Paths Importants

| Path                                 | Description                       |
| ------------------------------------ | --------------------------------- |
| `/media/vm/`                         | Mount point du filesystem Windows |
| `/media/vm/Users/{username}/Desktop` | Source des .lnk shortcuts         |
| `/mnt/ftp-{uuid}`                    | FUSE mount par session            |
| `/opt/server`                        | Workdir dans container Docker     |
| `/home/mallanic/Projects/Guacamole`  | Root projet sur host              |

### Credentials et Secrets

⚠️ **SÉCURITÉ CRITIQUE** ⚠️

**Fichiers contenant des credentials en clair**:

1. `index.js` lignes 2-4
2. `docker-compose.yml` lignes 14-17

**Variables**:

- `WINDOWS_HOSTNAME`: 192.168.3.2
- `WINDOWS_ADMIN_USERNAME`: Administrator
- `WINDOWS_ADMIN_PASSWORD`: [MASQUÉ - voir fichiers]
- `WINDOWS_USERNAME`: guacamole
- `WINDOWS_PASSWORD`: [MASQUÉ - voir fichiers]

**TODO PRODUCTION**:

1. ⚠️ Externaliser dans fichier .env (ne jamais commit)
2. ⚠️ Utiliser Docker secrets ou variables d'environnement externes
3. ⚠️ Chiffrer les secrets avec Vault, AWS Secrets Manager, etc.
4. ⚠️ Rotation régulière des mots de passe
5. ⚠️ Audit trail des accès

---

## 📝 Parser de Fichiers .lnk Windows

### Spécification

Implémente la spécification **MS-SHLLINK** (Shell Link Binary File Format)

**Fichier**: `src/lnkParser.js`

### Structure Parsée

1. **ShellLinkHeader** (76 bytes / 0x4C)

   - Magic number: 0x4C (validatio)
   - LinkFlags (offset 0x14)

2. **LinkTargetIDList** (optionnel)

   - Taille variable selon flags
   - Skippé mais taille lue

3. **LinkInfo** (optionnel si flag 0x02)

   - Contient le chemin local (`LocalBasePath`)
   - Ou chemin réseau (`NetworkShareName`)
   - **Problème connu**: Parfois retourne juste "C:\\" incomplet

4. **StringData** (plusieurs sections)
   - NAME_STRING (flag 0x04)
   - RELATIVE_PATH (flag 0x08) ← **Utilisé comme fallback**
   - WORKING_DIR (flag 0x10)
   - COMMAND_LINE_ARGUMENTS (flag 0x20)
   - ICON_LOCATION (flag 0x40) ← **Important pour icônes**

### Données Extraites

```javascript
{
  targetPath: "C:\\Program Files\\Mozilla Firefox\\firefox.exe",
  iconPath: "C:\\Program Files\\Mozilla Firefox\\firefox.exe",
  iconIndex: 0
}
```

### Cas Spéciaux Gérés

1. **Chemin incomplet dans LinkInfo**: Utilise RELATIVE_PATH comme fallback
2. **Chemins relatifs** (ex: `..\..\AppData\Local\...`): Convertis en absolus
3. **Format IconLocation**: Parse "path,index" (ex: "notepad.exe,1")
4. **Variables d'environnement**: Substitution basique

### Conversion de Chemins

**Fonction**: `convertToLinuxPath(windowsPath, lnkPath)`

```javascript
// Exemples de conversion:
"C:\\Program Files\\Firefox\\firefox.exe"
  → "/media/vm/Program Files/Firefox/firefox.exe"

"..\\AppData\\Local\\App\\app.exe" (avec contexte utilisateur)
  → "/media/vm/Users/guacamole/AppData/Local/App/app.exe"

"%ProgramFiles%\\App\\app.exe"
  → "/media/vm/Program Files/App/app.exe"
```

---

## 🎨 Extraction d'Icônes - Détails Techniques

### Méthodes Tentées (Ordre de Priorité)

**Fichier**: `src/iconExtractor.js`

#### 1. Fichier .ico Standalone

```javascript
// Cherche {exe}.ico
const icoPath = exePath.replace(/\.exe$/i, ".ico");
// Cherche aussi: icon.ico, app.ico, logo.ico, icon_256.ico
```

**Taux de succès**: ~5% (rare)

#### 2. icotool (icoutils)

```bash
icotool -x -i 1 application.exe
```

**Problème**: Erreur "reserved non-zero" sur .exe modernes
**Taux de succès**: ~10%

#### 3. wrestool (icoutils)

```bash
wrestool -l -t 14 application.exe  # List icon groups
wrestool -x -t 14 -n ICON_NAME application.exe
```

**Problème**: Erreur "No icons found" sur beaucoup d'exécutables
**Taux de succès**: ~15%

#### 4. sharp-ico (Direct PE Reading)

```javascript
const icons = await decode(exeBuffer);
```

**Problème**: Erreur "Invalid magic bytes" - ne supporte pas PE modernes
**Taux de succès**: ~5%

#### 5. PowerShell (MÉTHODE RETENUE) ✅

```powershell
Add-Type -AssemblyName System.Drawing
$icon = [System.Drawing.Icon]::ExtractAssociatedIcon('C:\path\to\app.exe')
$bitmap = $icon.ToBitmap()
$bitmap.Save('C:\temp\icon.png', [System.Drawing.Imaging.ImageFormat]::Png)
```

**Avantages**:

- Fonctionne toujours (100% succès)
- API Windows native
- Pas de dépendances externes

**Limitations**:

- Retourne seulement les petites icônes (32x32 ou 48x48)
- Pas d'accès aux grandes versions (256x256+) présentes dans l'exe

**Taux de succès**: **100%** ✅

### Post-Processing

Après extraction, toutes les icônes subissent:

```javascript
const resized = await sharp(iconBuffer)
  .resize(512, 512, {
    fit: "contain",
    background: { r: 0, g: 0, b: 0, alpha: 0 },
    kernel: sharp.kernel.lanczos3,
  })
  .png()
  .toBuffer();
```

**Algorithme**: Lanczos3 (meilleur qualité pour upscaling)
**Taille cible**: 512x512 (pour manifest PWA)
**Affichage réel**: 48x48 (page d'accueil)

### Extraction de Couleur

**Librairie**: ColorThief

```javascript
const color = await ColorThief.getColor(image);
// Retourne: [R, G, B]  ex: [251, 213, 64]
```

**Utilisé pour**:

- `theme-color` dans manifest
- `background-color` dans manifest
- Couleur de fond dans app.tpl.html

---

## 🔄 Flux de Lancement d'Application Complet

### 1. Découverte (Startup + Hourly)

```javascript
// src/app.js ligne 52
async function fetchApps() {
  const desktopPath = `/media/vm/Users/${process.env.WINDOWS_USERNAME}/Desktop`;
  const files = await fs.readdir(desktopPath);
  const lnks = files.filter((f) => f.endsWith(".lnk"));

  for (const lnk of lnks) {
    const { targetPath, iconPath, iconIndex } = await parseLnk(lnkPath);
    const iconBase64 = await extractIcon(finalIconPath, iconIndex, 512);
    const color = await ColorThief.getColor(image);
    const associations = await getAppFileAssociations(name);

    apps.push({ name, short, program, color, icon, fileHandlers });
  }
}

// Refresh toutes les heures
setInterval(update, 1000 * 60 * 60);
```

### 2. Affichage Liste (GET /)

```javascript
// assets/index.html chargé
// web/home.js récupère la liste
fetch("/apps")
  .then((res) => res.json())
  .then((apps) => apps.forEach((app) => addApp(app)));
```

### 3. Clic Launch (GET /:app)

```javascript
// src/session.js ligne 99
app.get("/:app", async (req, res) => {
  const guacdIndex = uuid.v4().slice(0, 32);

  // Créer WebSocket Server pour filesystem
  const ws = new WebSocketServer({ noServer: true });
  server.on("upgrade", upgradeHandler);

  // Créer FUSE mount
  const fileSystem = await createFileSystem(guacdIndex, ws);

  // Configurer Guacamole
  const guacServer = new GuacamoleLite(
    {
      server,
      noServer: true,
      path: `/${app.short}/${guacdIndex}`,
    },
    guacdOptions,
    clientOptions,
    callbackOptions
  );

  // Stocker la session
  guacdInstances[guacdIndex] = { port, guacServer, ws };

  // Rediriger vers la session
  res.redirect(`/${app.short}/${guacdIndex}`);
});
```

### 4. Ouverture Session (GET /:app/:uuid)

```html
<!-- assets/app.tpl.html chargé -->
<!-- web/index.js (compilé → dist/index.js) exécuté -->
```

```javascript
// web/index.js ligne 111
const tunnel = new Guacamole.WebSocketTunnel(`/${appName}/${sessionID}`);
tunnel.setUUID(sessionID);

const guac = new Guacamole.Client(tunnel);

const token = await encrypt(
  {
    connection: {
      type: "rdp",
      settings: {
        dpi: getDPI() + "",
        "color-depth": 24 + "",
      },
    },
  },
  clientOptions
);

guac.connect(
  "token=" + token + "&height=..." + "&width=..." + "&GUAC_AUDIO=audio/L16"
);
```

### 5. Connexion RDP

```
Browser WebSocket → guacamole-lite → guacd (port 4822) → RDP (192.168.3.2:3389)
```

**Configuration RDP envoyée**:

- Type: rdp
- RemoteApp: C:\Program Files\Mozilla Firefox\firefox.exe
- Drive: /mnt/ftp-{uuid}
- Audio: bidirectionnel
- Clipboard: sync via Guacamole
- Resize: display-update

### 6. Session Active

**Inputs**:

- Souris → `Guacamole.Mouse` → `guac.sendMouseState()`
- Touch → `Guacamole.Mouse.Touchscreen` → `guac.sendMouseState()`
- Clavier → `Guacamole.Keyboard` → `guac.sendKeyEvent()`

**Outputs**:

- Display → `guac.getDisplay()` → Canvas HTML
- Audio → `guac.onaudio` → Web Audio API
- Clipboard → `guac.onclipboard` → Clipboard API

**Filesystem**:

- Windows read/write → FUSE → WebSocket → Browser → File System Access API

### 7. Fermeture

```javascript
// Quand l'app Windows se ferme:
guacd[63]: INFO: RDP session closed

// Tunnel passe à CLOSED
tunnel.onstatechange → state === Guacamole.Tunnel.State.CLOSED

// Fermeture après 1 seconde
setTimeout(() => window.close(), 1000);

// Cleanup serveur
guacServer.on('close', () => {
  guacServer.close();
  ws.close();
  server.removeListener('upgrade', upgradeHandler);
  delete guacdInstances[guacdIndex];
});
```

---

## 🖥️ Cycle de vie de la VM Windows

**La VM cible est une machine libvirt/QEMU nommée `Windows`, et elle n'est pas
démarrée automatiquement.** Tout travail touchant l'agent Rust, la capture, la
recette WebRTC ou WinRM exige qu'elle tourne. Symptômes d'une VM éteinte :
`192.168.3.2` injoignable (« Aucun chemin d'accès pour atteindre l'hôte
cible ») et `/media/vm/` vide.

```bash
# État
virsh list --all            # « fermé » = éteinte, « en cours d'exécution » = démarrée

# Démarrer, puis attendre que WinRM réponde (~5 à 60 s)
virsh start Windows
until timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null; do sleep 5; done

# Puis attendre que le partage de fichiers soit monté : /media/vm se monte
# après que WinRM répond, pas en même temps — un `scripts/build-agent.sh`
# lancé dès que le port 5985 répond échoue avec « erreur : /media/vm n'est
# pas monté » (`scripts/sync-agent.sh`, qui teste le montage, pas le port).
# `mountpoint -q` ne suffit pas : /media/vm est un montage CIFS
# (//192.168.3.2/c) dont l'entrée persiste dans la table de montage même VM
# éteinte et connexion morte — il faut éprouver un ACCÈS réel, pas la seule
# présence de l'entrée.
until ls /media/vm/dev >/dev/null 2>&1; do sleep 5; done

# Arrêter proprement
virsh shutdown Windows
```

Une fois la VM démarrée, l'outillage habituel redevient disponible :

```bash
set -a && source .env && set +a      # charge les identifiants (fichier gitignoré)
node scripts/winrm.js '<commande PowerShell>'
scripts/build-agent.sh               # synchronise puis compile SUR la VM
scripts/run-agent.sh                 # lance l'agent en session interactive
```

**Ne jamais supposer la VM allumée.** Vérifier `virsh list --all` avant toute
séquence qui en dépend, et la démarrer si besoin — c'est une opération sûre et
idempotente.

⚠️ **`scripts/build-agent.sh` lancé sans avoir sourcé `.env` s'arrête EN
SILENCE**, après sa ligne « sources synchronisées », sans message ni statut
d'erreur : son `set -euo pipefail` avorte sur l'affectation du quota WinRM, dont
le `2>/dev/null` mange la cause. **Le symptôme se lit exactement comme une
compilation réussie et muette** — on mesure alors le binaire précédent sans
qu'aucune trace ne le dise. Relevé le 2 août 2026, revue finale du sous-bloc D2.
Sourcer `.env` d'abord, et se méfier d'un build qui ne dit rien.

---

## 🔊 Audio de la VM — état constaté (28 juillet 2026)

Relevé au moment de cadrer le chantier A, pour lever le risque « la VM n'a
peut-être aucun périphérique de rendu audio, donc WASAPI loopback ne produirait
rien ».

**Le risque est levé : deux endpoints de rendu sont actifs.**

| Périphérique | État |
| --- | --- |
| Haut-parleurs (Steam Streaming Speakers) | `OK` — actif, **virtuel**, aucun matériel requis |
| HDP-V104 (NVIDIA High Definition Audio) | `OK` — actif |
| NVIDIA Virtual Audio Device (Wave Extensible) (WDM) | pilote présent |
| « Sortie audio de l'ordinateur distant » (×11) | `Unknown` — endpoints RDP résiduels de sessions mortes |

`Audiosrv` tourne, démarrage `Automatic`. **Aucun pilote audio virtuel
supplémentaire n'est à installer.** Reste à confirmer par l'API réelle
(`IMMDeviceEnumerator::GetDefaultAudioEndpoint`) lequel est le périphérique par
défaut de la session interactive.

Commande de relevé :

```bash
node scripts/winrm.js "(Get-Service Audiosrv | Format-List Name,Status,StartType | Out-String); \
  Get-PnpDevice -Class AudioEndpoint | Select-Object FriendlyName,Status | Format-List | Out-String"
```

### Relevé de la sonde loopback (tâche 5, 28 juillet 2026)

Sonde `AUDIO_PROBE` (`agent/src/wasapi.rs` + `agent/src/diagnostics/audio.rs`), exécutée en
session interactive via `scripts/run-agent.sh` (task planifiée `/it`), sur le
périphérique de rendu par défaut de **cette session** (pas la session 0 de
WinRM).

- **Format de mixage exact** : `48000 Hz, 2 canaux, 32 bits, flottant`
  (`WAVE_FORMAT_EXTENSIBLE` / `KSDATAFORMAT_SUBTYPE_IEEE_FLOAT`). Compatible
  avec `SAMPLE_RATE_HZ = 48_000` de `agent/src/opus.rs` : aucun
  rééchantillonneur nécessaire.
- **Crête au repos** (10 s, rien ne joue) : `crete = 0`, `silencieux = true`,
  `lectures_vides = 1894`. Normal.
- **Crête avec son** :
  - Avec `[Console]::Beep(880, 400)` (commande exacte du brief) : `crete = 0`
    à nouveau, sur 20 s, malgré 10 bips. **Ceci n'est PAS le cas redouté**
    (périphérique par défaut détourné) : `[Console]::Beep` passe par l'ancienne
    API `kernel32!Beep` (haut-parleur PC), qui sur cette VM ne traverse pas le
    périphérique de rendu par défaut — c'est un artefact de méthode de test,
    pas un signal sur le pipeline audio.
  - Avec `(New-Object Media.SoundPlayer '...Windows Ding.wav').PlaySync()`
    (API multimédia standard, la même famille que ce que joueraient de vraies
    applications) : `crete = 2385`, `silencieux = false`,
    `echantillons = 424320` sur 15 s. **Le loopback capte bien ce que jouent
    les applications** — sonde n°2 répondue positivement.
- **Conclusion** : l'hypothèse « Steam Streaming Speakers détourné » n'est
  **pas** confirmée. Le format de mixage est directement compatible (48 kHz).
  Pour de futurs tests manuels sur cette VM, préférer `SoundPlayer` (ou toute
  API multimédia réelle) à `[Console]::Beep`, qui donne un faux négatif.

---

## 📡 Chantier C volet 1 — asservissement réseau (fusionné le 29 juillet 2026)

Commit de fusion `620bae5`. Recette complète :
`docs/superpowers/plans/2026-07-29-reseau-adaptatif-resultats.md`.

Le débit et la résolution d'encodage suivent ce que le lien porte. Prouvé sous
dégradation `netem` : estimation de 2,5 Mb/s à 148 kb/s, descente au barreau
plancher, navigateur recevant du 382×242 — vérifié des deux côtés.

### Le FEC audio et le mode faible latence sont mutuellement exclusifs

**À connaître avant de toucher à `agent/src/opus.rs`.** `RESTRICTED_LOWDELAY`
force `MODE_CELT_ONLY` (`opus_encoder.c:1349`), où `decide_fec` retourne 0 sans
condition (`opus_encoder.c:721`) : la redondance LBRR n'existe que dans SILK.
Le FEC activé au chantier A n'a donc **jamais rien émis** jusqu'à ce chantier.

L'application est désormais `Application::Audio` — arbitrage assumé de 4 ms de
pré-délai (120 → 312 échantillons) contre la résilience, l'audio restant
largement en avance sur les ~48 ms de latence vidéo. Ne pas revenir à
`LowDelay` sans rouvrir cet arbitrage.

**Piège de mesure** : à débit cible fixe, LBRR *redistribue* les octets, il ne
s'y *ajoute* pas. Vérifier le FEC par une taille de paquet est un test invalide.
La preuve valable est un décodage sur décodeur **neuf sans historique** :
`decode(paquet, sortie, fec=true)` rend 0,00 d'énergie sans perte déclarée
contre 585,02 avec.

### Banc de dégradation réseau

`scripts/netem.sh <lan|adsl|4g|congestionné|effondrement|off>`, sur
**`internalBridge`** (le pont de la VM ; aucun réseau libvirt n'est défini, il
est géré à la main). Pose la dégradation dans les **deux sens** — le sens
agent→navigateur arrive en entrée et exige une redirection `ifb` via
`tc mirred`. `lan` et `off` sont deux profils distincts à dessein : témoin de
recette contre retrait du banc. **Toujours reposer `off` en fin de mesure.**

### Angle mort de méthode, à corriger dans les recettes suivantes

Le banc tournait sur une fenêtre de **764×484** alors que le produit vise le
plein écran. Cet écart a produit à lui seul **deux des trois trouvailles de la
revue finale** : à cette taille le barreau plein n'exige que 1,11 Mb/s, franchi
d'emblée par l'amorçage du BWE, ce qui masquait une fausse alerte « Image
réduite par le réseau » qui se déclenche sur toute source ≥ 1080p. **Fixer une
résolution représentative au canevas de recette, et y exercer au moins un
redimensionnement de fenêtre.**

### Deux réserves connues, closes par le chantier de découpage des fichiers

Les deux réserves ci-dessous, ouvertes lors du chantier C volet 1, sont
**fermées** : le chantier de découpage de `transport.rs` (fichiers >500
lignes) a ajouté exactement deux tests à cet effet, au titre d'une exception
accordée pour cela — un chantier de découpage n'ajoute normalement pas de
comportement neuf, celui-ci ferme une dette de test constatée au passage.

- Les transitions de `Adaptation` (`Active` → `Indisponible`) et l'expiration
  de l'estimation BWE à 5 s sont désormais couvertes par
  `une_estimation_perimee_bascule_l_adaptation_en_indisponible_et_l_annonce`
  (`agent/src/transport/adaptation.rs`). Le test fait vieillir une estimation
  au-delà d'`EXPIRATION_ESTIMATION`, vérifie le basculement en
  `Adaptation::Indisponible`, que l'absence n'est journalisée qu'une fois, et
  que la décision est relayée au navigateur sous forme de message `Link`
  (`AgentControl::Link { adaptation: LinkAdaptation::Indisponible, .. }`) via
  `act_on_timeout` — puis qu'une estimation fraîche qui revient réarme
  l'annonce.
- Le câblage de `resize` (branche `a1` de `act_on_timeout`, désormais en
  `agent/src/transport/tick.rs`, qui délègue à
  `agent/src/transport/redimensionnement.rs`, laquelle appelle
  `Controleur::changer_source`) est désormais couvert par
  `un_redimensionnement_recalibre_le_controleur_sur_la_taille_obtenue`
  (`agent/src/transport/redimensionnement.rs`). Le test pose une source
  factice dont `resize` réussit mais impose un alignement pair (comme une
  vraie fenêtre Windows), pose un `pending_resize` sur une taille impaire, et
  vérifie que la session retient les dimensions RÉELLEMENT obtenues (pas
  celles demandées), que `encode_size_appliquee` suit cette taille obtenue, et
  qu'un refus de taille antérieur (`taille_refus_signalee`) est effacé par ce
  redimensionnement.

### Ce que le chantier D (multi-fenêtres) devra régler

> ❌ **CETTE DETTE EST DÉCRITE POUR UNE ARCHITECTURE QUI N'EXISTE PLUS, et le
> paragraphe ci-dessous n'est plus une consigne exécutable (annoté le 3 août
> 2026, sous-bloc D6).** Il suppose **N pistes dans UNE session**, donc une
> `PeerConnection` unique dont l'estimation de session doit être découpée. **Le
> produit a N SESSIONS** depuis D1 : chaque fenêtre est un processus enfant avec
> sa propre `PeerConnection`, donc son propre BWE, son propre `audio_bps` et son
> propre `video_mid`. **Il n'y a donc rien à découper d'une estimation
> partagée** — le problème réel est l'inverse : N estimateurs indépendants qui
> sondent chacun vers le lien entier.
>
> **Ce que D6 a fait à la place** : un **budget de session** (`BUDGET_BPS`, lu
> par le capteur) découpé en **parts** poussées à chaque enfant, qui les applique
> à son `Controleur` **et** à son `set_desired_bitrate`. La couche de répartition
> existe donc bien, mais elle répartit un budget **posé**, pas une capacité
> **mesurée** — et c'est délibéré, parce que la mesure a montré que le lien n'est
> pas le goulot (voir « Sous-bloc D6 »).
>
> ⚠️ **Les deux dettes voisines ne sont ni réglées ni caduques : elles sont
> DÉPLACÉES.** `audio_bps` n'est plus « un budget unique de session à retirer une
> fois » puisqu'il n'y a plus de session unique — mais **l'audio par fenêtre est
> le sujet de D7**, et rien n'a encore été fait de ce côté. Le filtre de
> `MediaEgressStats` sur un `video_mid` unique reste **juste** dans l'enfant,
> qui n'a bien qu'une piste vidéo.

`Controleur` s'instancie par flux sans difficulté, **mais son alimentation
non** : `Event::EgressBitrateEstimate` est une estimation **de session**, pas de
piste — c'est la capacité du chemin, partagée. Le chantier D devra donc insérer
une **couche de répartition** (parts égales ? prorata des pixels ? priorité à la
fenêtre au premier plan ?) avant de remettre la capacité à N contrôleurs. Deux
dettes voisines à régler en même temps : `audio_bps` est un budget unique de
session, à retirer une fois et non N fois, et le filtre de `MediaEgressStats`
s'appuie sur un `video_mid` unique. **À cadrer dans le plan du chantier D
plutôt qu'à découvrir à l'exécution.**

### Réglages du contrôleur, et lequel n'est pas calibré

`agent/src/congestion/echelle.rs` : `BPP_MIN` (0,05 bit par pixel et par image) est
**reconduit faute de preuve du contraire, pas confirmé** — le critère qui
l'aurait validé suppose un jugement visuel qui n'a jamais été porté. Il est
**couplé au `fps` de `Config`** (fixé à 60, la cadence délivrée, et non à
`ENCODER_FPS` qui vaut 90 et n'est que la cadence de sollicitation) : les deux
doivent être recalibrés **ensemble**. `DELAI_REMONTEE` a été porté de 10 à 20 s
pendant la recette pour réduire une oscillation, améliorée sans être éliminée.

> ⚠️ **Ces 20 s de `DELAI_REMONTEE` ont fait échouer trois mesures de D6, et
> c'est un piège de MÉTHODE à connaître (3 août 2026).** Le palier de focus de la
> recette D6 durait **25 s** : la fenêtre d'observation n'excédait la
> temporisation du mécanisme observé que de **25 %**, marge dans laquelle devaient
> encore tenir l'annonce de focus, la redistribution des parts et la montée de
> l'estimation. Trois promotions de barreau sur quatorze sont arrivées **après**
> la fin du palier — relevées 63 et 66 s plus tard sur la même fenêtre —, une
> quatrième a été **préemptée**, et l'échec s'imputait au **produit** alors qu'il
> venait du **protocole**. **Lire les constantes de temporisation du code AVANT
> de dimensionner un palier de mesure.**
>
> ⚠️ **`BPP_MIN` reste NON CALIBRÉ après D6, et le sous-bloc en a pourtant fait
> le levier central.** D6 a mesuré ce que rendent quatre barreaux de cette
> échelle à huit fenêtres (18,03 % / 7,99 % / 3,94 % / 1,47 % d'images jetées par
> le navigateur, une exécution par point) et a établi que **c'est la résolution,
> et non le débit, qui commande** — le débit n'étant que la commande de
> l'échelle. **Mais aucun jugement visuel n'a davantage été porté** : le critère
> qui validerait `BPP_MIN` manque exactement comme avant. Le couplage avec le
> `fps` de `Config` (60) est en revanche **employé** par D6 pour calculer les
> `min_bps` de chaque barreau : les deux se recalibrent toujours ensemble.

---

## 🌐 Chantier C volet 2 — traversée NAT (30 juillet 2026)

Recette complète : `docs/superpowers/plans/2026-07-29-traversee-nat-resultats.md`.

L'agent est un client TURN à part entière (`agent/src/turn/`) : il alloue un
relais **avant** de répondre à l'offre, publie le candidat relayé et le candidat
réflexif, encapsule en ChannelData ce qui doit passer par le relais, et
rafraîchit son bail. Le signaling délivre aux deux pairs des identifiants
éphémères (`signaling/src/ice.ts`) dérivés d'un secret qui ne quitte jamais le
serveur. Mesuré : le média traverse un relais réel pour **≈2 ms de RTT en plus**
(2,0 → 4,0 ms), sans perte de cadence.

### Lancer coturn : deux fichiers compose, pas un

`docker-compose.yml` est **gitignoré** (il porte les mots de passe Windows du
Guacamole historique en clair). Le relais vit donc dans un fichier séparé et
versionné, sans aucun secret :

```bash
docker compose -f docker-compose.yml -f docker-compose.coturn.yml up -d coturn
docker compose -f docker-compose.yml -f docker-compose.coturn.yml logs coturn
```

Variables à poser dans `.env` : `TURN_SECRET` (`openssl rand -hex 32`),
`TURN_REALM`, `TURN_EXTERNAL_IP`, et `TURN_URL` (lue par le **signaling**, sans
laquelle il n'annonce aucun relais et le journalise).

**coturn écoute sur toutes les interfaces de l'hôte, dont l'adresse publique** —
constaté (`UDP listener opened on: 90.87.35.18:3478`). L'accès est authentifié et
les identifiants expirent, mais c'est un relais joignable depuis Internet : à
restreindre (`--listening-ip` ou pare-feu) avant tout déploiement durable.

### Le signaling doit être relancé AVEC l'environnement

Piège rencontré : un serveur de signaling tournait depuis 36 h sans les variables
TURN, et les sessions ne recevaient donc aucune configuration ICE — sans que rien
ne le signale côté client. Vérifier l'environnement du processus **qui écoute
réellement**, pas de celui qu'on croit avoir lancé :

```bash
P=$(ss -ltnp | grep ':8080 ' | grep -o 'pid=[0-9]*' | cut -d= -f2 | head -1)
tr '\0' '\n' < /proc/$P/environ | grep ^TURN_URL=
```

### Relever PAR OÙ passe le flux, pas seulement qu'il passe

`client/verify-webrtc.mjs` prouve que le média traverse, jamais par quel chemin.
`client/recette/paire-candidats.mjs` relève la paire de candidats réellement
employée, le type des deux candidats, le RTT et le débit :

```bash
node client/recette/paire-candidats.mjs 'http://127.0.0.1:5174/?session=demo' 10000
FORCER_RELAIS=1 node client/recette/paire-candidats.mjs   # iceTransportPolicy 'relay'
```

`FORCER_RELAIS` intercepte le constructeur `RTCPeerConnection` dans la page :
aucune modification du code client à committer puis retirer.

### `relay ↔ relay` est inatteignable en laboratoire — ce n'est pas un défaut

Sur un pont où les deux pairs se voient, la paire nominée est toujours
`relay ↔ host` : pour que le relais de l'agent fonctionne, coturn doit pouvoir
joindre l'agent, et cette même joignabilité valide la paire `host`, prioritaire
en ICE. Des règles de pare-feu Windows bloquant l'UDP direct n'y changent rien
(le trafic relayé arrive depuis la plage de relais, donc autorisé). **Prouver le
chemin relayé côté agent pour le média exige deux réseaux réellement distincts.**
Le chemin d'encapsulation est en revanche exercé et prouvé pour les contrôles de
connectivité ICE (`CREATE_PERMISSION` et `CHANNEL_BIND` acceptés par coturn).

### Trois objets TURN expirent, pas un seul

Le bail de l'**allocation** (600 s) n'est pas le seul minuteur. Une
**permission** dure 300 s (RFC 5766 §8) et une **liaison de canal** 600 s (§11).
Passé ces délais, le serveur cesse de relayer **sans rien annoncer** — ni erreur,
ni message : une session relayée mourrait d'un silence au bout de 5 minutes.

Le plan du chantier ne prévoyait que le bail. `agent/src/turn/canaux.rs`
réaffirme désormais chaque liaison à 150 s (moitié de la permission, la plus
courte des trois durées). Toute évolution du client TURN doit préserver ces
**trois** rafraîchissements.

**Piège lié, qui a coûté une seconde mesure** : `handle_packet` reconduisait le
bail à *chaque* réponse de succès du serveur. Or ni `CreatePermission` ni
`ChannelBind` n'en portent — leurs réponses, arrivant toutes les 150 s,
repoussaient sans fin un rafraîchissement dû à 300 s, et l'allocation mourait à
600 s. Seules les réponses à `Allocate` et `Refresh` reconduisent un bail : la
méthode se lit avec `messages::methode_de`, qui défait l'entrelacement
classe/méthode de la RFC 5389 §6.

Trois traces `info` rendent tout cela observable (`état du client TURN` une fois
par minute, émission du bail, réaffirmation d'un canal) : c'est par elles que le
diagnostic a été fait, elles sont rares par construction — ne pas les retirer.

### Une page Chrome sans interface gèle au bout de 5 minutes

Piège de recette, coûteux : deux sessions de 11 minutes se sont interrompues à
331 s et 340 s — soit 300 s plus le délai de révocation du consentement ICE. Ce
n'était ni le produit ni le relais (la session **directe** tombait pareil), mais
Chrome qui gèle une page jamais mise au premier plan.

Toute mesure de plus de 5 minutes doit lancer Chrome avec
`--disable-background-timer-throttling`,
`--disable-backgrounding-occluded-windows` et
`--disable-renderer-backgrounding` (posées dans
`client/recette/paire-candidats.mjs` ; `client/verify-webrtc.mjs` ne les a pas,
ses mesures ne dépassant pas 25 s).

**Leçon de méthode :** le délai collait si bien au minuteur des permissions TURN
que la cause a d'abord été imputée au relais, à tort. Une mesure témoin sur le
chemin *sans* la fonctionnalité suspecte coûte dix minutes et évite une
conclusion fausse.

### Ne jamais tracer par paquet dans la boucle de transport

Une session relayée a échoué à s'établir uniquement parce que le binaire portait
encore une trace `tracing::info!` par `Transmit` — 18 619 lignes en quelques
secondes, écrites sur un partage CIFS depuis la boucle. **La mesure détruisait ce
qu'elle mesurait.** Compter ou échantillonner, jamais tracer par paquet.

### Réserve connue : pas d'appariement des transactions

`TurnClient::handle_packet` (`agent/src/turn/allocation.rs`) accepte la réponse
du serveur sans vérifier que son identifiant de transaction apparie la requête en
cours. Les `trans_id` sont retenus dans `Etat` — c'est là que l'appariement se
brancherait — mais jamais relus. Une réponse tardive ou rejouée fait donc avancer
la machine à états.

---

## 🪟 Sonde de capture multi-fenêtres (30 juillet 2026)

Résultats complets :
`docs/superpowers/plans/2026-07-30-sonde-capture-multifenetre-resultats.md`.
Journaux bruts : `docs/superpowers/plans/journaux-sonde-multifenetre/`, accents
corrompus en amont par la page de code PowerShell. **Encodages mixtes, pas
tous UTF-8** : `dxgi.log`, `wgc.log`, `replis.log` et `nvenc.log` sont en
**UTF-16LE** (`iconv -f UTF-16LE -t UTF-8 fichier.log` pour les lire ou les
`grep` — un `grep` direct dessus ne trouve rien) ; les journaux `banc-*.log`
sont en UTF-8 (avec BOM). **Cette conversion ne concerne QUE ce répertoire** :
`scripts/run-agent.sh` a été corrigé le 31 juillet 2026 et les journaux de
`journaux-mesures-prealables/` sont tous en UTF-8 sans BOM, accents intacts,
`grep`-ables tels quels.

Chantier de **mesure sans livrable produit** : trancher, avant de spécifier le
chantier D, quelle voie de capture rend une image correcte **par fenêtre** quand
les fenêtres se recouvrent. Le banc vit dans
`agent/src/diagnostics/multifenetre/`, piloté par variables d'environnement
(`MULTIFENETRE_DXGI`, `_WGC`, `_REPLIS`, `_BANC` + `MULTIFENETRE_N`).

### Ce qui est mesuré — aucune voie n'est simplement viable

| Voie | Verdict |
| --- | --- |
| `Windows.Graphics.Capture` | **ÉLIMINÉE** — `CreateForWindow` en `0x800706BE` (`RPC_S_SERVER_UNAVAILABLE`), 3 reproductions. `IsSupported()` rend `true` et le service `CaptureService_5865d` tourne : ni composant absent, ni service arrêté |
| Un moniteur virtuel par fenêtre | **CONDITIONNELLE** — mécanisme établi, sortie DXGI réelle 3413×960 sur l'adaptateur qui porte NVENC (relevé sans journal joint — session Apollo non reproductible sans le propriétaire du poste) ; **plafond non mesuré** (un seul client Apollo apparié) |
| Tuilage disjoint | **CONDITIONNELLE** — 800×360 à 8 fenêtres, et surtout : **les menus débordent** de ≈284 px sur la tuile voisine. La voie censée garantir le non-recouvrement ne le garantit pas |
| `PrintWindow(PW_RENDERFULLCONTENT)` | **CONDITIONNELLE** — rend l'image **juste** d'une fenêtre D3D **recouverte** (contre-intuitif, vérifié) ; limite = **chemin CPU**, 29,1 i/s/fenêtre à N=2 |

**La capture ne décroche pas jusqu'à 8 fenêtres** (voie `duplication`, cadence
par fenêtre : 80,2 / 99,4 / 98,8 / 107,5 i/s à N = 1 / 2 / 4 / 8, toutes égales
entre elles) — **mais à aire totale fixe** : `disposition::tuiles` découpe le
bureau, donc la surface par fenêtre décroît quand N croît (débit de pixels
quasi constant, ~208-258 MP/s, par construction). Ce montage ne dit rien du cas
où N fenêtres garderaient chacune sa résolution utile (8×1280×720 = 2,8× le
bureau mesuré) : le nombre de fenêtres n'est pas prouvé neutre en soi. Le banc
est validé par le fait que la voie de production **se pollue bien** sous
recouvrement (`verdicts_faux` = 449 / 449 / 536 à N = 2/4/8), ce qui **coupe la
passe d'encodage** — il n'existe donc aucune mesure d'encodage multi-fenêtres
par cette voie, par construction du protocole.

> ✅ **Note du 31 juillet 2026 — les deux réserves du paragraphe ci-dessus sont
> levées** par le chantier des duplications parallèles (section « N duplications
> DXGI de front » plus bas). **Le cas « N fenêtres gardant chacune leur
> résolution utile » a été exercé** : 8 sorties de 1280×720, aire totale
> multipliée par 8 (0,92 → 7,37 Mpx), cadence par fenêtre inchangée (90,1 i/s).
> Et **des mesures d'encodage multi-fenêtres existent désormais** — quatre rangs,
> jusqu'à N=8, zéro verdict faux. Ce montage-là n'a pas de recouvrement (une
> fenêtre par sortie), donc pas de porte éliminatoire : ce n'est pas le montage à
> aire fixe ci-dessus, et **les deux séries ne se comparent d'aucun chiffre**.

**Plafond d'encodage, formulation bornée, composant du refus non identifié** :
sur un périphérique D3D11 **unique et partagé**, à 720p/60/8 Mb/s, 8 instances
du pipeline Media Foundation réussissent, la 9ᵉ échoue à la **liaison du type
d'entrée** (`MF_E_UNSUPPORTED_D3D_TYPE`). Ce n'est **pas** « la limite NVENC de
cette carte » — le transform n°9 s'instancie sans peine. Deux appels
`SetInputType` s'enchaînent entre le succès et l'échec (encodeur H.264 puis
Video Processor MFT du convertisseur de couleur) et rien n'indiquait lequel
refusait — corrigé par un `.context()` distinct sur chacun
(`agent/src/encode.rs`), pour que la prochaine mesure tranche.

> ⚠️ **Corrigé le 31 juillet 2026 : ce n'était aucun de ces deux
> `SetInputType`.** La mesure ② du chantier de mesures préalables (rapport
> `task-9-report.md`, journaux `docs/superpowers/plans/journaux-mesures-prealables/nvenc-*.log`)
> a relevé une chaîne de causes **nue** avec ces deux contextes déjà en place :
> l'appel fautif était un `?` sans contexte, à savoir **`SetOutputType` de
> l'encodeur H.264**. Le libellé de `MF_E_UNSUPPORTED_D3D_TYPE` parle du type
> d'**entrée** alors que l'appel refusé règle le type de **sortie** — c'est très
> probablement ce libellé qui avait égaré l'attribution ci-dessus. **Ne pas se
> fier au texte de ce HRESULT pour désigner un appel.** Dix appels du chemin de
> construction portent désormais un contexte distinct.

**Le plafond ne vient pas du partage du périphérique** (mesure ② du même
chantier) : avec **un périphérique D3D11 neuf par encodeur**, le plafond reste
**8**, refus au même `SetOutputType`. Cette attribution causale porte une
réserve : **comparaison à deux variables confondues**, le mode séparé n'ouvrant
aucune duplication DXGI là où le mode partagé en ouvre une — il faudrait une
coïncidence pour que deux effets se compensent exactement, mais le témoin propre
(mode séparé *avec* duplication) n'a pas été exercé. Sous cette réserve, séparer
les périphériques ne fait gagner aucune fenêtre — et le coût correspondant
(partager des textures entre le périphérique de capture et ceux des encodeurs)
n'a donc pas à être payé. Ce que la mesure **ne** dit **pas** non plus : quelle
couche impose ce plafond (NVENC, pilote, Media Foundation, ou virtualisation),
s'il tient à d'autres résolutions ou débits, et si 8 encodeurs tiennent la
cadence *ensemble* — aucune image n'a été soumise, seule la **construction** est
mesurée. Enfin, détruire un encodeur n'a **pas** été montré libérer la place :
la mise en sommeil des fenêtres masquées reste à éprouver par une séquence
« créer 8 → en détruire 1 → tenter un 9ᵉ ».

> ✅ **Note du 31 juillet 2026 — « si 8 encodeurs tiennent la cadence ensemble »
> est répondu : ils la tiennent.** Huit encodeurs alimentés de front pendant
> 10 s rendent **90,1 i/s par fenêtre** en capture+encodage, zéro verdict faux
> (chantier des duplications parallèles, section plus bas). Portée exacte : sur
> **huit périphériques D3D11 distincts**, 1280×720 à 60 Hz et 8 Mb/s, **une**
> exécution par rang, unités H.264 comptées et jamais décodées. Le montage de la
> mesure ② — **périphérique unique partagé** — n'a, lui, toujours jamais été
> alimenté. **Les autres réserves de ce paragraphe restent entières** : la couche
> qui impose le plafond de 8 n'est pas identifiée, rien n'est su d'autres
> résolutions ou débits, et la mise en sommeil des fenêtres masquées demeure une
> conjecture — « créer 8 → en détruire 1 → tenter un 9ᵉ » n'a toujours pas été
> jouée.
>
> ✅ **ELLE A ÉTÉ JOUÉE le 3 août 2026 (sous-bloc D5), et la conjecture est
> CONFIRMÉE : détruire un encodeur libère la place.** Quatre exécutions,
> **10 cycles « détruire un, en construire un » sur 10** à chacune, dans
> l'arrangement de production (un processus, un périphérique D3D11 par
> encodeur). Le plafond de 8 porte donc sur la **concurrence**, pas sur les
> créations cumulées. ⚠️ **La couche qui l'impose reste inconnue**, et ce banc
> n'a soumis aucune image ni ouvert aucune duplication.

**Voie recommandée** : le moniteur virtuel par fenêtre (seule voie qui préserve
le chemin GPU en supprimant le recouvrement par construction), avec `PrintWindow`
en repli mesuré pour les fenêtres non-jeu. Le relevé Apollo (3413×960) qui fonde
cette voie n'a pas de journal joint et n'est pas reproductible sans le
propriétaire du poste.

> ⚠️ **Recommandée, et éprouvée le 1ᵉʳ août 2026 en conditions de produit
> (sous-bloc D1) : elle tient à arrangement FIGÉ et s'effondre dès qu'une
> fenêtre s'ouvre.** Créer une sortie virtuelle fait abandonner le mutex des
> duplications DXGI déjà ouvertes, donc tue toutes les captures en cours. Voir
> la section « Sous-bloc D1 » plus bas.
>
> ✅ **CETTE PHRASE N'EST PLUS VRAIE — le sous-bloc D2, le même jour, a réparé
> ce défaut et l'a démontré réparé en conditions de produit.** L'abandon du
> mutex se produit toujours (il n'est ni évité ni expliqué), mais il est
> désormais **encaissé** : la duplication est relâchée puis rouverte dans une
> fenêtre de reprise, et **44 pertes d'accès n'ont tué aucune session**. La voie
> tient donc à arrangement **dynamique** — jusqu'à **quatre** fenêtres
> simultanées, où un **plafond distinct** apparaît (la 5ᵉ duplication DXGI, dans
> un 5ᵉ processus, est refusée en `0x887A0022` ; **la couche qui l'impose n'est
> pas identifiée**). Voir la section « Sous-bloc D2 » plus bas.
>
> ✅ **Le sous-bloc D3 (2 août 2026) a caractérisé ce plafond distinct : il porte
> sur le nombre de PROCESSUS concurrents tenant une duplication, et vaut
> exactement 4.** Quinze exécutions, 3 par rang : huit duplications dans un seul
> processus passent, huit réparties sur quatre processus passent, mais la
> cinquième duplication dans un cinquième processus est refusée — **avec moins de
> duplications ouvertes que les rangs qui réussissent**. ⚠️ **La couche qui
> l'impose n'est toujours PAS identifiée** ; seul l'objet du plafond l'est.
>
> ✅ **Le sous-bloc D4 (2 août 2026) CONTOURNE ce plafond : la capture est
> mutualisée dans un processus unique, qui a tenu 8 duplications et
> 8 encodeurs.** Le plafond de quatre **processus** ne borne donc plus le nombre
> de fenêtres capturées. ✅ **Et il ne borne plus le nombre de fenêtres qui
> DIFFUSENT non plus : la seconde recette de D4 en relève HUIT simultanément**,
> images décodées par le navigateur sur les huit voies. *(Une première rédaction
> de cette annotation disait « ce nombre vaut ZÉRO » : c'était le relevé de la
> première recette de D4, avant que la tâche 10 ne répare le canal.)* **La
> couche du plafond reste, elle, toujours non identifiée** — D4 le contourne, il
> ne l'explique pas. Voir les sections « Sous-bloc D3 » et « Sous-bloc D4 »
> plus bas.

> ✅ **Les deux mesures que cette sonde déclarait bloquantes ont été prises le
> 31 juillet 2026** (plafond de sorties virtuelles, plafond d'encodage sur
> périphériques séparés) — voir la section suivante. Le chantier D est
> spécifiable. Le repli `PrintWindow`, lui, **recule** : mesuré à N=4 et N=8, il
> ne tient pas la cible de 8 fenêtres.

### Pièges — à connaître avant de toucher à ce terrain

- **Animer les mires.** Desktop Duplication n'émet une trame **qu'au changement
  du bureau**. Une mire immobile fait rendre `WAIT_TIMEOUT` à toutes les
  acquisitions : on mesure zéro image et on conclut à tort à une panne.
- **Peindre en D3D11, jamais en GDI.** `PrintWindow` sait faire redessiner une
  fenêtre par `WM_PRINT` : sur une mire GDI il rendrait toujours une image
  juste, et la voie 4 serait **validée à tort**. Le banc peint par chaîne
  d'échange D3D11 (`mires.rs`), et c'est ce qui donne son poids au résultat.
- **Un processus par voie.** Ces API échouent par **plantage du processus**
  (`0xc0000005` vu au jalon 1), pas par code d'erreur : une sonde monolithique
  perd toutes les mesures déjà faites.
- **DXGI n'autorise qu'UNE seule duplication ouverte par sortie** — exactement
  une, pas « un nombre très limité ». Un `DesktopCapture` provisoire laissé en
  vie fait échouer la suivante en `0x80070057`.
- **Ne pas refondre acquisition et recadrage dans le trait `VoieDeCapture`.**
  C'est la lacune du plan initial : la première voie du tour consommait
  l'`AcquireNextFrame`, les autres récoltaient `WAIT_TIMEOUT` — **famine dès
  deux fenêtres**, et le plan désignait ce trait comme la couture destinée au
  chantier D : la lacune serait allée en production. Corrigé : acquisition
  mutualisée une fois par tour (`SourceDuplication::amorcer`), **une texture de
  destination par voie** (sans quoi les voies s'écrasent mutuellement).
- **WMI ment sur la résolution** : champ vu périmé de 68 s. Source de vérité =
  `GetDesc`/`DesktopCoordinates`. Corollaire : la sortie virtuelle est annoncée
  5120×1440 par WMI mais mesurée **3413×960** (rapport 1,5 = DPI 150 %) — si un
  recadrage est calculé sur le rectangle virtualisé alors que la texture est aux
  dimensions physiques, il sera décalé d'un facteur 1,5.
- **Leçon de méthode** : les onze rondes de correction de ce chantier ont
  quasiment toutes porté sur des **rapports qui affirmaient au-delà de leur
  relevé**, jamais sur des bugs. Sur un chantier de mesure, le coût est dans la
  discipline de l'énoncé, pas dans le code.

---

## 📐 Mesures préalables au chantier D (31 juillet 2026)

Résultats complets :
`docs/superpowers/plans/2026-07-31-mesures-prealables-chantier-d-resultats.md`.
Journaux : `docs/superpowers/plans/journaux-mesures-prealables/` — **tous en
UTF-8 sans BOM, accents intacts, aucune conversion nécessaire** (contrairement
à ceux de la sonde ci-dessus). Le document de reconnaissance du canal de
contrôle du pilote y vit aussi : `canal-de-controle.md`.

Second **chantier de mesure sans livrable produit** : lever les quatre inconnues
que la sonde ci-dessus laissait ouvertes. **Les quatre sont levées, chacune avec
son journal versé** — la faiblesse que la sonde avait laissée sur son propre
chiffre fondateur. **Le chantier D est spécifiable.**

### Les quatre chiffres

| # | Résultat | Journal |
| --- | --- | --- |
| ① | **Plafond de sorties virtuelles = 10.** Refus du pilote à la 11ᵉ création (IOCTL `0x00222000`, `0x80070044` / `ERROR_TOO_MANY_NAMES`), **prouvé par identité** — les onze sorties sont énumérées nommément par la ligne de refus (liste mémorisée au relevé qui précède, ouvert 0,673 ms plus tôt, **pas** une relecture fraîche : le libellé du journal suggère le contraire). Cible du chantier D = 8 : la voie tient avec 2 de marge | `moniteurs-montee-en-n.log` |
| ② | **Plafond d'encodage = 8, inchangé sur périphériques D3D11 séparés.** 9 périphériques distincts et vivants côté mode `separe`, même rang refusé. **Le partage du périphérique n'était donc pas la contrainte** | `nvenc-partage-temoin.log`, `nvenc-separe.log` |
| ③ | **Windows compose bien sur un moniteur virtuel sans écran physique**, et Desktop Duplication en rend l'image exacte : 900/900 verdicts justes, **zéro image noire**, 90,0 i/s/fenêtre. L'hypothèse fondatrice de la voie recommandée tient | `moniteurs-capture.log`, `moniteurs-capture-n1-encodage.log` |
| ④ | **`PrintWindow` ne tient pas l'échelle** : 17,6 i/s/fenêtre à N=4, **8,8 à N=8** (17,2 et 8,8 avec l'encodage). Recollé aux deux rangs de la sonde, le débit de pixels décroît **sur les quatre rangs sans palier** : **116,64 → 75,43 → 45,62 → 20,28 MP/s**, à opposer aux 208–258 MP/s quasi constants de `duplication` | `printwindow-n4.log`, `printwindow-n8.log` ; N=1 et N=2 : `journaux-sonde-multifenetre/banc-printwindow-{1,2}.log` |

**Acquis d'outillage** : notre code **commande le pilote SudoVDA lui-même** (canal
IOCTL, `canal-de-controle.md`) — plus besoin d'une session Apollo pour faire
paraître une sortie virtuelle ; une **purge autonome** rattrape les sorties
orphelines, éprouvée sur un état réellement sale (8 orphelines, constat et
confirmation par des processus tiers).

> ✅ **Acquis complémentaire de D8 (5 août 2026) : le canal IOCTL n'est PAS le
> seul levier sur une sortie virtuelle.** Ses six IOCTL n'exposent aucun
> changement de mode — ce qui a longtemps fait croire qu'une sortie était figée à
> sa taille de création —, mais **l'API d'affichage Windows ordinaire
> (`ChangeDisplaySettingsExW` sur `\\.\DISPLAYn`) en change bien le mode**,
> relecture DXGI à l'appui, **mouvement confirmé 5 fois**. La sortie **garde son
> nom, son attache et sa place dans la table**. ⚠️ **Portée exacte** : jamais
> sur une sortie **dont la duplication est ouverte**, ce qui est le cas du
> produit — voir « Sous-bloc D8 ». Et **une sortie ne naît pas à la taille
> demandée** mais à la dernière laissée au registre.

### Ce que ces mesures NE disent pas

- **L'arrangement que la voie recommandée propose réellement n'est pas mesuré
  ICI** : N sorties virtuelles, **une fenêtre chacune**, donc **N duplications
  DXGI de front**. Le banc de ce chantier a posé N fenêtres sur **UNE** sortie.
  DXGI n'autorisant qu'une duplication par sortie, la question était réelle.
  ✅ **Elle a été mesurée le 31 juillet 2026 — voie reçue à N=8** : voir la
  section « N duplications DXGI de front » ci-dessous.
  ⚠️ **Mais dans un ORDRE que le produit n'a pas** : le banc créait ses N sorties
  avant d'ouvrir la moindre duplication. Le sous-bloc D1 a exercé l'ordre réel
  (une fenêtre s'ouvre pendant que d'autres capturent) et il échoue — voir la
  section « Sous-bloc D1 ».
  ✅ **L'ordre réel passe depuis le sous-bloc D2** (1ᵉʳ août 2026) : la reprise
  sur perte d'accès encaisse la perturbation, éprouvée au banc à k = 1, 2, 4
  puis en conditions de produit jusqu'à **quatre** fenêtres. **Rien au-delà de
  quatre** : un plafond distinct y arrête la montée — voir la section
  « Sous-bloc D2 ».
  ⚠️ **La formulation de cette phrase était fausse et D3 l'a corrigée** : elle
  disait « sur le nombre de duplications DXGI simultanées **dans des processus
  distincts** ». Le plafond ne porte **pas** sur un nombre de duplications :
  huit duplications tiennent, réparties sur un, deux ou quatre processus. Il
  porte sur le **nombre de processus** concurrents tenant une duplication, et
  vaut **exactement 4** — le rang qui échoue n'a que 4 duplications ouvertes
  quand ceux qui passent en ont 8. Voir « Sous-bloc D3 ».
  ✅ **Et D4 a supprimé le « rien au-delà de quatre », côté CAPTURE comme côté
  DIFFUSION** : la capture mutualisée met les N duplications dans un processus
  unique, qui en a tenu 8 avec 8 encodeurs, et la **seconde recette de D4**
  relève **8 fenêtres qui diffusent simultanément**. *(La première recette de D4
  n'en relevait aucune — canal défectueux, réparé par la tâche 10.)* Voir
  « Sous-bloc D4 ».
- **La comparaison des deux modes d'encodage porte sur DEUX variables
  confondues** : le mode `separe` n'ouvre aucune duplication DXGI là où
  `partage` en ouvre une. Le témoin propre n'a pas été exercé.
- **La couche qui impose le plafond de 8 n'est pas identifiée**, aucune image
  n'a été soumise (seule la *création* est mesurée), et **détruire un encodeur
  n'a pas été montré libérer la place** — la mise en sommeil des fenêtres
  masquées repose donc sur une conjecture.
  ✅ **Des images ONT depuis été soumises sur un périphérique D3D11 UNIQUE ET
  PARTAGÉ** (seconde recette de D4, 2 août 2026) : huit encodeurs d'un même
  processus alimentés ensemble, **494,4 i/s cumulées**, décodées par un
  navigateur. ⚠️ **Le reste de cette ligne tient intégralement** : la couche du
  plafond de 8 n'est **toujours pas** identifiée, et **détruire un encodeur n'a
  toujours pas été montré libérer la place** — au contraire, la seconde recette
  de D4 relève que **reconfigurer** un encodeur à 8 fenêtres est refusé au même
  `SetOutputType`, 18 fois sur 18, ce qui rend cette question **urgente** plutôt
  que théorique. Voir « Sous-bloc D4 ».
  ✅ **Des images ont depuis été soumises** (31 juillet 2026) : 8 encodeurs
  alimentés ensemble tiennent 90,1 i/s par fenêtre — mais sur huit
  périphériques D3D11 **distincts**, pas sur le périphérique unique partagé de
  cette mesure-ci. **La couche du plafond et la mise en sommeil restent, elles,
  entièrement ouvertes.**
  ✅ **La mise en sommeil N'EST PLUS OUVERTE (3 août 2026, sous-bloc D5) :
  détruire un encodeur libère bien la place** — 4 exécutions, 10 recyclages sur
  10 chacune —, et **le défaut qui en découlait est mort** : à huit fenêtres,
  `set_encode_size` réussit désormais **198 fois sur 198**, contre 18 refus sur
  18 en D4, parce qu'il détruit l'ancien encodeur avant d'en construire un neuf.
  ⚠️ **La couche du plafond, elle, reste entièrement inconnue.**
- **La cause du refus à la 11ᵉ sortie n'est pas isolée**, et on ignore si le
  vivier de 10 est global au pilote ou par client (Apollo pingue le même
  pilote). **L'unité du chien de garde (`delai = 3`) reste inconnue : aucune
  unité n'est exclue, pas même la seconde.**

### ⚠️ Défaut alors ouvert — depuis diagnostiqué et corrigé (31 juillet 2026)

> ✅ **Ce défaut est traité** : diagnostic, réfutation de `MFShutdown` et
> correctif au chantier « N duplications DXGI de front » ci-dessous, §7 de
> `docs/superpowers/plans/2026-07-31-duplications-paralleles-resultats.md`.
>
> ⚠️ **Et le bornage ci-dessous porte une erreur à ne pas reprendre : il laisse
> croire à un défaut DÉTERMINISTE.** « Les deux exécutions » ne dit pas combien
> avaient passé. Il est **intermittent** — 2 plantages sur 6 exécutions du cas
> comparable, et un rapport antérieur avait eu **quatre exécutions propres
> d'affilée sur un binaire non corrigé**. Un défaut intermittent qu'on croit
> déterministe se déclare « corrigé » à la première exécution qui passe.

**La passe d'encodage du banc tue le processus, sur la voie `duplication`.**
Bornage exact, à ne pas élargir :

- **à la SORTIE de la boucle, pas pendant** — les deux exécutions écrivent leur
  dixième et dernière ligne périodique à début + 10,00 s, et le bilan n'est
  jamais atteint. Cela désigne la **libération** du `Vec<H264Encoder>` et de la
  duplication, **pas** la soumission d'images ;
- **quelle que soit la sortie capturée** : le même banc sur le **bureau
  physique** meurt au même endroit — la sortie virtuelle est hors de cause ;
- **pas sur `printwindow`** : `printwindow-n4.log` et `printwindow-n8.log`
  portent tous deux leur ligne `passe terminée passe="capture+encodage"` ;
- **un encodage doit probablement avoir réellement eu lieu** : la mesure ②
  forme le couple duplication + encodeurs et **survit** à la destruction de ses
  8 encodeurs — mais sans avoir jamais soumis d'image.

Conséquence : la garde ne court pas, donc **la sortie virtuelle survit au
processus** (`MULTIFENETRE_VDD_PURGE=1` pour la retirer).

### Pièges neufs — à connaître avant de toucher à ce terrain

- **Un journal PowerShell lisible demande DEUX réglages, pas un.** Le mojibake
  des journaux de la sonde ne venait pas seulement de `Tee-Object` en UTF-16LE,
  mais AUSSI de `[Console]::OutputEncoding` resté sur la page de code OEM, qui
  abîmait les accents **en lisant** la sortie du processus enfant, avant même
  l'écriture. Le `StreamWriter` règle l'écriture, `[Console]::OutputEncoding` la
  lecture. Symptôme : un `grep` sur un mot accentué rend 0 quand le même `grep`
  sur sa partie ASCII rend 1. *(Ne pas remplacer `Tee-Object` par
  `Out-File -Encoding utf8` : il replie les lignes à la largeur de console.)*
- **Ne jamais se fier au texte d'un HRESULT pour désigner un appel.** Voir le
  bloc de correction d'attribution ci-dessus : dix annotations de contexte ont
  été nécessaires pour savoir quel appel refusait le 9ᵉ encodeur.
- **Un compteur ne suffit pas quand un tiers agit sur le système.** Apollo peut
  ajouter une sortie à tout instant : une addition externe compense exactement
  un retrait, et un contrôle par cardinal passe alors qu'une sortie a disparu.
  **Comparer des ensembles de noms, jamais des nombres.**
- **Le contrôle qui vaut se fait depuis un processus NEUF.** Le processus
  mesureur est juge et partie, et une sortie virtuelle lui survit.
- **Un plantage à la destruction se lit comme un plafond.** Instrumenter la
  sortie autant que l'entrée (deux traces encadrant le relâchement).
- **Modifier le banc rend les mesures antérieures non comparables** — et il faut
  le dire : la lecture de pixel a changé de portée en cours de chantier, les
  cadences `printwindow` viennent d'un banc qui relisait deux fois moins. Risque
  borné par un témoin, **borné n'est pas nul**.
- **Un compte de créations obtenu par minutage est un artefact de minutage.**
  L'épreuve de salissure a produit 8 sorties, pas 10, parce que le `sleep` de
  l'hôte ne mesure pas le temps de vie de l'agent. Le plafond reste 10.
- **Le mode de défaillance dominant reste l'énoncé, pas le code** — comme pour
  la sonde. Sur tout le chantier, **un seul** point a vu le code contredire son
  rapport ; tout le reste était des phrases qui affirmaient au-delà du relevé.

### Variables d'environnement du banc et des sondes

Un processus par voie (ces API échouent par **plantage du processus**, pas par
code d'erreur).

| Variable | Effet |
| --- | --- |
| `MULTIFENETRE_DXGI=1` | Relève la topologie DXGI et sort — le contrôle d'état depuis un processus neuf |
| `MULTIFENETRE_WGC=1` | Sonde `Windows.Graphics.Capture` (voie éliminée) |
| `MULTIFENETRE_REPLIS=1` | Sonde les voies de repli |
| `MULTIFENETRE_BANC=duplication\|printwindow` + `MULTIFENETRE_N=1..8` | Le banc de cadence |
| `MULTIFENETRE_SORTIE=<\\.\DISPLAYn>` | Force la sortie DXGI capturée par le banc. **Un NOM, plus un couple d'index** depuis D2 : `DesktopCapture::sur_sortie` résout par nom (les index sont positionnels). Un `0:1` récolte « aucune sortie DXGI nommée 0:1 » |
| `MULTIFENETRE_CONTRAT=1` | Éprouve le contrat IOCTL du pilote (deux tampons simples, sans effet de bord) |
| `MULTIFENETRE_VDD=1` | **Mesure ①** — montée en N de sorties virtuelles jusqu'au refus |
| `MULTIFENETRE_VDD_VEILLE=<secondes>` | Épreuve du chien de garde : une sortie, aucun ping, relevé à 1 Hz |
| `MULTIFENETRE_VDD_PURGE=1` | **Purge autonome** des sorties orphelines |
| `MULTIFENETRE_VDD_CAPTURE=1` | **Mesure ③** — crée une sortie virtuelle et y lance le banc |
| `MULTIFENETRE_NVENC=partage\|separe` | **Mesure ②** — plafond d'encodeurs, périphérique D3D11 partagé ou un par encodeur |
| `MULTIFENETRE_NVENC_CYCLES=<k>` | **Sous-bloc D5, la mesure pivot** — monte jusqu'au refus dans l'arrangement de PRODUCTION (un périphérique D3D11 par encodeur), puis répète *k* fois « détruire un, en construire un ». **Le cycle répété est le cœur** : un seul recyclage ne distingue pas un plafond de concurrence d'un plafond de créations cumulées. Branché **avant** `MULTIFENETRE_NVENC`, les deux variables ayant un préfixe commun |
| `MULTIFENETRE_VDD_PARALLELE=<1..8>` | **Chantier des duplications parallèles** — N sorties virtuelles × 1 fenêtre × 1 duplication DXGI × 1 encodeur, trois passes (témoin, capture, capture+encodage), contrôle d'image **en rotation**, chien de garde pingué à 1 Hz |
| `MULTIFENETRE_EPREUVE_FILE_MS=<ms>` | Bouche la file de travail sérialisée imposée à la MFT pendant la passe — c'est l'épreuve qui montre que la barrière n'est pas un placebo |
| `MULTIFENETRE_REPRISE=<k>` | **Sous-bloc D2** — *k* sorties virtuelles, *k* duplications, puis **une sortie de plus** créée en cours de capture : éprouve que les *k* duplications reprennent et rendent encore des images justes. Sonde post-mortem sur les voies mortes |
| `MULTIFENETRE_PLAFOND=<P>x<D>` | **Sous-bloc D3** — le **porteur** : crée K = P×D sorties virtuelles, bat le chien de garde, **ne duplique rien lui-même**, lance P processus **sondes** en escalier (la *i+1* attend que la *i* soit prête ou en échec), puis détruit ses sorties et compare la topologie **par ensemble de noms** |
| `MULTIFENETRE_PLAFOND_SONDE=<noms,séparés,par,virgules>` | **Sous-bloc D3** — la **sonde**, posée par le porteur et **jamais à la main** (avec `MULTIFENETRE_PLAFOND_RANG=<i>`). Ouvre une duplication par nom, les tient jusqu'au signal d'arrêt, journalise le `HRESULT` exact. **Branche en tête de tout l'aiguillage** : sans quoi un `MULTIFENETRE_VDD_PURGE=1` résiduel, hérité par l'enfant, détruirait les sorties vivantes du porteur en pleine mesure |
| `CAPTEUR=1` | **Sous-bloc D4** — lance l'agent en **capteur** de capture mutualisée (serveur du tube `\\.\pipe\agent-capteur`). Posée par le superviseur lui-même (`lancer_capteur`), pas à la main ; transmise par `scripts/run-agent.sh`. Un capteur qui hériterait de `SUPERVISEUR` se prendrait pour un superviseur |
| `AGENT_TRACE_EXCEPTIONS=1` | Arme le filtre d'exception (pile symbolisable de la faute, journal séparé d'`agent.log`). Inerte sans la variable. `…_FICHIER` en change la destination ; `…_AUTOTEST=1` **tue délibérément le processus** pour éprouver l'instrument |
| `BUDGET_BPS=<bps>` | **Sous-bloc D6** — le **budget de débit de toute la session**, lu par le **capteur** seul (`capteur/sommeil/parts.rs`), qui le découpe en parts et les pousse aux enfants. Défaut **12 000 000**. Transmise par `scripts/run-agent.sh` (tâche 9). C'est une variable de **produit**, pas de banc. Trace de contrôle : `budget de debit de la session budget_bps=<valeur>` — **comparer la VALEUR, jamais la seule présence de la ligne**, et **pas avant la première fenêtre** : elle vient d'un `OnceLock` initialisé au premier calcul de parts |
| `SOURCE_TRACE=1` | ⚠️ **MORT DES DEUX CÔTÉS en multi-fenêtres** (constaté le 3 août 2026, sous-bloc D6). Les écrivains vivent dans `windows_source.rs` — `PRODUCED` **313**, `TICKS` **518**, `CAPTURED` **544** (ordre non positionnel : ne pas apparier à la liste `TICKS`/`CAPTURED`/`PRODUCED`) —, donc dans le **capteur** depuis D4 ; le lecteur unique est `demarrage.rs:127-129` (déplacé depuis `142-144` par les
remaniements de D7), donc dans
l'**enfant** ; et `main.rs:268` rend la main à `capteur::executer` avant que `demarrage::executer` ne soit atteint. La variable n'affiche donc que des **zéros** côté enfant, et les compteurs du capteur ne sont lus par **personne**. **Elle ne décrit plus que le chemin mono-fenêtre**, sans `CAPTEUR` ni `SUPERVISEUR`. ✅ **Les cinq numéros de ligne de cette case ont été REVÉRIFIÉS le 5 août 2026 (D8, tâche 12) et sont tous EXACTS** — `313`, `518`, `544`, `demarrage.rs:127-129`, `main.rs:268` : la mention « valeur non revérifiée pour le reste » est donc levée. ✅ **LE REMÈDE A ÉTÉ APPLIQUÉ le 6 août 2026 (D9, tâche 11), et TOUT CE QUI PRÉCÈDE EST DEVENU DE L'HISTOIRE — y compris les cinq numéros de ligne, qui ne désignent plus rien.** Les trois statiques ont disparu : elles sont un champ `telemetrie` **par session** de `WindowsSource`, dans **`agent/src/windows_source/telemetrie.rs`** (**72** lignes, **pur, aucun `cfg`**, deux tests d'hôte). Le lecteur mort de `demarrage.rs` a été retiré ; **le lecteur est désormais `agent/src/capteur/fenetre/trace.rs`** (**43**), qui vit dans le CAPTEUR, là où les compteurs sont écrits, et qui trace **sous le span `fenetre{session=…}` posé par D7** — donc attribuable sans champ supplémentaire. **La convention de la variable est INCHANGÉE : la simple PRÉSENCE active** (à l'inverse de `PLEIN_ECRAN`/`AUDIO`/`SUPERVISEUR`/`CAPTEUR`, où `=0` désarme). Trace : `compteurs de capture ticks=… capturees=… produites=…`. Toujours transmise par `scripts/run-agent.sh`. ⚠️ **Ce qui reste dans `demarrage.rs`, côté enfant, est la partie ENCODEUR** (tentatives et accumulation de capture, entrées/sorties du convertisseur et de l'encodeur) : elle, n'a pas bougé et reste lue là. **Effet de bord recherché et obtenu** : `windows_source.rs` retombe de 638 à **628** |
| `MULTIFENETRE_MODE_SORTIE=<L>x<H>` | **Sous-bloc D8** — la sonde **P1** (`agent/src/diagnostics/multifenetre.rs:169`) : crée une sortie virtuelle, relit sa taille **courante** par DXGI, choisit une cible parmi les modes annoncés **en excluant cette taille courante**, tente `ChangeDisplaySettingsExW`, et **juge sur le MOUVEMENT relu par DXGI, jamais sur une égalité** — la première version rendait `P1 RECU` sans que rien n'ait bougé, son critère ne pouvant pas échouer. Rend `P1 NON MESURABLE` si aucun mode ne diffère de la taille courante. Transmise par `scripts/run-agent.sh` *(les numéros de ligne publiés ici — `:77` puis `:78` — ont dérivé DEUX fois : `PLEIN_ECRAN_MODE_SORTIE` en avait ajouté un, D9 l'a retiré et a ajouté `PART_SONDAGE`. **Ne plus recopier de numéro de ligne pour ce script : `grep -n` avant de s'y fier.**)*. ⚠️ **ÉTENDUE PAR LA PHASE P DE D9** : elle porte désormais la duplication DXGI ouverte et tenue pendant la tentative (l'écart banc/produit que D8 laissait béant), un **témoin sans duplication** dans la même exécution, une **quatrième combinaison de drapeaux** (`flags = 0`, sélectionnable par `MULTIFENETRE_MODE_SORTIE_DRAPEAUX`), le compte des **pertes d'accès infligées à deux voisines**, et une **épreuve de PERSISTANCE** (une sortie de plus est créée après le changement, et la sortie sous test est relue). Voir la section D9. ⚠️ **C'est AUSSI le remède opérationnel au blocage produit par pollution du registre** : une sortie naît à la dernière taille laissée au registre, et un registre resté à 2560×1440 empêche toute fenêtre de s'attacher — `MULTIFENETRE_MODE_SORTIE=1280x720` le rétablit. ⚠️ **Un lancement à elle seule** : l'aiguillage retourne après la première sonde reconnue, un `MULTIFENETRE_VDD_PURGE=1` dans le même lancement l'annulerait en silence |
| `PLEIN_ECRAN=0` | **Sous-bloc D8** — **variable de PRODUIT**, pas de banc. **Désarme le mécanisme ENTIER** : ni relecture du style (`capteur/fenetre.rs`), ni changement de mode de sortie (`windows_source/redimensionnement.rs`). ⚠️ **`=0` désactive ; une simple PRÉSENCE n'active pas** — même convention qu'`AUDIO`, `SUPERVISEUR` et `CAPTEUR`, et pour la même raison : tester `is_ok()` activerait le plein écran en écrivant `PLEIN_ECRAN=0` pour le couper. Lue dans le **capteur** seul (`capteur/plein_ecran.rs:78`), par `OnceLock` — l'enfant ne fait que relayer. Transmise par `scripts/run-agent.sh:34`. Traces de contrôle : `plein ecran DESARME (PLEIN_ECRAN=0) : …` au démarrage du capteur, et `redimensionnement ignoré : PLEIN_ECRAN=0 …` à chaque `resize`. ⚠️ ~~**C'est la SEULE parade actuelle au défaut HiDPI ouvert**~~ — **plus vrai depuis la revue finale de branche de D8** : le changement de mode étant désormais désarmé PAR DÉFAUT (ligne suivante), le défaut HiDPI est inatteignable en configuration livrée, et `PLEIN_ECRAN=0` est devenu la parade du mécanisme **entier**, plus la seule parade d'un défaut |
| ❌ ~~`PLEIN_ECRAN_MODE_SORTIE=1`~~ **CETTE VARIABLE N'EXISTE PLUS** (retirée le 6 août 2026, D9 tâche 3 — ni dans le code, ni dans `scripts/run-agent.sh` ; les legs 12 et 13 **cessent d'exister** au lieu d'être différés. Voir la section D9). Tout ce qui suit est le relevé de D8, conservé pour son diagnostic. | ~~**Sous-bloc D8, revue finale de branche (5 août 2026)** — **variable de PRODUIT**. **ARME** le changement de mode de la sortie virtuelle (`windows_source/redimensionnement/mode_sortie.rs`), **DÉSARMÉ PAR DÉFAUT**. ⚠️ **Convention INVERSE de `PLEIN_ECRAN`, à dessein** : on désarme sur `=0` ce qui est livré, on **arme sur `=1`** ce qui ne l'est pas — et ici la simple présence ne suffit pas non plus, il faut la valeur `1`. **Ce qui reste actif sans elle** : détection du style, annonce `PleinEcran`/`Fullscreen`, armement client. **Pourquoi** : critère ② **JAMAIS EXERCÉ** (`mode_sortie_demande=0` aux deux exécutions) et **deux Critiques ouvertes** — C1, la pollution du registre qui bloque les ouvertures de fenêtre ultérieures (portée inconnue : **cinq GUID SudoVDA distincts** au journal de recette) ; C2, la reprise D2 court-circuitée par une `Err` sur un échec **transitoire** de réouverture. **Les deux sont délibérément NON CORRIGÉES**, le désarmement les rendant inatteignables. Garde et raisons : `agent/src/capteur/plein_ecran.rs::changement_de_mode_arme`. Transmise par `scripts/run-agent.sh:35`. Traces : `changement de mode de sortie ARME (…)` au premier appel si armée, `redimensionnement ignoré : changement de mode de sortie DÉSARMÉ (…)` à chaque `resize` sinon.~~ **Ces deux traces n'existent plus** ; `resize` en mode `SortieEntiere` journalise désormais `redimensionnement ignoré : la source capture une sortie DXGI entière (voir le constat de mesure de capteur::plein_ecran, sous-bloc D9)` |
| `PART_SONDAGE=0` | **Sous-bloc D9, tâche 12** — **variable de BANC, jamais une configuration livrée**. Neutralise l'appel `set_desired_bitrate` de `agent/src/transport/part.rs` : c'est le bras « désarmé » de l'A/B différentiel que D6 laissait dû (son leg n°4). ⚠️ **Convention de `PLEIN_ECRAN` — `=0` DÉSARME, une simple présence n'active pas** ; l'appel est armé par défaut. Lue dans l'**enfant**. Transmise par `scripts/run-agent.sh`. Trace, émise **seulement si désarmé** : `objectif de sondage DESARME (PART_SONDAGE=0) : bras A/B, jamais une configuration livrée` (`warn!`). ⚠️ **L'A/B qu'elle sert a été joué et N'ÉTABLIT RIEN** : 2 exécutions par bras, écart de trafic cumulé +23,2 % dans le sens attendu, mais **variance intra-bras +83,1 %** — plus grande que l'écart mesuré. Voir la section D9 |

---

## 🖥️🖥️ N duplications DXGI de front sur N sorties virtuelles (31 juillet 2026)

Résultats complets :
`docs/superpowers/plans/2026-07-31-duplications-paralleles-resultats.md`.
Conception : `docs/superpowers/specs/2026-07-31-duplications-paralleles-design.md`.
Journaux : `docs/superpowers/plans/journaux-duplications-paralleles/` — **tous en
UTF-8**, accents `grep`-ables tels quels.

Troisième **chantier de mesure**, qui prend la seule mesure que les mesures
préalables laissaient due : **l'arrangement que la voie recommandée du chantier D
propose réellement** — une sortie virtuelle par fenêtre, une duplication DXGI par
sortie, un encodeur par sortie. Tout ce qui avait été mesuré jusque-là l'avait été
sur un montage qui n'est pas celui-là.

### Le résultat : **voie REÇUE**

> ⚠️ **Reçue par ce banc, et par lui seul.** Il crée ses N sorties virtuelles
> **avant** d'ouvrir la moindre duplication. Le sous-bloc D1 (1ᵉʳ août 2026) a
> exercé l'ordre du produit — une fenêtre s'ouvre alors que d'autres capturent —
> et **il échoue** : la création de la sortie fait abandonner le mutex des
> duplications ouvertes et tue toutes les sessions. Rien ci-dessous n'est
> réfuté ; c'est la portée qui est plus étroite qu'il n'y paraît.
>
> ✅ **L'ordre du produit passe depuis le sous-bloc D2 (1ᵉʳ août 2026)** : le
> mutex est toujours abandonné, mais la reprise l'encaisse et **aucune session
> n'en meurt**. ⚠️ **Ce qui reste plus étroit qu'il n'y paraît** : ce banc tient
> **8 duplications de front dans UN SEUL processus** ; en **processus distincts**
> — l'arrangement du produit — la **5ᵉ** est refusée (`0x887A0022`). Que la
> différence tienne au multi-processus est une **INFÉRENCE** : rien ne rapproche
> formellement les deux montages.
>
> ✅ **Cette inférence a été REMPLACÉE par une mesure le 2 août 2026
> (sous-bloc D3).** Un banc unique a opposé 1, 2, 4 et 8 processus à D
> duplications chacun : huit duplications passent sur un, deux ou quatre
> processus, et le refus tombe au **cinquième processus**. **La différence tient
> bien au multi-processus**, et le plafond vaut **exactement 4 processus**. Ce
> qui reste inconnu est la **couche** qui l'impose.

Le critère posé d'avance était : à N=8, **≥ 60 i/s par fenêtre en
capture+encodage et aucun verdict faux**.

| N | cadence/fenêtre, capture+encodage | verdicts faux | aire totale | débit de pixels **calculé** |
| --- | --- | --- | --- | --- |
| 1 | 90,1 i/s (`paralleles-n1.log:55`) | 0 | 0,92 Mpx | 83,0 MP/s |
| 2 | 90,1 i/s (`paralleles-n2.log:68`) | 0 | 1,84 Mpx | 166,1 MP/s |
| 4 | 90,1 i/s (`paralleles-n4.log:94`) | 0 | 3,69 Mpx | 332,1 MP/s |
| **8** | **90,1 i/s** (`paralleles-n8.log:146`) | **0** | **7,37 Mpx** | **664,3 MP/s** |

**90,1 i/s exactement sur les quinze voies des quatre rangs** en capture+encodage
(90,0–90,1 en capture nue), soit **1,50 fois le seuil** au rang du critère. Huit
duplications ouvertes de front (`paralleles-n8.log:78`), huit encodeurs matériels
NVENC construits sans refus (l. 93 à 114), huit périphériques D3D11 tenus pour
distincts (l. 47 à 75, tous `protection_precedente=false` — **inférence** sur la
sémantique de `SetMultithreadProtected`, aucun relevé de huit pointeurs
distincts), topologie rendue **nom pour nom** à son état de départ, zéro
`ERROR`, zéro `WARN`, **aucun rang rejoué**.

Deux réserves que ce tableau ne porte pas, et qu'il ne faut pas perdre en le
recopiant (§6 des résultats) :

- **la restauration « nom pour nom » n'est pas une preuve d'absence d'effet
  résiduel** — elle porte sur l'ensemble des **noms** de sorties attachées, et
  sur rien d'autre : ni mémoire, ni état du pilote, ni ressources DXGI ;
- **la fraîcheur du binaire mesuré n'est adossée à aucune pièce versée.** La
  sortie de `scripts/build-agent.sh` a été recopiée d'un terminal, jamais
  capturée dans un fichier. Vérifiable depuis le dépôt, et rien de plus :
  `git status --porcelain agent/ scripts/` vide à `db8cc85`, et la date du
  binaire sur la VM antérieure de 1 min 40 s à l'horodatage du commit —
  **cohérent** avec un binaire bâti sur ces sources, sans le prouver.

### Ce que ce montage a de neuf : l'aire croît avec N

**Tous les bancs antérieurs de ce projet mesuraient à aire totale fixe.**
`disposition::tuiles` découpe le bureau : le débit de pixels y est quasi constant
*par construction*, et le nombre de fenêtres n'y est **pas prouvé neutre en
soi**. Ici chaque fenêtre a sa sortie de 1280×720, facteur d'échelle 1 (pas de
piège DPI). **Le fait de ce chantier se lit entièrement dans sa propre série** :
l'aire totale est multipliée par 8 de N=1 à N=8 (0,92 → 7,37 Mpx, **relevé**) et
la cadence par fenêtre ne bouge pas (90,1 i/s aux quatre rangs, **relevé**) ; le
débit de pixels correspondant, **calculé**, va de 83,0 à 664,3 MP/s.

⚠️ **Ne rapprocher les deux séries d'AUCUN chiffre**, ni cadence ni débit — la
sonde partage *une* acquisition entre N recadrages d'aire fixe, celle-ci ouvre
*N* acquisitions sur N surfaces constantes. Un débit étant le produit d'une
cadence par une aire, deux séries incommensurables sur les cadences le restent
sur les débits. *(Une première rédaction de cette section affirmait « le débit
passe de 248–258 à 664 MP/s sans que la cadence bouge » : 248–258 vient de la
sonde et non de cette série, la cadence bouge bel et bien d'une série à l'autre
— 107,5 → 90,1 i/s —, et la borne basse de la sonde à N=1, 208 MP/s, était
écartée sans le dire.)*

### Le défaut de libération des encodeurs : corrigé, et deux risques assumés

**Il n'était pas déterministe mais intermittent** (2/6). **`MFShutdown` n'était
pas en cause** : retiré entièrement du chemin, la faute revient (1/5) — sa
présence dans les deux premières piles était fortuite. La faute réelle : **la MFT
NVIDIA a un élément de travail encore en vol** quand on relâche l'encodeur, et il
entre dans un verrou qui n'existe plus (`RtlEnterCriticalSection`, chemin
contendu, `DebugInfo` nul, sur un fil de pool `CSerialWorkQueue`).

**Correctif** (`agent/src/encode/arret.rs`) : une file de travail Media Foundation
**sérialisée par encodeur** imposée à la MFT (`IMFRealTimeClientEx::SetWorkQueueEx`),
puis dépôt d'une **sentinelle** attendue avant tout relâchement — une attente
**bornée sur une condition observable**, pas un délai. Que le travail de la MFT
transite bien par cette file est éprouvé : boucher la file 3 000 ms arrête
l'encodage net pendant exactement cette durée, la capture continuant.
**0 récidive sur 20 exécutions contre 2 sur 6** — *ce n'est pas une preuve
d'absence, et l'énoncé porte toujours son nombre d'exécutions.*

**Deux risques ouverts et assumés** :

- **`IMFShutdown::Shutdown` est non borné dans un `Drop`, et un gel y a été
  OBSERVÉ** (1 fois sur 6 à N=4, processus vivant treize minutes plus tard,
  `2ter-gel-n4-shutdown.log`). **Cause non attribuée** — l'exécution portait
  aussi une file au convertisseur, retirée depuis, et le départage n'a pas été
  fait. **Le retirer n'est pas une option** : sans lui la faute revient 2 fois
  sur 5, barrière pourtant franchie. Arrêt et barrière ne sont pas redondants.

  ⚠️ **Ce risque est PRÉSENT, pas réservé au chantier D.**
  `Drop for H264Encoder` court **déjà en production mono-fenêtre** : à chaque
  changement de barreau de l'adaptation réseau (`set_encode_size`) et à chaque
  redimensionnement (`resize`), sur le fil unique de `Session::run`
  (`spawn_blocking`) — un gel y figerait la session entière. **Borne du pire
  cas par destruction d'encodeur** : `2 × DELAI_BARRIERE + 2 × DELAI_ARRET_MFT`
  = **8 s** de partie bornée (6 s sur les machines éprouvées, le convertisseur
  n'exposant pas `IMFShutdown`), **et rien ne borne le total** — ni les quatre
  `ProcessMessage`, ni les deux `Shutdown()`. Nominal relevé : 0,5 ms par
  encodeur, 4,0 ms pour huit. Les deux traces qui encadrent l'appel sont en
  **`info!`** et non `debug!` — l'exploitation tourne en `RUST_LOG=info`, et une
  mitigation muette n'en est pas une. **Ne pas les redescendre.**
- **Le convertisseur de couleur n'est couvert par rien.** Sans effet tant qu'il
  retombe sur `CLSID_VideoProcessorMFT` (synchrone), mais **sur un hôte doté d'un
  Video Processor matériel ce serait une MFT matérielle sans barrière** —
  configuration qu'aucune machine éprouvée n'expose, donc **non mesurée**.

### Ce que cette mesure NE dit pas

- **Une exécution par rang, donc AUCUN taux** — ni fréquence d'échec, ni
  variabilité des cadences. Le gel de `Shutdown` vu 1/6 à N=4 n'est ni observé ni
  exclu par une exécution unique à N=4.
- **Rien au-delà de 8 sorties, rien entre 4 et 8** : **8 est ce qui a été demandé
  et obtenu, PAS une limite trouvée.** Le plafond de duplications simultanées
  n'est pas mesuré (le vivier de sorties est de 10 : un rang 9 ou 10 serait
  mesurable, il ne l'a pas été).
- **Rien de la latence**, rien d'autres résolutions ou débits (1280×720@60,
  8 Mb/s, passes de 10 s), rien sur une durée longue.
- **La justesse est ÉCHANTILLONNÉE** : contrôle en rotation, une voie par tour,
  soit **~113 lectures par voie à N=8**, pas 901. « Zéro verdict faux » vaut sur
  les 901 lectures effectuées, pas sur les 7 208 images capturées.
- **Les débits de pixels sont CALCULÉS**, pas relevés (le banc journalise des
  cadences et des images, jamais des pixels).
- **Les mires ne sont pas des applications** : D3D11 plein cadre, sans occlusion,
  sans interaction, sans redimensionnement.
- **Aucune unité H.264 n'a été décodée ni regardée.**
- **Ce qui borne la cadence à ~90 i/s n'est pas mesuré** — un plafond juste
  au-dessus et un plafond très au-dessus se liraient pareil ici.
- **Rien du comportement quand Apollo consomme le même vivier de 10.**
- **Le cas d'exploitation réel n'est pas couvert** : le banc crée ses N encodeurs
  d'un coup et les détruit d'affilée à la fin, **jamais un seul pendant que les
  autres encodent** — ce que fera pourtant la fermeture d'une fenêtre.
- **La mise en sommeil des fenêtres masquées reste une conjecture** : « créer 8 →
  en détruire 1 → tenter un 9ᵉ » n'a pas été jouée.
  ✅ **Jouée le 3 août 2026 (D5), conjecture confirmée** : 4 exécutions,
  10 recyclages sur 10 chacune. Voir « Sous-bloc D5 ».
- **Le cas d'exploitation « détruire un encodeur pendant que les autres
  encodent » est, lui, exercé depuis D5** — c'est ce que fait chaque
  endormissement —, mais **sur le chemin du produit et non à ce banc-ci**.

### Pièges neufs — à connaître avant de toucher à ce terrain

- **Ne jamais se fier à la pile du fil principal pour désigner une cause.** Elle
  montrait `MFShutdown` dans les deux vidages ; coïncidence de minutage. Ce qui
  tranche est de **retirer la variable suspecte et de voir si le symptôme
  survit** — cela a coûté une campagne entière.
- **`MFSHUTDOWN_COMPLETED` ne veut pas dire « plus rien en vol ».** Une MFT rend
  cet état en `attente_ms=0` et fait planter le processus quelques instants plus
  tard. Fait acquis, réutilisable.
- **Un défaut intermittent qu'on croit déterministe se déclare corrigé à la
  première exécution qui passe.** Mesurer le taux **avant** de corriger, et lui
  opposer une campagne d'un ordre de grandeur au-dessus.
- **Ne pas totaliser des exécutions qui n'exercent pas la même chose** : seule la
  ligne comparable s'oppose à la référence ; un agrégat est un nombre sans
  référent.
- **Ne pas utiliser `git add -A` dans un arbre partagé** — un `git add -A
  agent/src` a emporté dans un commit le travail concurrent d'une autre tâche,
  sans sa déclaration de module : le commit ne compilait pas. **Nommer les
  fichiers.**
- **Un banc à aire fixe et un banc à aire croissante ne se comparent pas.** Les
  deux séries existent désormais dans ce dépôt.
- **Corriger une affirmation réfutée exige de la CHERCHER, pas de la corriger là
  où on nous l'a montrée.** Les documents longs ont un sommaire, et c'est lui
  qu'on lit : traiter le chapitre de détail en laissant le sommaire intact laisse
  le lecteur repartir avec une tâche déjà faite. Balayer sur les formules
  (« encore due », « non diagnostiqué », « jamais expliqué »…).
  **Corollaire, payé une ronde plus tard : chercher par le SENS, pas par la
  formule.** Ce balayage cherchait « **non** diagnostiqué » ; la phrase qui a
  survécu disait « **pas** diagnostiqué », et c'était la conclusion d'une section
  entière, contredisant l'encadré posé soixante lignes plus haut. Une négation se
  dit de plusieurs façons, et **c'est celle qu'on n'a pas listée qui survit** :
  balayer sur la *chose niée* (un diagnostic, une mesure, une explication) en
  énumérant les tournures — « pas / non / jamais / seulement localisé / sans
  explication / reste ouvert / n'est établi par rien ». Et **annoter
  l'affirmation elle-même, pas sa voisine**.
- **Une clé de lecture posée dans le code doit être vérifiée contre le journal
  avant d'être recopiée.** Le commentaire de `passes.rs` expliquait le rapport
  `unites`/`images` par un ratio 90/60 qui prédisait 600 unités là où le journal
  en montrait **450** — la fermeture arithmétique était juste
  (`images = unites + nv12_ecartees + au plus 1 en vol`, vérifiée aux quatre
  rangs), **l'attribution causale ne l'était pas**. Reformulée en constat ; la
  cause du rapport d'un demi **n'est pas établie**.
- **Corollaire à retenir pour le chantier D** : les 90,1 i/s sont une cadence de
  **capture**, pas d'unités H.264 délivrées — ce montage rend **45 unités par
  seconde et par fenêtre**.

---

## 🪟🌐 Sous-bloc D1 — tranche verticale multi-fenêtres (1ᵉʳ août 2026)

> ✅ **À LIRE AVANT CETTE SECTION — le sous-bloc D2, le même jour, a réparé le
> défaut bloquant de D1, et les SEPT points de suite de son §9 sont clos**
> (cinq par D2, deux par le correctif final de branche `e9691eb`). Les
> affirmations ci-dessous restent le relevé **de D1**, mais celles qui portent
> sur ce qui est possible aujourd'hui sont annotées une à une. Verdict à jour :
> section « Sous-bloc D2 » plus bas.

Résultats complets :
`docs/superpowers/plans/2026-08-01-multifenetres-tranche-verticale-resultats.md`.
Conception : `docs/superpowers/specs/2026-08-01-multifenetres-tranche-verticale-design.md`.
Journaux : `docs/superpowers/plans/journaux-multifenetres-d1/` — **UTF-8 sans
BOM**, avec les séquences ANSI de `tracing` comme les journaux des chantiers
précédents (`sed 's/\x1b\[[0-9;]*m//g'` pour les lire à plat). Le répertoire porte
aussi les pièces qui ne sont pas des journaux d'agent : les captures d'écran des
fenêtres navigateur, la relecture `WM_GETTEXT` des Bloc-notes, l'environnement
du signaling, le relevé des veilles prolongées, et **l'instrument lui-même**
(`pilote-recette.mjs`, versé dans son état final, celui de la dernière
exécution).

**Premier chantier de PRODUIT du modèle multi-fenêtres**, et première exécution
réelle : jusqu'ici la seule preuve était le compilateur et les tests des parties
pures. Le superviseur (`agent/src/superviseur/`) détecte les fenêtres, leur donne
une sortie virtuelle, y pose la fenêtre, lance un enfant par fenêtre, et parle à
une page-shell (`client/src/shell.ts`) qui ouvre une fenêtre navigateur par
fenêtre Windows.

### Le verdict : ça marche, et ça ne tient pas

**Acquis, vérifié en session réelle** : une fenêtre navigateur par fenêtre
Windows, chacune sur sa sortie virtuelle, chacune capturée et encodée par **son
propre processus**, chacune montrant **son** application et elle seule, plein
cadre à 1280×720, RTT 1–3 ms — **jusqu'à quatre simultanées**, sur de vraies
applications (Bloc-notes, Explorateur, Firefox) et non des mires.
⚠️ **Ces quatre fenêtres PRÉEXISTAIENT au démarrage du superviseur** : ce sont
celles que l'énumération initiale trouve. **Le cas produit — un utilisateur
ouvre une application — a été tenté deux fois et a échoué deux fois.** D1 sait
éclater un bureau tel qu'il est ; il ne sait pas en accueillir une de plus.

> ✅ **D2 sait en accueillir une de plus** : montées 1→2, 2→3 et 3→4 propres, en
> conditions de produit, sur de vraies applications. La restriction « fenêtres
> préexistantes » est **levée**. Ce qui la remplace est un plafond de **quatre**
> fenêtres simultanées, d'une autre nature (§ « Sous-bloc D2 »).
>
> ⚠️ **Et D3 a rendu la restriction inverse obligatoire (2 août 2026)** : le
> garde-fou d'attente de viewport s'applique désormais à **toutes** les entrées,
> y compris préexistantes. **Si la page-shell se connecte plus de 30 s après le
> superviseur, les fenêtres préexistantes sont abandonnées** — et jamais
> reproposées. « Lancer le navigateur AVANT le superviseur » n'est plus un
> conseil. Le plafond de quatre est par ailleurs **caractérisé** par D3 : il
> porte sur le nombre de **processus** (§ « Sous-bloc D3 »).

Le son est porté par **une seule** fenêtre (+59 710 octets RTP audio en 9,4 s sur elle
seule, les trois autres sessions n'ayant aucune piste audio). Aucune sortie n'a
fuité : ensemble des **noms** de sorties identique au départ aux trois contrôles
depuis un processus neuf, superviseur pourtant tué net à chaque fois.

**Bloquant** : **créer une sortie virtuelle fait abandonner le mutex des
duplications DXGI déjà ouvertes** (`0x887A0026`, « Le mutex indexé a été
abandonné »). Toute nouvelle fenêtre tue donc **toutes** les sessions en cours,
et l'emballement qui suit vide la page-shell alors que les applications Windows
sont toujours là. **Reproduit sur trois exécutions versées sur trois, plus une
quatrième dont les journaux ne sont pas joints.** La correspondance est exacte : sur les **17**
créations de sortie des trois journaux, **13 n'ont aucune duplication ouverte
→ 0 erreur** (9 parce qu'aucun enfant n'a encore été lancé, 4 entre deux
vagues), et **4 en ont → 9 erreurs**, soit `1, 1, 3, 4`. Ce `1, 1, 3, 4` est le
nombre d'**enfants qui capturent**, PAS le nombre de lignes `duplication de
sortie établie` — dans F il y en a douze pour quatre enfants et quatre erreurs. ⚠️ **La DESTRUCTION d'une
sortie n'est pas mise en cause : le cas n'a jamais été exercé** — les
dix-sept destructions des trois journaux tombent toutes hors de toute
duplication ouverte. D1 **n'est pas reçu**.

> ✅ **Corrigé par D2, et la destruction a depuis été exercée.** L'abandon du
> mutex se produit toujours — il n'est ni évité ni expliqué — mais il est
> **encaissé** : la duplication est relâchée puis rouverte dans une fenêtre de
> reprise, et 44 pertes d'accès n'ont tué aucune session. **La destruction d'une
> sortie abandonne le mutex elle aussi** (relevé sous duplication ouverte,
> aucune création intercalée), et la reprise l'encaisse également. Ce paragraphe
> reste le relevé exact de D1 ; il ne décrit plus le comportement du dépôt.

### Quatre défauts à connaître avant de toucher à ce terrain

> ✅ **Les quatre sont corrigés par D2** (nom DXGI partout ; garde `sur_sortie`
> en tête de `resize` ; viewport arrondi en pair côté client ; appariement
> tolérant à 4 px et attente **sur condition observable** au lieu d'un délai
> plat). Le diagnostic ci-dessous garde sa valeur — c'est pourquoi il reste —
> mais **ne pas repartir de ces quatre points comme s'ils étaient ouverts**.
> ⚠️ Un piège s'y est ajouté depuis : **le pilote QUANTIFIE la résolution
> demandée** (1280×632 demandé → sortie 1280×720), ce que 4 px de tolérance ne
> rattrapent pas. ⚠️ **Et D8 a établi que ce n'est pas seulement de la
> quantification : une sortie naît à la DERNIÈRE TAILLE LAISSÉE AU REGISTRE, pas
> à celle demandée** — voir le piège correspondant de la section D2 plus bas et
> la section « Sous-bloc D8 ».

- **`(index_adaptateur, index_sortie)` n'est PAS un identifiant de sortie.** Il
  est positionnel et change dès qu'une sortie apparaît ou disparaît. Le
  superviseur le passe pourtant à l'enfant, qui le résout plus tard : d'où des
  `Error: aucune sortie DXGI à l'index adaptateur 0, sortie 5`. Le `nom_sortie`
  (`\\.\DISPLAYn`) est le seul identifiant stable, et le superviseur l'a déjà.
- **`WindowsSource::resize` ignore le mode `sur_sortie`.** Il redimensionne la
  fenêtre Windows et reconstruit une duplication du **bureau** : hors écran,
  donc `la fenêtre est hors de l'écran`, capture de secours sur le bureau
  physique, et `soumission à l'encodeur échouée … NV12` **une fois par image**.
  En mode « une sortie par fenêtre », il n'y a rien à redimensionner.
- **Une hauteur de viewport impaire rend toute fenêtre impossible.** Le pop-up
  d'un navigateur annonce couramment une hauteur impaire (1280×**713** mesuré).
  `resize` force les dimensions paires (`& !1`) : la taille demandée ne peut
  alors jamais égaler celle de la source. **Arrondir le viewport avant de créer
  la sortie.**
- **`DELAI_RATTACHEMENT = 1500 ms` n'est pas toujours suffisant**, et
  l'appariement par égalité stricte de dimensions échoue alors : sortie créée à
  1280×713, rendue par DXGI à 1280×720 une fois, à 1280×713 l'essai suivant.
  **Ce n'est pas le facteur DPI de 1,5** que les documents redoutaient, c'est une
  course.

### Ce que D1 devait relever et n'a PAS relevé

- **Le plafond d'encodeurs en multi-processus.** Pas approché : **4 encodeurs
  NVENC construits de front dans 4 processus, aucun refus** ; 6 sorties
  virtuelles attachées simultanément, aucun refus du pilote non plus. Le 8
  connu reste un chiffre de **processus unique**.
- **L'injection clavier**, ni démontrée ni réfutée. Deux causes possibles,
  non départagées : côté navigateur le premier clic est consommé par
  `requestPointerLock` (activation utilisateur, qu'un clic CDP ne fournit pas
  sans interface) ; côté agent **`SendInput` est global à la session Windows** —
  le clavier va à la fenêtre au premier plan, et aucun `SetForegroundWindow`
  n'est fait. Le second point est **structurel** et vaudra quel que soit le
  premier.
  ✅ **D2 l'a démontrée** : `SetForegroundWindow` est désormais posé avant
  injection (retour vérifié : 4 succès, 0 refus), et les quatre Bloc-notes qui
  ont une session ont reçu **chacun sa propre frappe**, pas le cumul. **Portée
  exacte** : une frappe par fenêtre, sonde **séquentielle**, aucune frappe
  concurrente — `SendInput` **reste global à la session Windows**, et ce relevé
  ne dit rien de deux utilisateurs frappant en même temps.
- Rien de la latence ni de la cadence, 3 applications seulement, session vivante
  la plus longue ≈ **50 s**, une exécution exploitée par configuration.
  ⚠️ **Toujours vrai après D2** — latence et cadence n'ont pas davantage été
  mesurées, et le **plafond d'encodeurs en multi-processus** n'a pas été
  approché non plus (4 encodeurs de front dans 4 processus, la mort survenant
  avant tout encodeur au 5ᵉ).

### ⚠️ La VM se met en veille prolongée toute seule — deux mesures perdues

**Deux horodatages, et l'écart entre eux est réel** : l'invité amorce la
transition à **09:40:04 et 10:40:04 UTC** (Kernel-Power 187/42), QEMU n'est
terminé qu'à **09:40:09 et 10:40:10** — les ~5 s d'écriture de l'image
d'hibernation. Ce n'est pas une minuterie d'inactivité (`STANDBYIDLE` et
`HIBERNATEIDLE` sont à 0) : le journal Windows nomme l'initiateur,
`\Windows\System32\shutdown.exe` (Kernel-Power **187**), pour une transition de
type hibernation (Kernel-Power **42**). Relevé versé :
`journaux-multifenetres-d1/veille-prolongee-vm.txt`.
⚠️ **Le motif horaire n'est PAS établi** : le journal libvirt porte **quatre**
extinctions sur la journée — 08:29:55, 09:40:09, 10:40:10 et 11:30:27 UTC —
soit des intervalles de **70, 60 puis 50 minutes**, dont deux seulement sur la
minute :40. **Le déclencheur exact n'est pas identifié** — la
seule tâche planifiée appelant `shutdown` est désactivée depuis avril 2025 ;
`sunshine`/`sunshinesvc` tournent et sont des suspects **non éprouvés**. La
seule règle prudente : **vérifier que la VM a survécu après toute séquence
longue**, plutôt que de se fier à une fenêtre horaire.

```bash
# Symptômes : /media/vm répond « L'hôte cible est arrêté ou en panne »,
# virsh list --all dit « fermé », run-agent.sh échoue en écrivant son .ps1.
grep -E "terminating on signal|shutting down" /var/log/libvirt/qemu/Windows.log | tail -4
```

### Pièges d'outillage rencontrés

- **`scripts/run-agent.sh` ne transmettait pas `SUPERVISEUR`** — corrigé. Sans
  cette ligne, l'agent démarre en mode mono-fenêtre sans rien signaler.
- **Un agent SURVIT à l'hibernation de la VM**, et `run-agent.sh` ne tue pas
  l'agent existant : il recrée la tâche et la lance. **Vérifier
  `Get-Process agent` avant chaque exécution**, sinon on mesure le processus
  précédent.
- **Sans `--disable-popup-blocking`, la démonstration est vide et muette** : la
  shell ouvre ses fenêtres hors geste utilisateur, Chrome les refuse toutes, et
  la seule trace est un message dans la page-shell.
- **Chrome sans interface survit à la mort de son pilote.** Une exécution
  entière a été perdue parce que le pilote s'est attaché à une instance
  résiduelle, sur un port réutilisé, dont les pages périmées ne recevaient plus
  rien. **Un port de débogage qui répond ne prouve pas que c'est le bon
  navigateur.**
- **Une capture d'écran CDP suffit à déclencher l'effondrement** : elle provoque
  un `Resize`, qui rétrécit la fenêtre Windows, qui engendre un `SHOW`, qui crée
  une session, qui crée une sortie, qui tue toutes les captures.
  **L'instrument détruisait ce qu'il mesurait** — même leçon que la trace par
  paquet du chantier TURN, sous une autre forme.
  ⚠️ **Depuis D2, le dernier maillon ne tue plus rien** (la reprise l'encaisse),
  mais **la chaîne demeure entière jusque-là** : une capture CDP provoque
  toujours un `Resize`, donc un `SHOW`, donc une session et une sortie de plus.
  **La contrainte de protocole tient** : pas de capture d'écran pendant une
  mesure. Et une autre raison s'y ajoute — toute évaluation CDP sur une page
  portant un flux WebRTC actif peut **ne jamais rendre** (voir la section D2).
- **Le signaling ne mémorise que les offres SDP** : les annonces
  `fenetre-ouverte` émises avant que la page-shell ne soit connectée sont perdues
  sans trace. **Lancer le navigateur AVANT le superviseur.**
- **`agent.log` mêle le superviseur et tous ses enfants** (stdout hérité), sans
  rien qui distingue l'émetteur hors le champ `session=` de certaines lignes.

---

## 🪟🔁 Sous-bloc D2 — arrangement multi-fenêtres dynamique (1ᵉʳ août 2026)

Résultats complets :
`docs/superpowers/plans/2026-08-01-multifenetres-arrangement-dynamique-resultats.md`.
Conception : `docs/superpowers/specs/2026-08-01-multifenetres-arrangement-dynamique-design.md`.
Journaux : `docs/superpowers/plans/journaux-multifenetres-d2/` — **UTF-8**, avec
les séquences ANSI de `tracing` (`sed 's/\x1b\[[0-9;]*m//g'` pour lire à plat).
**Une exception d'encodage** : `build-agent-11bis.log`, dont les lignes revenant
du PowerShell distant sont mutilées (voir les pièges plus bas).

D2 ne fait qu'une chose : lever ce qui empêchait D1 d'être reçu.

> ✅ **À LIRE AVANT CETTE SECTION — le sous-bloc D3 (2 août 2026) a clos les
> trois points de suite du §7 que D2 laissait, et caractérisé le plafond de
> quatre.** La recréation de sortie à chaque relance est supprimée (0 réouverture
> imputable à une relance, mesuré) ; la fuite de capacité est fermée ;
> `CAPACITE` est passée de 8 à **4**. Le plafond porte sur le nombre de
> **processus** concurrents tenant une duplication, **pas** sur le nombre de
> duplications. ⚠️ **La couche qui l'impose reste inconnue.** Les affirmations
> ci-dessous restent le relevé **de D2** ; celles que D3 réfute ou complète sont
> annotées une à une. Verdict à jour : section « Sous-bloc D3 » plus bas.
>
> ✅ **Et le sous-bloc D4 (2 août 2026) a MUTUALISÉ la capture, ce qui retire ce
> plafond de quatre : un seul processus y tient 8 duplications et 8 encodeurs, et
> sa SECONDE recette relève 8 fenêtres qui DIFFUSENT simultanément.**
> `CAPACITE` est repassée à 8. ⚠️ **Ce 8 n'a été confronté à aucune mesure de
> plafond** : la montée s'y arrête, donc sur le produit lui-même, et n'approche
> aucun plafond du système — 8 tient, rien ne dit que 9 ne tiendrait pas.
> *(La première recette de D4 ne faisait rien diffuser du tout ; le canal a été
> réparé par la tâche 10.)* Voir la section « Sous-bloc D4 » plus bas.

### Le verdict est DOUBLE — ne le simplifier dans aucun sens

**① Le défaut central de D1 est réparé, et démontré réparé en conditions de
produit.** Créer une sortie virtuelle ne tue plus les captures en cours : **44**
pertes d'accès `0x887A0026` encaissées sur le passage décisif, **aucune session
perdue**, montées 1→2, 2→3 et 3→4 propres, **une seule** ligne `clôture de
session amorcée` sur tout le passage — et elle est **sollicitée** (0,7 s après un
`WM_CLOSE` réel). Sur de vraies applications, pas des mires.

**② Le critère de réception exigeait CINQ fenêtres simultanées ; on en atteint
QUATRE.** La 5ᵉ duplication DXGI, **dans un 5ᵉ processus**, est refusée en
`0x887A0022` (`DXGI_ERROR_NOT_CURRENTLY_AVAILABLE`) par une limite de
**concurrence** qui **résiste à trois secondes de patience explicite** — ce
qu'aucune mesure antérieure n'avait éprouvé. **La couche qui l'impose n'est pas
identifiée**, et **rien n'établit que 4 soit une borne du système**.

> ✅ **D3 a caractérisé cette limite** : elle porte sur le nombre de
> **processus** concurrents tenant une duplication, et vaut **exactement 4** —
> huit duplications tiennent sans peine dès lors qu'elles sont réparties sur au
> plus quatre processus. ⚠️ **Les deux réserves de ce paragraphe TIENNENT
> INTÉGRALEMENT** : la couche qui l'impose n'est **toujours pas** identifiée, et
> **rien n'établit toujours que 4 soit une borne du système**.

**Donc D2 n'est pas reçu au sens de son critère, et il répare pourtant ce pour
quoi il existait.**

> ✅ **Le critère de CINQ fenêtres simultanées est atteint, et dépassé, par la
> seconde recette de D4 (2 août 2026) : HUIT fenêtres diffusent ensemble** — non
> pas en levant la limite de concurrence, mais en la **contournant**, toutes les
> duplications vivant désormais dans un processus unique. ⚠️ **La limite de
> quatre processus n'est ni levée ni expliquée** : elle a seulement cessé d'être
> rencontrée.

### La cause, trouvée au bout de TROIS mesures dont deux réfutations

`rouvrir()` demandait une seconde duplication de la même sortie **sans avoir
relâché la première**. DXGI n'autorise qu'une duplication ouverte par sortie
(doctrine déjà portée par ce fichier) : l'appel **réussissait** et rendait un
objet **mort-né**, qui reperdait son accès aussitôt.

| Tirage | Ce qui changeait | Tentatives/voie | `images_apres` | Verdict |
| --- | --- | --- | --- | --- |
| 1 | 3 tentatives sans délai | 3 (budget entier, brûlé en 14–21 ms) | 0 | RÉFUTÉ |
| 2 | fenêtre 8 s, pas 150 ms | **54** (le maximum théorique) | 0 | RÉFUTÉ |
| 3 | **relâche puis acquiert** | **1** | `[892]`/`[889,888]`/`[884,883,883,882]` | **REÇU** |

**Une exécution par rang à chaque tirage : aucun taux.** Entre le 2ᵉ et le 3ᵉ,
une seule variable de comportement a changé — l'attribution causale est propre
sous cette réserve.

Délai perturbation → réouverture, **relevé** : **+48 ms** (k=1), **+70/+81 ms**
(k=2), **+105 à +144 ms** (k=4). La fenêtre de 8 s consomme donc **1 tentative
sur 54** : elle est **très surdimensionnée pour le cas mesuré**, mais **aucun cas
lent n'a été observé** — **ne pas la réduire sur la foi de ce seul relevé.**

> **Leçon de méthode, chère :** deux réfutations coûteuses ont été closes non par
> un tirage de plus mais par la **relecture d'une ligne**. Quand deux mesures
> successives réfutent une hypothèse **sans que le symptôme change de forme**,
> relire le chemin avant de recalibrer.

### Trois acquis que D1 déclarait ouverts

- **Le clavier atteint chaque fenêtre séparément.** `SetForegroundWindow` avant
  injection, retour vérifié (4 succès, 0 refus) ; les quatre Bloc-notes qui ont
  une session ont reçu **chacun sa frappe**, la cinquième — sans session — rien.
  ⚠️ **Portée exacte** : une frappe par fenêtre, sonde séquentielle, aucune
  frappe concurrente. **`SendInput` reste global à la session Windows.**
- **La DESTRUCTION d'une sortie abandonne le mutex elle aussi**, et la reprise
  l'encaisse. D1 déclarait le cas « jamais exercé » ; il l'est (destruction,
  puis deux pertes d'accès à +22 et +25 ms, **aucune création intercalée**).
- **L'ordre `relâcher/acquérir` était la cause.** La piste du **périphérique
  D3D11 conservé**, formulée après la 2ᵉ réfutation, devient **SANS OBJET — et
  non pas « réfutée »** : elle n'a **jamais** été mise à l'épreuve, elle n'a
  simplement plus rien à expliquer. Elle reste disponible si un symptôme voisin
  réapparaissait.

### La suite à donner, nommée précisément

1. **Ne plus détruire puis recréer la sortie virtuelle à chaque relance
   d'enfant** — c'est **la vraie cause des 32 réouvertures parasites**. Une
   fenêtre condamnée fait passer le compteur de 6 à 38 à elle seule. Le réessai
   à l'ouverture ajouté pour cela n'en a supprimé **aucune** : **aucun de ces
   échecs n'était un transitoire** (quatre séquences, fenêtre entière courue,
   zéro reprise réussie). Aucun réessai ne peut rien contre cette cause-là.
   **C'est la première chose à corriger.**
   ⚠️ **Les « 44 avant / 44 après » sont des comptes ARRÊTÉS à la fin de la
   séquence de critère, pas des totaux de fichier.** Le journal versé de la
   seconde exécution va plus loin (phase clavier, captures, arrêt) et porte
   **50** réouvertures en fin de fichier ; le 44 comparable s'y relit par une
   borne temporelle explicite. Ne jamais opposer un total de fichier à un compte
   fenêtré.
   ✅ **TRAITÉ par D3 (2 août 2026).** La sortie virtuelle est désormais
   **retenue** entre la mort d'un enfant et sa relance : sur la recette du
   critère, **un seul** `sortie virtuelle créée` et **un seul** `détruite`
   encadrent **quatre** lancements successifs de la même fenêtre condamnée, et
   la fenêtre bornée porte **0** réouverture imputable à une relance et **0**
   session saine perdue. ⚠️ **Une seule exécution retenue, aucun taux**, et
   **aucune image n'a été comptée** — voir « Sous-bloc D3 ».
2. **Identifier la couche du plafond de quatre.** Ce n'est ni le plafond de
   sorties virtuelles (10 : `création de sortie refusée` = **0** sur tout le
   passage — les neuf créations y sont **successives**, pas simultanées, et ne
   borneraient rien par elles-mêmes), ni celui des encodeurs NVENC (8 : la mort
   survient **avant** tout encodeur, et **aucune pièce de D2 ne compte
   d'encodeurs**). Le
   rapprochement avec les **8 duplications d'un seul processus** du 31 juillet
   est une **INFÉRENCE** — rien ici ne l'établit. Fermer une fenêtre puis en
   rouvrir une réussit : la place libérée suffit.
   ⚠️ **PARTIELLEMENT TRAITÉ par D3, et il faut lire la nuance.** D3 a supprimé
   l'inférence : une campagne de 15 exécutions établit que le plafond porte sur
   le nombre de **processus** concurrents tenant une duplication, et vaut
   **exactement 4** — le rang qui échoue a **moins** de duplications ouvertes
   que ceux qui passent. **Mais la COUCHE reste inconnue**, et c'était l'objet
   littéral de ce point : il est **caractérisé, pas résolu**.
   ⚠️ **Le PRODUIT des points 1 et 2 n'est écrit nulle part ailleurs, et c'est
   lui qui coûte.** `CAPACITE = 8` (`superviseur/boucle.rs`) est désormais connu
   **supérieur au plafond mesuré de 4** : le superviseur accepte donc quatre
   fenêtres qui ne peuvent pas aboutir. Chacune échoue au bout de ses
   `RELANCES_MAX = 3` relances, soit **quatre tentatives** (l'originale plus
   trois) ; et chaque tentative **détruit puis recrée** une sortie virtuelle —
   la recréation étant précisément ce que le point 1 identifie comme la cause
   des réouvertures parasites infligées aux sessions **saines**. Les fenêtres 5
   à 8 déclenchent ainsi **jusqu'à 16 cycles création/destruction**, et de
   l'ordre de **128 abandons de mutex** sur les sessions qui fonctionnent —
   **pour zéro chance de succès**. Le 16 se dérive du code (4 fenêtres × 4
   tentatives) ; le 128 est un **ordre de grandeur** (16 recréations × les
   duplications alors ouvertes), **pas un relevé** : aucune exécution de D2 n'a
   dépassé 4 fenêtres. **Ni `CAPACITE` ni le comportement n'ont été changés en
   fin de branche** — ce serait une décision de conception, et le remède réel
   est celui du point 1.
   ✅ **D3 a pris cette décision de conception : `CAPACITE` vaut désormais 4**
   (`superviseur/boucle.rs`), et le remède du point 1 est appliqué par ailleurs.
   Les 16 cycles et les ~128 abandons de mutex décrits ci-dessus n'ont donc plus
   de cause. **La phrase « n'ont été changés en fin de branche » ne décrit plus
   le dépôt.**
3. **Une fuite de capacité reste ouverte** pour une fenêtre **neuve** dont la
   page-shell ne répond **jamais** : ni relancée ni abandonnée, elle consomme sa
   place indéfiniment. **Défaut préexistant, pas introduit par D2** ; le
   garde-fou (`DELAI_ATTENTE_VIEWPORT_MAX = 30 s`, majorant non calibré) a été
   volontairement borné aux entrées **relancées**. Remède proposé par la revue :
   tamponner `attente_depuis` **paresseusement** au premier passage de
   `relancer_les_orphelines`, ce qui garde `Table` pure et ne bouge aucun
   appelant.
   ✅ **TRAITÉ par D3**, exactement par ce remède : le tampon est posé
   paresseusement, `Table` reste pure et aucun appelant n'a bougé. ⚠️ **Effet de
   bord assumé** : le garde-fou couvre maintenant aussi les fenêtres
   **préexistantes**, qui sont abandonnées si la page-shell tarde plus de 30 s —
   et une entrée abandonnée n'est **jamais reproposée** (défaut préexistant,
   nommé et non corrigé).

### Ce que D2 n'établit PAS

Une exécution par rang au banc, **une exécution exploitée par configuration** à
la recette : **aucun taux, nulle part**. Rien au-delà de 4 fenêtres. Le
**mécanisme de l'abandon du mutex reste inconnu** — on sait le traiter, pas
l'expliquer. Rien de la latence, de la cadence, de la durée (session la plus
longue ≈ 4 min 30 s). **Plafond d'encodeurs en multi-processus toujours pas
approché.** Aucun redimensionnement, aucun recouvrement, aucun déplacement de
fenêtre. Le chemin `resize` n'est **pas exercé** après le correctif
`new_sans_attente`. La branche `est_ouverture_retentable(ACCES_PERDU)` n'a
**jamais** été exercée à l'ouverture. **Le chemin d'extinction propre du
superviseur n'a jamais été exercé** (arrêt net par `schtasks /end`).

⚠️ **D3 n'a levé AUCUN de ces points**, hors le « rien au-delà de 4 fenêtres »
qui est désormais caractérisé plutôt qu'étendu. Le mécanisme de l'abandon du
mutex reste inconnu, le plafond d'encodeurs en multi-processus n'a toujours pas
été approché, et rien de la latence, de la cadence ni de la durée n'a été
mesuré.

⚠️ **D4 n'en a levé que DEUX** : « rien au-delà de 4 fenêtres » est étendu à
**8 fenêtres qui diffusent** dans le processus capteur unique (seconde recette),
et **la cadence est désormais mesurée** — 58,3 i/s par fenêtre à N = 4, 494,4 i/s
cumulées au capteur à N = 8. **Tout le reste tient intégralement** : mécanisme de
l'abandon du mutex toujours inconnu, plafond d'encodeurs en multi-processus
toujours pas approché (D4 n'a plus qu'un seul processus qui encode), rien de la
**latence** ni de la **durée**, aucun redimensionnement, aucun recouvrement,
aucun déplacement de fenêtre, chemin d'extinction propre du superviseur toujours
jamais exercé.

### Pièges neufs — à connaître avant de toucher à ce terrain

- **Le pilote de sortie virtuelle QUANTIFIE la résolution demandée** : 1280×632
  demandé rend une sortie **1280×720**, soit 88 px d'écart, très au-delà de la
  tolérance d'appariement de 4 px. **Aucune fenêtre ne s'ouvre alors, et rien ne
  le dit hors du journal d'agent.** Imposer au navigateur une taille que le
  pilote rend à l'identique.
  ⚠️ **CE PIÈGE EST PLUS PROFOND QUE « QUANTIFICATION », et D8 l'a établi
  (5 août 2026, tâche 3bis) : une sortie NE NAÎT PAS À LA TAILLE DEMANDÉE, mais
  à la DERNIÈRE TAILLE LAISSÉE AU REGISTRE** par un `CDS_UPDATEREGISTRY`
  antérieur — chaîne `avant(N) = après(N-1)` vérifiée sur trois transitions
  consécutives, **confirmée et reproduite, jamais expliquée** ⚠️ (sur **un seul
  journal brut versé**, donc une exécution : voir la réserve rétablie en
  section D8). La taille passée
  à la création est purement et simplement ignorée par le pilote.
  **Conséquence opérationnelle mesurée** : la préparation de la recette D8 a
  trouvé le produit **entièrement bloqué** — le superviseur créait et détruisait
  des sorties en boucle, chacune refusée par `sortie créée mais introuvable dans
  la topologie DXGI`, parce que le registre était resté à 2560×1440 d'une mesure
  antérieure alors que `superviseur/boucle.rs` exige une correspondance exacte
  avec 1280×720. **Une pollution de registre laissée par une sonde ne fausse pas
  seulement la sonde suivante : elle bloque le produit.** Remède employé : la
  sonde P1 elle-même (`MULTIFENETRE_MODE_SORTIE=1280x720`), en préparation.
- **Toute évaluation CDP sur une page portant un flux WebRTC actif doit être
  BORNÉE.** `Page.captureScreenshot` peut ne **jamais** rendre ; une relecture de
  `window.__console` s'y est figée de la même façon.
- **`scripts/run-agent.sh` ne transmettait pas `MULTIFENETRE_REPRISE`** — même
  piège que `SUPERVISEUR` en D1. **Toute variable neuve du banc doit y être
  ajoutée explicitement**, sinon l'agent démarre sans elle et sans rien signaler.
  ✅ **Piège évité en D3 (`MULTIFENETRE_PLAFOND`) et en D6 (`BUDGET_BPS`, tâche 9
  dédiée à cette seule ligne)** : il vaut aussi pour les variables de **produit**,
  pas seulement pour celles du banc.
- **`build-agent.sh` ne pose pas `[Console]::OutputEncoding`** : les lignes
  revenant du PowerShell distant en reviennent mutilées (« Au caract⏎re
  Ligne:1 »). C'est le **défaut à deux réglages** déjà documenté plus haut. Le
  script est partagé : signalé, **non corrigé**.
  ⚠️ **Toujours non corrigé au 3 août 2026, et il a mutilé les journaux de
  pilote de D6** : ceux-ci portent des octets de contrôle isolés à la place des
  accents (`Op\x02ration r\x02ussie`) et se classent en « data ». Un `grep` sur
  un mot accentué y rend zéro.
- **Un journal d'agent s'écrase facilement**, et deux pièces ont été perdues
  ainsi dans ce sous-bloc — dont celle qui aurait étayé une affirmation qu'il a
  fallu retirer. **Copier le journal avant tout relevé qui écrit au même
  endroit.**
- **`pkill -f <motif>` depuis un shell dont la ligne de commande contient le
  motif tue le shell lui-même** (exit 144, la suite de la chaîne ne s'exécute
  pas). Tuer par PID relevé.
- **Paint ouvre DEUX fenêtres éligibles** (`Paint` + `UIRibbonWorkPane`) : sur
  une recette qui compte des fenêtres, n'employer que des applications à fenêtre
  unique — ou compter les fenêtres, jamais les lancements.
- **Un défaut du CODE fourni par un plan doit être signalé, pas recopié.** Une
  tâche a implémenté verbatim un code de brief qui rendait **muet le tout premier
  refus** — exactement le cas que la trace existait pour révéler. La clause
  « signaler un défaut du plan » vaut aussi pour un **bug** dans le code fourni,
  pas seulement pour une divergence de spécification.

---

## 🪟🔢 Sous-bloc D3 — retenir les sorties, et caractériser le plafond de concurrence (2 août 2026)

Résultats complets :
`docs/superpowers/plans/2026-08-02-multifenetres-plafond-concurrence-resultats.md`.
Conception : `docs/superpowers/specs/2026-08-02-multifenetres-plafond-concurrence-design.md`.
Journaux : `docs/superpowers/plans/journaux-multifenetres-d3/` — **37 fichiers
suivis par git** (24 au premier niveau, plus 13 dans `instrument/`), **UTF-8**,
avec les séquences ANSI de `tracing` (`sed 's/\x1b\[[0-9;]*m//g'` pour lire à
plat ; `agent-critere1.txt` est déjà mis à plat).

D3 prend trois points du §7 de D2 — ne plus recréer la sortie à chaque relance,
identifier ce sur quoi porte le plafond de quatre, et accorder `CAPACITE` — et
écarte le reste.

### Le verdict — les deux critères sont TENUS

**① La sortie virtuelle est RETENUE d'une relance à l'autre.** Sur une séquence
où une fenêtre est condamnée à répétition pendant que trois autres capturent,
fenêtre bornée `09:06:44.639142Z` → `09:06:48.784516Z` : **0** réouverture de
duplication imputable à une relance, **0** session saine perdue. Un **seul**
`sortie virtuelle créée id=257` et un **seul** `détruite id=257` encadrent
**quatre** `enfant lancé … sortie=\\.\DISPLAY8`. La recréation de sortie que D2
désignait comme la cause des 32 réouvertures parasites **n'a plus lieu** — c'est
un fait de code, relevé une fois en conditions de produit ; **une seule exécution
retenue, aucun taux**.

**② Le plafond de quatre porte sur le nombre de PROCESSUS, et vaut exactement 4.**
15 exécutions, 5 rangs × 3 essais, sondes **minimales** (aucun encodeur, aucune
fenêtre, aucun WebRTC) :

| Rang | P × D | Duplications ouvertes | Issue |
| --- | --- | --- | --- |
| témoin | 1 × 8 | 8 | **OK 3/3** |
| A | 2 × 4 | 8 | **OK 3/3** |
| B | 4 × 2 | 8 | **OK 3/3** |
| contrôle | 4 × 1 | 4 | **OK 3/3** |
| C | 8 × 1 | **4 au moment du refus** | **KO 3/3**, sonde 4 (5ᵉ processus), `0x887a0022` |

**Le témoin passe, donc H2 est réfutée** : le plafond ne tient pas au fait que
le **créateur** des sorties soit un autre processus que le duplicateur — c'est
exactement le montage de `1x8`, et il passe.

**Le fait le plus tranchant, à ne pas perdre en recopiant ce tableau** : le rang
qui **échoue** n'a que **4** duplications ouvertes au moment du refus, quand des
rangs qui **réussissent** en ont **8**. Il en a donc **moins** que ceux qui
passent — ce n'est pas une absence de corrélation, c'est une **exclusion
positive** du nombre total de duplications comme cause. Et `4x2` et `8x1` créent
le **même** nombre de sorties virtuelles (`nombre=9 attachees=9` dans les deux) :
entre ces deux rangs, **seul le nombre de processus diffère**.

Le refus est un **état atteint**, pas une extrapolation : aux trois essais de
`8x1`, la ligne `verdict reçu sonde=4` précède **toutes** les lignes
`arrêt demandé, relâchement des duplications` des sondes 0 à 3 — les quatre
premières tiennent encore leur duplication quand la cinquième est refusée.

### La décision d'arrangement, et `CAPACITE`

La règle de décision était écrite **avant** la mesure (conception §3.5). H1
confirmée, sa ligne s'applique sans arbitrage :

- **la capture mutualisée** — un seul processus tenant les N duplications et
  distribuant les textures — est **DÉSIGNÉE pour D4**, et **non implémentée** ;
  ✅ **D4 l'a implémentée (2 août 2026), et sa recette a mesuré qu'UN SEUL
  processus tient bien 8 duplications DXGI et 8 encodeurs NVENC.** Le plafond de
  quatre **processus** cesse donc de mordre. ✅ **Et D4 EST REÇU sur ses trois
  critères, à la SECONDE recette** (`ddf2915`) : 8 fenêtres diffusent, tuer le
  capteur ne tue aucune session, la cadence est relevée des deux côtés. *(La
  première recette échouait sur les trois — le canal capteur→enfant ne délivrait
  pas sa réponse d'attache ; réparé par la tâche 10.)* ⚠️ **Un défaut ouvert
  demeure** : à 8 fenêtres, l'adaptation par la résolution est refusée 18 fois
  sur 18. Voir « Sous-bloc D4 » plus bas.
- **`CAPACITE` passe de 8 à 4** (`agent/src/superviseur/boucle.rs`). À 8, le
  superviseur acceptait quatre fenêtres dont aucune ne pouvait aboutir, chacune
  brûlant `RELANCES_MAX + 1` tentatives dont chacune recréait une sortie
  virtuelle. **Valeur MESURÉE sur cette VM, non prouvée être une borne du
  système.**
  ⚠️ **Cette phrase ne décrit plus le dépôt : D4 a reporté `CAPACITE` à 8**, par
  coïncidence avec le plafond d'encodeurs connu. **Ce 8 n'a été confronté à
  aucune mesure de PLAFOND** — les deux recettes de D4 s'y arrêtent, donc sur le
  produit lui-même, sans approcher aucune borne du système. ⚠️ **Il a en
  revanche été confronté à une mesure de FONCTIONNEMENT, et il ne la passe qu'à
  moitié** : à 8 fenêtres, la reconfiguration d'encodeur (`set_encode_size`) est
  refusée 18 fois sur 18, alors qu'elle réussit 3 fois sur 3 à 2 fenêtres. **8
  fenêtres se capturent et se diffusent, mais elles ne s'adaptent plus par la
  résolution.**
  ✅ **Ces deux phrases ne décrivent plus le dépôt (3 août 2026, D5).**
  `CAPACITE` vaut **10**, et ce 10 n'est plus « le plafond d'encodeurs » : les
  deux plafonds viennent désormais de couches différentes — `CAPACITE = 10` du
  **vivier de sorties du pilote** (mesuré 10 le jour même de la recette, refus à
  la 11ᵉ en `0x80070044`), `vivier::PLAFOND_EVEIL = 8` du **matériel
  d'encodage**. Et **l'adaptation par la résolution fonctionne de nouveau à
  8 fenêtres** : 198 changements acceptés, 0 refusé. ⚠️ **Ce 10 n'a pas
  davantage été confronté à un plafond du système que le 8 ne l'était** : c'est
  la constante du produit qui refuse la 11ᵉ fenêtre, le pilote n'étant jamais
  sollicité pour elle.

⚠️ **Précision de vocabulaire, pour que D4 ne se trompe pas de repli.** Ce que la
conception de D3 nomme « le repli de la spec §8 » est la **capture mutualisée**.
Le §8 de la conception de **D2** décrit sous « gardé en réserve » un mécanisme
**différent** : *sérialiser* (le superviseur fait relâcher, crée, fait rouvrir).
Même case, pas la même chose — **c'est la mutualisation que D3 désigne**.

### Acquis d'outillage qui dépasse ce sous-bloc — la compilation croisée Windows

**Le dépôt vérifie désormais son code `#[cfg(windows)]` sur l'hôte Linux.**
mingw-w64 est installé ; depuis `agent/` :

```bash
cargo check --target x86_64-pc-windows-gnu
```

Relevé le 2 août 2026 : **sortie 0, 9 avertissements `dead_code`, aucun dans
les fichiers neufs**. *(Ils ne sont pas tous préexistants : l'un d'eux vise
`taille_sortie_de`, accesseur ajouté par D3 et employé par les seuls tests —
voir le §5 des résultats. La phrase disait « préexistants » ; le §5 la
corrigeait déjà, sans que cette occurrence-ci soit balayée.)*

⚠️ **Portée exacte** : cela couvre **types, emprunts, visibilités et durées de
vie** ; cela **ne couvre PAS l'édition de liens**, la cible réelle du projet
étant `msvc` sur la VM. Ce n'est donc pas un substitut à
`scripts/build-agent.sh`. C'est en revanche la levée, **en grande partie**, de la
réserve « non vérifiable sur l'hôte » qui pesait sur tout le code `#[cfg(windows)]`
depuis le début du projet — **à employer avant toute compilation distante.**

### Un changement de comportement au démarrage, assumé

Le garde-fou `DELAI_ATTENTE_VIEWPORT_MAX = 30 s` (majorant **non calibré**)
s'applique désormais à **toutes** les entrées en attente de viewport, et plus
seulement aux entrées relancées : c'est ainsi que la fuite de capacité du §7.3 de
D2 est fermée. Conséquence : **si la page-shell se connecte plus de 30 s après le
superviseur, les fenêtres préexistantes sont abandonnées** — et une entrée
abandonnée n'est **jamais reproposée**, le hook ne réémettant rien pour une
fenêtre déjà ouverte (**défaut préexistant**, nommé et non corrigé). La règle
« lancer le navigateur AVANT le superviseur » cesse d'être un conseil.

### Ce que D3 n'établit PAS

- **La couche qui impose le plafond n'est TOUJOURS pas identifiée** — Windows,
  DXGI, pilote NVIDIA, SudoVDA, virtualisation. D3 répond à *sur quoi porte* le
  plafond, jamais à *qui l'impose*.
- **Rien n'établit que 4 soit une borne du système** : c'est le point d'arrêt
  observé sur cette VM, à 1280×720 / 60 Hz.
- **VM mono-GPU, une seule sortie physique** : rien d'un plafond par adaptateur,
  aucune série virtuel/physique.
- **Aucune image capturée, aucun encodeur construit** par les sondes. **Le
  plafond d'encodeurs en multi-processus reste entièrement ouvert** — et il
  devient le **risque n°1 de D4**.
  ⚠️ **Ce risque N'A PAS été levé par D4, et il ne s'est pas non plus manifesté**
  — parce qu'il n'a pas été approché : D4 mutualise la capture dans **un seul**
  processus, où 8 encodeurs se construisent sans refus, et ses recettes
  s'arrêtent à 8 sur `CAPACITE`. **Le plafond d'encodeurs en MULTI-processus
  reste entièrement ouvert**, exactement comme cette ligne le dit. ⚠️ **Le
  plafond MONO-processus, lui, s'est manifesté d'une autre façon** : à 8
  encodeurs vivants, en **reconstruire** un (ce que fait `set_encode_size`) est
  refusé au même `SetOutputType` — voir « Sous-bloc D4 ».
- **Un rang `5x1` autonome n'a pas été joué** (ni `6x1`, ni `7x1`). L'état qu'il
  aurait mesuré a bien été atteint et refusé 3/3 **comme sous-produit de `8x1`**,
  mais il manque une mesure dédiée. Le comportement entre 6, 7 et 8 processus est
  inconnu : l'escalier s'arrête au premier refus.
- **Le sens de « processus » n'est pas creusé** : 8 **fils** dans un seul
  processus n'a pas été mesuré.
- **Trois essais par rang à la campagne, mais UNE SEULE exécution retenue à la
  recette du critère 1** — aucun taux de ce côté.
- **Aucune image n'a été comptée pendant la recette.** « Les sessions saines
  tournent sans interruption » vaut sur ce que le journal établit — processus
  vivants, ICE établi, aucune `clôture de session amorcée` — mais leurs sorties
  ont bien perdu leur mutex à répétition (`DISPLAY5` **4** fois, `DISPLAY6` 3,
  `DISPLAY7` 2), chacune rouverte du premier coup. **Aucune trame comptée.**
- **La condamnation ne frappe qu'un enfant tout juste lancé** : la mort survient
  100 à 204 ms après le lancement aux quatre tours. **La mort d'un enfant qui
  capture depuis longtemps, en pleine diffusion, n'est pas exercée.**
- **Le contrôle de topologie final est affaibli par une hibernation de VM
  intercalée** : il prouve qu'aucun artefact ne survit à long terme, **pas** que
  le chemin de libération a fonctionné — cette preuve-là est ailleurs, et
  antérieure au redémarrage (`détruite id=257` puis `rendue au pilote`).
- **Trois allocations TURN ont échoué** pendant la recette (cause non
  investiguée) : elle s'est jouée sans relais, sur candidats `host`.
- **Rien de la latence, de la cadence, de la durée** (journaux de campagne :
  6,42 s à 9,69-9,70 s), aucun redimensionnement, aucun recouvrement, aucun
  déplacement de fenêtre, aucune charge d'encodage réelle (Bloc-notes statique).

### Pièges neufs — à connaître avant de toucher à ce terrain

- ⚠️ **La sonde dite « minimale » n'est pas nue** : elle porte
  **inévitablement** un `ID3D11Device` avec `SetMultithreadProtected(true)`,
  `DuplicateOutput` l'exigeant. L'étage « ajouter un périphérique D3D11 » de
  l'escalade prévue si H3 s'était vérifiée est donc **déjà franchi par
  construction**. Ce qui est exclu du montage, c'est l'**encodeur**, la fenêtre et
  la session WebRTC — pas le périphérique.
- **`nodejs-winrm` enveloppe TOUJOURS la commande** dans
  `powershell -Command "& { … }"` (`usePowershell=true` dans `scripts/winrm.js`).
  Un script inline portant des guillemets doubles entre en collision avec cette
  enveloppe, et **le symptôme est un script qui ne tourne jamais** — pas une
  erreur claire. Écrire le script sur le partage et l'invoquer par `-File`.
- **`scripts/run-agent.sh` ne transmet pas les variables neuves** — piège payé en
  D1 (`SUPERVISEUR`) et en D2 (`MULTIFENETRE_REPRISE`), évité ici en ajoutant
  `MULTIFENETRE_PLAFOND` **dans la même tâche** que le mode.
- **Ne jamais interpoler une valeur d'environnement brute dans un composant de
  chemin** : `..` est significatif sous Windows. Une ronde de correction l'avait
  introduit, rattrapé à la suivante — **une correction peut introduire une casse
  neuve**.
- **Le porteur hérite ses variables aux sondes**, et `MULTIFENETRE_VDD_PURGE` y
  aurait détruit les sorties du porteur **en pleine mesure**. L'environnement des
  enfants se nettoie explicitement.
- **Ne pas chasser le compte absolu d'avertissements `clippy`** : il dérive d'une
  exécution à l'autre selon la fraîcheur du build (99 puis 100 relevés par deux
  relecteurs). Ce sont tous des `dead_code` dus au `#[cfg(windows)]`. **Vérifier
  la nature, jamais le nombre.**
- **Attendre le FAIT, jamais une durée.** Le brief prévoyait `sleep 45` par rang ;
  un polling sur deux lignes de fait a montré que la durée réelle est de 6,4 à
  9,7 s.
- **Le chien de garde du pilote se fait attendre davantage à mesure que le rang
  monte** : `intervalle_ping_max_ms` va de **372 ms** (`1x8`) à **1012 ms**
  (`8x1`), monotone — et ce 1012 est obtenu sur un escalier **arrêté à 5 sondes
  sur 8**. **L'unité du `delai = 3` du pilote reste inconnue** : à surveiller.
- **La fraîcheur du binaire mesuré n'est adossée à aucun horodatage versé** — elle
  se déduit d'une trace introduite par un commit connu, présente dans les quinze
  journaux. **Cohérent, non prouvé.**

### Les deux variables d'environnement neuves

`MULTIFENETRE_PLAFOND=<P>x<D>` (le porteur) et `MULTIFENETRE_PLAFOND_SONDE`
(la sonde, posée par le porteur et jamais à la main) sont décrites dans le
**tableau des variables du banc**, section « Mesures préalables au chantier D »
plus haut — un seul tableau, pour qu'il n'y ait qu'un endroit à consulter.

---

## 🧩 Sous-bloc D4 — capture mutualisée : huit fenêtres diffusent (2 août 2026)

Résultats complets :
`docs/superpowers/plans/2026-08-02-multifenetres-capture-mutualisee-resultats.md`.
Conception : `docs/superpowers/specs/2026-08-02-multifenetres-capture-mutualisee-design.md`.
Journaux : `docs/superpowers/plans/journaux-multifenetres-d4/` — **36 fichiers,
tous en UTF-8, séquences ANSI déjà retirées** : ils se `grep`ent à plat, sans
`sed` (contrairement à ceux de D1, D2 et D3). **Deux recettes y cohabitent** :
la première sans préfixe, la seconde à préfixe `r2-`.

> ⚠️ **CE SOUS-BLOC A ÉTÉ RECETTÉ DEUX FOIS, ET LA SECONDE RENVERSE LA
> PREMIÈRE.** La recette n°1 (tâche 9, binaire `f2b3fbe`) a échoué sur les trois
> critères et **diagnostiqué pourquoi** ; la tâche 10 a réécrit la couche de
> transport du canal (`716eda9`, `ddf2915`) ; la recette n°2 (tâche 11, binaire
> `ddf2915`) **tient les trois critères**. Tout ce qui suit jusqu'au titre
> « ✅ La seconde recette » est le relevé **de la première**, conservé pour son
> diagnostic, et annoté là où il ne décrit plus le dépôt.

D4 remplace « un processus par fenêtre » par trois étages : le **superviseur**
(qui ne duplique rien), un **capteur** unique qui tient les N duplications DXGI
et les N encodeurs et sert chaque enfant par un tube nommé
(`\\.\pipe\agent-capteur`), et les **N enfants** réduits à WebRTC, l'entrée et
l'audio. `WindowsSource` n'est ni déplacé ni modifié ; une `SourceDistante`
implémente `VideoSource` côté enfant.

### Le verdict est DOUBLE, et il ne se simplifie dans aucun sens

**① La thèse de D4 est soutenue au niveau de la capture.** Un **unique**
processus capteur (`capteur lancé pid=18212`) a tenu **8 duplications DXGI** sur
huit sorties virtuelles nommément distinctes (`\\.\DISPLAY5` à `\\.\DISPLAY12`)
et **8 encodeurs matériels NVENC**, **zéro `WARN`, zéro `ERROR`**, pendant que
huit enfants journalisaient chacun `source distante servie par le capteur`. **Le
plafond de quatre PROCESSUS établi par D3 cesse donc de mordre.**

**② D4 N'EST PAS REÇU : aucune session WebRTC ne s'établit, sur aucun rang.**
Les trois critères sont non tenus. Le nombre de pages navigateur qui **diffusent
réellement** (`framesDecoded` en croissance) vaut **0** aux rangs 1 à 8.

> ✅ **CE POINT ② N'EST PLUS VRAI.** La seconde recette (2 août 2026, binaire
> `ddf2915`) relève **8 fenêtres qui diffusent simultanément**, `DIFFUSENT=8` au
> rang 8. **Les trois critères sont tenus.** Voir « ✅ La seconde recette » plus
> bas.

**Ces deux faits ne se compensent pas.** Le premier porte sur des
**constructions** réussies — aucune image n'a été comptée sur ces huit voies, et
les huit encodeurs n'ont **jamais été alimentés ensemble**.

> ✅ **CETTE RÉSERVE EST LEVÉE.** Les huit encodeurs d'un unique processus ont
> été **alimentés ensemble** par la seconde recette : **494,4 i/s cumulées**
> (51,6 à 66,2 i/s par fenêtre sur huit sessions, `r2-critere1-montee-agent.log`),
> et le navigateur a décodé de vraies images H.264 sur les huit voies. **Cela
> lève du même coup la réserve de la mesure ② du 31 juillet 2026** — « aucune
> image soumise, seule la construction est mesurée » — pour le cas du
> périphérique D3D11 unique et partagé.

### Le défaut bloquant, et ce qu'on en sait exactement

> ✅ **CE DÉFAUT EST CORRIGÉ** (tâche 10, `716eda9` + `ddf2915`) : **deux
> connexions par fenêtre**, une par sens (A = média, le capteur écrit et
> l'enfant lit ; B = commandes, stricte alternance sur un seul fil à chaque
> bout), plus un **fil écrivain dédié par fenêtre** côté capteur et une **borne
> de 12 s** sur l'attente d'une réponse de commande. Démontré corrigé en
> conditions de produit par la seconde recette. ⚠️ **Le MÉCANISME du défaut
> reste inconnu** : la correction le rend impossible par construction, elle ne
> l'explique pas, et l'asymétrie relevée ci-dessous n'est **toujours pas**
> expliquée. **Ne jamais réintroduire un fil lecteur sur la connexion de
> commandes** — c'est la doctrine que ce diagnostic laisse.

**L'écriture du capteur sur le tube n'aboutit pas tant que son fil de lecture de
commandes a une lecture bloquante pendante sur la même instance de tube.**
L'enfant reste donc bloqué dans `lire_trame` en attendant sa réponse d'attache,
n'atteint jamais le signaling, et aucune session ne naît.

C'est un fait **différentiel**, pas une conjecture : sur un binaire par ailleurs
identique, empêcher ce fil d'entrer en lecture fait aboutir l'attache des **deux**
côtés en 34 µs, passer l'ICE à `connected`, et arriver de vraies images H.264
1280×720 au navigateur (`defaut-canal-3` et `-4`).

⚠️ **Le MÉCANISME reste inconnu, et l'explication naturelle est CONTRARIÉE.**
La sérialisation des E/S sur un objet fichier synchrone Windows expliquerait le
côté capteur, mais **côté enfant l'écriture aboutit** alors que son propre fil
lecteur est en lecture bloquante — on le lit à ce que `commander` échoue sur son
*délai de réponse* (« aucune réponse du capteur à `Debit { … }` »), libellé
attaché au seul `recv_timeout`. **Il y a une asymétrie que rien n'explique.**

**Le remède n'est pas un correctif** : E/S recouvrantes (`FILE_FLAG_OVERLAPPED`
aux deux bouts, `ConnectNamedPipe` compris) ou une connexion par sens — dans les
deux cas de la conception, sur la couche même que le sous-bloc a bâtie.

### Un second défaut, celui-là corrigé (commit `f2b3fbe`)

La réponse `Attachee` était écrite dans un `BufWriter` **sans être vidée**, là où
toutes les autres écritures du fichier l'étaient. Elle ne partait donc qu'à la
première **image** — or Desktop Duplication n'émet qu'au changement du bureau, et
un Bloc-notes immobile n'en produit aucune. **Leçon générale : une réponse de
protocole se vide à l'écriture, jamais en pariant sur le trafic qui suit.**

Ce correctif est juste **et n'a rien débloqué** : le blocage s'est déplacé en
amont, et une ligne de journal qui s'affichait avant a **cessé** de s'afficher.
**Une trace qui disparaît après un correctif est une information.**

### Ce que la recette a mesuré du critère 2 — et le zéro à ne pas lire de travers

Capteur tué **par PID relevé** à 4 fenêtres. La **supervision** tient : relance
en **≈ 0,54 s** (`capteur mort, relancé pid_mort=28328 pid_neuf=19640`), et les
quatre sorties virtuelles sont **réemployées** (rétention acquise en D3). Mais
**les quatre enfants meurent à l'instant même**, en `ExitStatus(1)`, et sont
remplacés par quatre sessions **neuves** (`w-2,4,6,10` → `w-11,12,13,14`).

⚠️ **`clôture de session amorcée` vaut 0 sur la fenêtre de l'épreuve, et ce zéro
est VIDE DE SENS** : aucune session n'était établie, il n'y avait rien à clore.
⚠️ **Le chemin de reprise de `SourceDistante` (fenêtre de 15 s + `rattacher`)
n'a pas été ATTEINT** — ni validé, ni invalidé : les enfants étaient bloqués
avant d'entrer dans la boucle de transport.

> ✅ **Le chemin de reprise A ÉTÉ ATTEINT ET EXERCÉ** par la seconde recette,
> sur quatre sessions qui diffusaient réellement : rattachement en **538 à
> 689 ms**, **0** enfant terminé, **0** clôture de session, **0** session neuve.
> Le zéro n'est plus vide de sens.

### Ce que D4 n'établit PAS

> ⚠️ **Liste de la PREMIÈRE recette.** La seconde en lève plusieurs points et en
> laisse plusieurs entiers ; la liste à jour est sous « ✅ La seconde recette ».

**Deux exécutions de recette, une par critère : aucun taux, nulle part.** Aucun
plafond du système approché (la montée s'arrête sur `CAPACITE = 8`, une constante
du produit, affichée à la shell comme « plus aucune sortie virtuelle
disponible » ; **aucun `HRESULT` de refus n'apparaît**). Aucune unité H.264
décodée ni regardée. **Le critère 3 n'a pas de relevé du tout** — et le « avant »
sur `7d7e254` n'a pas été joué, faute d'« après » auquel l'opposer. Rien de la
latence, de la durée, du redimensionnement, du recouvrement, de l'audio, de
l'injection clavier. Une seule application (Bloc-notes) et **immobile**.

### Pièges neufs — à connaître avant de toucher à ce terrain

- **Une fenêtre IMMOBILE ne produit aucune image.** Le piège des mires du banc
  frappe ici le chemin de production : toute recette qui veut des images doit
  **animer sa source**. Ce protocole ne l'a pas fait.
- **Un `BufWriter` transforme une poignée de main en pari sur le trafic** (§
  ci-dessus).
- **Un diagnostic qui change deux choses n'établit rien** : un tirage où le fil
  lecteur dormait 25 s rendait aussi les commandes sans réponse, ce qui affamait
  `Session::run` à 10 s par commande et cassait la session pour une raison
  étrangère à la question. Versé pour mémoire, **exclu du raisonnement**.
- **Un Bloc-notes fait avancer le compteur de sessions de DEUX** (`w-2, w-4,
  w-6 …`) : une seconde fenêtre éligible et fugace est détectée par lancement.
  C'est la leçon de Paint sous une autre forme — **compter les fenêtres, jamais
  les lancements.**
- **Le journal du PILOTE n'est pas de l'UTF-8 sans précaution** : les lignes que
  `run-agent.sh` renvoie de PowerShell portent des octets de contrôle isolés
  (`Op\x02ration r\x02ussie`), qui font classer le fichier en « data ». Défaut à
  deux réglages déjà connu, ici sur le chemin de l'**hôte** et non de la VM.
- **La VM s'est hibernée EN PLEINE MESURE** (`18:05:41Z`), une exécution perdue,
  et **trois processus `agent` ont survécu au redémarrage**. Les deux pièges
  documentés, rencontrés tels quels dans la même journée.

### Marge étroite neuve à surveiller

`agent/src/capteur/distante.rs` est à **487 lignes** (marge **13**) — fichier
neuf de D4, qu'aucun document ne signalait. `agent/src/superviseur/boucle.rs`
est à **491** (la conception annonçait 485) et `agent/src/demarrage.rs` à **472**
(elle annonçait 468). Les trois lignes du tableau de dette sont inchangées.

> ⚠️ **Le remède du canal N'EST PAS passé par `distante.rs`** : la tâche 10 a
> porté sur `capteur/tube.rs`, `capteur/serveur.rs`, `capteur/fenetre.rs` et
> `capteur/protocole.rs`. `distante.rs` est **inchangé, toujours à 487 lignes**,
> et la marge de 13 reste à surveiller pour la même raison. Le remède du défaut
> **neuf** trouvé par la seconde recette touchera, lui, `encode.rs` (1536, dette
> gelée) ou `capteur/fenetre.rs` (329).
>
> ❌ **Les trois `487` de cette section sont FAUX, et l'étaient déjà le jour où
> ils ont été écrits.** Relevé du 3 août 2026 (sous-bloc D5), par la commande :
> `agent/src/capteur/distante.rs` fait **235 lignes**, ses tests vivant à part
> dans `agent/src/capteur/distante/tests.rs` (385). Le 487 comptait le fichier
> **avant** cette extraction, faite en fin de D4, et personne ne l'a repris
> après. **Il n'y a jamais eu de « marge de 13 » à surveiller sur ce fichier**,
> et la vigilance qu'appelait cette section portait sur un fichier qui n'était
> pas menacé. Le remède du défaut neuf n'a finalement touché ni `encode.rs` ni
> `capteur/fenetre.rs` mais `windows_source.rs`, qui en est **maigri** (648 →
> 631).
>
> ⚠️ **Les trois chiffres de cette annotation-ci ont vieilli à leur tour**
> (relevé par la commande le 3 août 2026, vague de correction finale de D6) :
> `capteur/distante.rs` vaut **288** (et non 235), `distante/tests.rs` **409**
> (et non 385), `capteur/fenetre.rs` **407** (et non 329), et
> `windows_source.rs` est remonté de 631 à **638**. **Une annotation qui
> corrige un nombre périmé vieillit exactement comme le nombre qu'elle
> corrigeait** : la seule défense reste la commande, jamais la recopie.

---

## ✅ La seconde recette de D4 — les trois critères sont tenus (2 août 2026)

Binaire mesuré : `ddf2915`, **9 035 264 octets**. Journaux à préfixe `r2-`.
**Quatre corrections de protocole** que la première recette s'était prescrites :
source **animée**, ceinture éprouvée, survie de la VM contrôlée **après chaque
rang**, et le « avant » du critère 3 réellement joué.

| Critère | Verdict | Le chiffre, **relevé** |
| --- | --- | --- |
| 1 — dépasser quatre fenêtres diffusant | **TENU** | **8 fenêtres diffusent simultanément** ; refus au rang 9 par `CAPACITE = 8` |
| 2 — tuer le capteur ne tue aucune session | **TENU** | **0** enfant terminé, **0** clôture, **4/4** rattachements en 538 à 689 ms |
| 3 — cadence avant / après | **MESURÉ** | **61,76 → 58,30 i/s** par fenêtre à N = 4 (moyennes calculées) |

⚠️ **Une exécution par critère : AUCUN TAUX, nulle part.**

### La source doit BOUGER, et c'est ce qui change tout

La première recette employait le Bloc-notes, **immobile** : Desktop Duplication
n'émet une trame qu'au changement du bureau. La seconde emploie une fenêtre
**Chrome `--app`** sur une page `canvas` animée par `requestAnimationFrame`,
mesurée à **90,0 Hz de rAF** sur la VM (`instrument/anim-d4.html`,
`instrument/preparer-d4b.ps1`). **Un `--user-data-dir` par fenêtre est
obligatoire**, sans quoi Chrome rejoint son instance existante et l'on compte
des lancements au lieu de fenêtres.

### Le fait central : un seul processus tient huit voies vivantes

Au rang 8 : **1** `capteur lancé`, **8** `sortie virtuelle créée`, **8**
`enfant lancé`, **8** `fenêtre attachée au capteur`, **0** `clôture de session`,
**0** `ERROR`, et **28** pertes d'accès `0x887a0026` toutes encaissées par la
reprise de D2 — dans un **seul** processus, ce qui n'avait jamais été mesuré.

Le capteur produit **494,4 i/s cumulées (calculé)** et le compteur de l'enfant
s'apparie au sien **à moins de 0,2 i/s près sur les huit voies** : **le trajet
IPC ne perd rien de mesurable.** Le navigateur, lui, ne décode que 224,9 i/s
cumulées au palier de 30 s — l'écart est **en aval du canal** (huit sessions à
~10 Mb/s sur le même pont, RTT relevé montant de 2 ms à 104 ms). **Cette
attribution au réseau est une INFÉRENCE** : aucune mesure de charge du pont.

> ❌ **CETTE INFÉRENCE EST RÉFUTÉE (3 août 2026, sous-bloc D6, tâche 1).** La
> mesure de charge du pont qui manquait a été prise : **il porte ≥ 1,44 Gb/s en
> TCP** (3 exécutions, minimum relevé 1 448,74 Mb/s) **et 1,64 Gb/s en UDP**
> (**1 exécution sur 3 — la seule instrumentée au noyau** : les deux autres
> émettent 1 615 et 1 665 Mb/s côté VM, mais **rien n'établit leur arrivée**),
> soit **15 à 27 fois** les 96 Mb/s que huit fenêtres à 12 Mb/s pouvaient
> cumuler. **Le pont
> ne pouvait pas être saturé.** Et `packetsLost` vaut **0** — pas « négligeable »,
> zéro — aux quatre exécutions du banc de décrochage de D6 comme aux sept
> exécutions de sa recette.
>
> **Le goulot est le DÉCODEUR DU NAVIGATEUR.** Témoin relevé sur **une** fenêtre
> à 82,257 Mb/s : le navigateur jette **53,6 %** des images reçues en passant
> **90,2 % du temps mural dans le seul décodage vidéo**, pendant que le réseau
> perd 0,0413 % des paquets. ⚠️ **Portée** : ce montage a une fenêtre là où D4 en
> avait huit — que la chute *de D4* soit due au décodeur est **cohérent avec** ce
> relevé, pas **prouvé par** lui. Ce qui est établi sans réserve, c'est que **la
> saturation du pont est exclue**. Voir « Sous-bloc D6 ».
>
> ⚠️ **Et le décodage LOGICIEL est une propriété du montage de recette**
> (`--disable-gpu` sur un Chrome sans interface), **pas du produit** : un
> navigateur à décodage matériel n'a été éprouvé ni en D4, ni en D6.

### La ceinture répond : `commander` rend une erreur, il ne suspend pas

C'est ce que la doc de `Canal::commander` (`agent/src/capteur/tube.rs`) laissait
explicitement à la recette. 96,7 ms après la relance du capteur :

```
WARN agent::transport::adaptation: l'encodeur refuse le réglage du débit à chaud
  erreur=Le canal de communication est sur le point d’être fermé. (os error 232)
```

`os error 232` = `ERROR_NO_DATA`. La parade prévue — le capteur ferme ses
handles en mourant, la lecture **échoue** au lieu de se suspendre — fonctionne
sur le chemin réel. ⚠️ **Portée** : une seule mise à mort, et la commande fautive
a été émise **après** la mort, pas pendant son vol — le cas d'une commande en
vol à l'instant exact n'a pas été isolé. ⚠️ **La borne de 12 s côté capteur
n'a PAS été exercée** : aucune expiration au journal, c'est du code jamais couru.

### ⚠️ Le défaut NEUF : à 8 fenêtres, l'adaptation par la résolution est morte

**18 changements de taille d'encodage refusés, 0 réussi**, sur la montée du
critère 1 — le premier **0,56 s après l'attache de la huitième fenêtre** :

```
changement de taille d'encodage refusé, barreau conservé
  erreur=le capteur a refusé : configuration du type de sortie de l'encodeur
  H.264 (transform matériel): … (0xC00D6D76)
```

`0xC00D6D76` = `MF_E_UNSUPPORTED_D3D_TYPE` au `SetOutputType` — **exactement
l'appel et le code du refus du 9ᵉ encodeur relevés le 31 juillet 2026.**

**Épreuve différentielle, même binaire, 2 fenêtres au lieu de 8** (sous
`netem adsl`, pour forcer une descente de barreau) : **3 succès, 0 refus**, le
navigateur rapportant bien des trames `640x360`. Le chemin `set_encode_size` à
travers le capteur **fonctionne**.

Lecture : **reconfigurer un encodeur en construit transitoirement un neuf**, et
à 8 encodeurs vivants le neuvième est refusé. ⚠️ **Deux variables diffèrent
entre les deux exécutions** (le nombre de fenêtres et la dégradation `netem`) —
l'attribution tient parce que `netem` détermine *si* le contrôleur demande un
changement, jamais si l'encodeur l'accepte, et que l'erreur est un refus de
**construction**. **La couche qui impose le 8 reste inconnue.**

**Conséquence produit** : à `CAPACITE = 8`, la seule réponse à la congestion qui
reste est le **débit**, la résolution étant gelée — la moitié du dispositif du
chantier C volet 1 disparaît au rang maximal, signalée par un seul `WARN`.
**C'est le seul défaut ouvert de D4**, et la première chose à traiter en D5.

> ✅ **CE DÉFAUT EST MORT (3 août 2026, sous-bloc D5).** `set_encode_size`
> **détruit l'ancien encodeur avant d'en construire un neuf**
> (`agent/src/windows_source.rs`), et ne demande donc plus un neuvième encodeur
> transitoire au matériel. Relevé en conditions de produit, à huit fenêtres
> éveillées : **198 changements de taille acceptés, 0 refusé**, sur trois
> exécutions complètes (53 / 42 / 103). ⚠️ **Le prix du remède est réel et n'a
> PAS été exercé** : si la construction du neuf échoue, l'ancien n'est plus là et
> la source s'épuise — aucun refus n'ayant eu lieu, ce chemin n'a jamais couru.
> ⚠️ **La couche qui impose le plafond de 8 reste inconnue** : D5 la contourne,
> comme D4 contournait celui de quatre processus.

### Ce que la seconde recette n'établit PAS

**Aucun taux, nulle part.** **Le plafond suivant côté capture n'est toujours pas
nommé** : la montée s'arrête sur `CAPACITE = 8`, une constante du produit — 8
tient, rien ne dit que 9 ne tiendrait pas. **La couche du plafond de 8 encodeurs
reste inconnue.** **Le mécanisme du défaut de canal de la première recette reste
inconnu.** **Aucune unité H.264 décodée hors du navigateur ni regardée** : la
justesse de l'image n'est pas contrôlée, seul `framesDecoded` l'est. Rien de la
**latence**, rien de la **durée** (session la plus longue < 5 min), aucun
redimensionnement, aucun recouvrement, aucun déplacement, aucun clavier, aucun
audio. Une seule application et une seule animation. **La mort d'un enfant
pendant que les autres diffusent n'est pas exercée**, ni la fermeture d'une
fenêtre en cours de diffusion. Le **chemin d'extinction propre du superviseur**
n'a toujours jamais été exercé.

### Pièges neufs — à connaître avant de toucher à ce terrain

- ⚠️ **Un `rsync -a` qui remonte le temps fait qu'un `cargo build` ne bâtit
  RIEN, et le dit comme un succès.** Après un aller-retour de sources sur la VM
  (jouer un commit antérieur puis revenir), les sources HEAD sont **plus
  anciennes** que les artefacts du commit joué, et cargo rend
  `Finished release profile in 0.13s` **en gardant le binaire précédent**. Un
  `touch` depuis l'hôte n'y change rien (CIFS), un `touch` depuis Windows non
  plus. **Seul `cargo clean --release -p agent` débloque.** **Vérifier la
  TAILLE du binaire après tout aller-retour de sources ; une compilation de
  0,13 s est un aveu.**
- **`build-agent.sh` lancé depuis un `git worktree` s'arrête EN SILENCE après
  « sources synchronisées »** : le worktree n'a pas de `node_modules`, donc
  `scripts/winrm.js` échoue et son `2>/dev/null` mange la cause — le **même**
  mode de défaillance que le `.env` non sourcé. Remède : lier `node_modules`
  dans le worktree.
- **Le statut de refus de la page-shell ne porte que le DERNIER refus**, et il
  nommait ici une fenêtre PowerShell fugace plutôt que la fenêtre demandée.
  **Croiser avec le relevé des fenêtres de la VM**, qui montre la neuvième
  fenêtre restée sur le bureau physique à `+130+130` quand les huit autres sont
  tuilées à droite.
- **Copier `agent.log` APRÈS la fin réelle de l'exécution, pas à la fin du
  pilote** : les enfants meurent quand le navigateur se ferme, donc **après** la
  copie, et leurs lignes de libération partent avec le journal suivant. Une
  pièce a été perdue ainsi.
- **Un compteur de cadence côté enfant qui s'apparie à celui du capteur est le
  bon instrument pour DISCULPER un canal** — c'est lui qui autorise à dire que
  la perte est en aval.
  ⚠️ **Vrai, et insuffisant : « en aval » n'est pas « dans le réseau ».** D6 a
  mesuré le lien (≥ 1,44 Gb/s, `packetsLost` = 0) et le décodeur (90,2 % du temps
  mural en décodage vidéo, 53,6 % d'images jetées sur une fenêtre) : **l'aval
  incriminé était le NAVIGATEUR**, pas le pont. **Disculper un maillon ne désigne
  pas le coupable suivant** — il faut une mesure par maillon.
- **Une source animée doit l'être à une cadence CONNUE, affichée par la source
  elle-même** : sans ce chiffre, une capture lente et une source lente se lisent
  pareil.

---

## 🛏️ Sous-bloc D5 — le vivier d'encodeurs, et la mise en sommeil (3 août 2026)

Résultats complets :
`docs/superpowers/plans/2026-08-02-multifenetres-vivier-encodeurs-resultats.md`.
Conception : `docs/superpowers/specs/2026-08-02-multifenetres-vivier-encodeurs-design.md`.
Journaux : `docs/superpowers/plans/journaux-multifenetres-d5/` — **UTF-8,
séquences ANSI déjà retirées** : ils se `grep`ent à plat, sans `sed`, comme ceux
de D4. L'instrument y vit aussi (`instrument/`), versé dans son état final.

D5 traite comme **un seul sujet** ce que D4 laissait en deux : le défaut ouvert
de D4 (à 8 fenêtres, `set_encode_size` refusé 18 fois sur 18) et la mise en
sommeil des fenêtres masquées. Les deux reposaient sur **la même conjecture**,
ouverte depuis le 30 juillet 2026 : *détruire un encodeur libère-t-il la place ?*

### ① La mesure pivot : la conjecture est CONFIRMÉE

`MULTIFENETRE_NVENC_CYCLES=10`, dans **l'arrangement de production** (un
processus, **un périphérique D3D11 par encodeur** — ni le mode `partage` ni le
mode `separe` du banc de juillet). **Quatre exécutions, 10 recyclages sur 10
chacune** : monter jusqu'au refus du 9ᵉ (au `SetOutputType`, `0xC00D6D76`, le
même appel et le même code que le 31 juillet), puis dix fois « détruire un, en
construire un ».

**Le plafond de 8 porte sur la CONCURRENCE, pas sur les créations cumulées.** Le
cycle répété est ce qui le prouve : un seul recyclage n'aurait pas distingué les
deux, et le vivier, qui recycle par construction, aurait déclenché la seconde
panne en production.

⚠️ **Ce que la mesure pivot ne dit pas** : la couche qui impose le 8 (inconnue
depuis le 30 juillet 2026), rien d'autres résolutions ni débits, **aucune image
soumise**, **aucune duplication DXGI ouverte** par ce banc.

### ② Le produit : dix fenêtres ouvertes, huit qui diffusent

Le navigateur annonce visibilité et focus sur le data channel
(`client/src/visibilite.ts`) ; l'enfant relaie au capteur ; un **vivier LRU pur**
(`agent/src/capteur/vivier.rs` — aucun `cfg`, aucun objet COM, entièrement
testé) décide qui dort ; le fil de fenêtre relâche ou reconstruit son
`WindowsSource`. **La sortie virtuelle n'est jamais touchée** : c'est ce qui rend
le sommeil sans effet sur les fenêtres voisines — aucun abandon de mutex n'est
provoqué par un endormissement.

Relevé en conditions de produit, sur de vraies fenêtres Chrome animées :

| Critère | Verdict | Le chiffre, **relevé** |
| --- | --- | --- |
| C1 — dépasser huit fenêtres | **TENU sur le fond** | **10 fenêtres ouvertes, 8 qui diffusent, 2 figées** ; LRU = les deux plus anciennes ; les **deux** raisons de sommeil exercées et observées |
| C1 — refus du rang 11 par le PILOTE | **NON TENU** | le refus vient de `CAPACITE = 10`, constante du produit, annoncée à la page-shell |
| C2 — le défaut de D4 est mort | **TENU** | **198 changements de taille acceptés, 0 refusé** (53 / 42 / 103 sur trois exécutions), contre 18 refus sur 18 en D4 |
| C3 — le réveil est borné | **MESURÉ** | agent **113–124 ms**, navigateur **366–545 ms** sur les deux gestes de l'exécution retenue |

`CAPACITE` passe de 8 à **10**, `vivier::PLAFOND_EVEIL` vaut **8** : **les deux
plafonds ne viennent plus de la même couche** — le premier du pilote de sorties
virtuelles, le second du matériel d'encodage. **Les faire suivre l'un l'autre
serait une erreur.**

**Ce que coûte une endormie : rien de mesurable, et le journal le dit de
lui-même** — `session=w-4 images=0 endormie=true cadence="0.0"` pendant que les
huit éveillées tiennent **58,4 à 68,6 i/s** chacune.

**Le contrat du vivier est observable sur le chemin réel** : le sommeil précède
toujours le réveil qu'il finance (92 ms d'écart au geste d'éviction, 82 ms au
masquage). Un réveil appliqué le premier demanderait transitoirement un encodeur
de plus que le plafond.

### ③ Le refus au rang 11 : ce qui est établi, et ce qui ne l'est pas

La onzième fenêtre est refusée par la **constante du produit**, jamais par le
pilote — qui n'est pas sollicité pour elle. Ce qui a été établi, et qui n'est pas
la même chose : **le vivier du pilote vaut bien 10 le jour de la recette**,
mesuré douze minutes après elle depuis un processus neuf (`vivier-pilote.log`) —
refus à la 11ᵉ création en `0x80070044`, onze sorties énumérées nommément.

**La constante ne borne donc plus le système en dessous de lui ; elle lui est
égale, à la date de la mesure.** Dépasser 10 exigerait de rendre des sorties au
pilote, donc d'infliger un abandon de mutex à toutes les voisines à chaque
endormissement — **ce n'était pas le marché de D5** (conception §4.2).

### ④ Pièges neufs — à connaître avant de toucher à ce terrain

- ⚠️ **Un Chrome sans interface rapporte `document.hidden = true` pour TOUTE
  fenêtre d'arrière-plan.** Une recette du sommeil qui n'y prend pas garde ne
  mesure pas ce qu'elle croit : à sept fenêtres, six pages sur sept se
  déclaraient cachées. **La visibilité de la recette D5 est donc IMPOSÉE par le
  pilote de recette**, page par page — c'est sa limite la plus lourde, et **la
  minimisation d'une vraie fenêtre n'a jamais été jouée**.
- **`Page.addScriptToEvaluateOnNewDocument` ne court PAS sur une page ouverte par
  `window.open`** : la course contre la création du document est perdue. Éprouvé
  isolément (`instrument/essai-visibilite.mjs`) — l'amorce y marque les pages
  qu'elle atteint, et les popups n'en portent pas la marque. **Poser l'override
  explicitement, page par page, est le seul moyen sûr.**
- **Le bandeau client garde son TEXTE une fois masqué** (`expirer()` lève la
  persistance sans effacer `textContent`). Lire `#status` prouve donc qu'un
  message est arrivé, **pas qu'il était affiché**. Et c'est le **seul** endroit
  observable où la *raison* du sommeil apparaisse : ni l'agent ni l'enfant ne la
  journalisent.
- **Le bandeau des pages d'application est `#status` ; `#statut` est celui de la
  page-shell.** Une exécution entière a lu le mauvais et n'a rien vu.
- ⚠️ **Après un `Stop-Process -Force` sur les agents, les sorties virtuelles
  SURVIVENT** (neuf relevées depuis un processus neuf) : il n'existe aucun chemin
  de libération sur une mort brutale. Le chien de garde du pilote finit par les
  reprendre — observé une fois en moins d'une minute, une autre fois pas encore
  au bout de deux. **Purger (`MULTIFENETRE_VDD_PURGE=1`) entre deux exécutions**,
  sans quoi la suivante démarre avec un vivier déjà entamé.
- **Ouvrir une fenêtre endort brièvement sa voisine** dans ce montage (114 à
  218 ms), pour la raison du premier piège. L'hystérésis ne peut rien contre, par
  construction : elle ne protège pas contre le masquage, qui est un geste
  explicite.
- **Un `Runtime.evaluate` qui rend un objet `Window`** (`window.open(...)`)
  échoue en `Object reference chain is too long` avec `returnByValue` : rendre
  une chaîne.

### ⑤ Ce que D5 n'établit PAS

**Aucun taux, nulle part** : une exécution rapportée, deux confirmations, et deux
exécutions abandonnées **versées avec leur diagnostic**. `HYSTERESIS = 2 s` et
`REPIT_APRES_ECHEC = 500 ms` restent **non calibrées** — le battement rapide qui
aurait jugé la première n'a pas été joué, et **aucun réveil n'a été refusé**,
donc le second chemin n'a jamais couru. Le prix du remède de C2 (si la
construction du neuf échoue, l'ancien n'est plus là) n'a **pas** été exercé.
**Rien de la latence de bout en bout**, rien de la durée (5 min 20 s au plus),
une seule application, aucun clavier, aucune souris, aucun audio, aucun
redimensionnement, aucun déplacement. **La mort d'un enfant pendant que les
autres diffusent et la fermeture d'une fenêtre en cours de diffusion** ne sont
toujours pas exercées. **Le chemin d'extinction propre du superviseur** n'a
toujours jamais été exercé — et le piège des sorties survivantes montre ce que
coûte son absence. **Les trois couches inconnues le restent** : celle du plafond
de 8 encodeurs, celle du plafond de 4 processus, et le mécanisme de l'abandon du
mutex DXGI.

### ⑥ La suite : D6, puis D7

> ⚠️ **CE PARAGRAPHE EST PÉRIMÉ SUR TROIS POINTS (annoté le 3 août 2026, à la
> fin de D6). La numérotation a changé** :
>
> - **D6 n'a fait QUE le partage de la capacité**, pas l'audio par fenêtre ;
> - **D7 est désormais l'audio par fenêtre**, avec son inconnue d'API Windows ;
> - **D8 est le plein écran et Keyboard Lock** — c'est-à-dire l'ancien D7.
>   ✅ **FAIT le 5 août 2026, et sur cinq critères de recette UN SEUL —
>   Keyboard Lock (③) — n'a été ni mesuré ni codé**, par décision : l'instrument
>   (Chrome sans interface) n'entre pas réellement en plein écran. Voir la
>   section « Sous-bloc D8 ».
>
> ❌ **Et la dette telle qu'elle est nommée ci-dessous décrit une architecture
> qui n'existe plus** : `Event::EgressBitrateEstimate` n'est une estimation « de
> session, pas de piste » que dans un montage à N pistes **dans une seule**
> `PeerConnection`. Le produit a **N sessions** depuis D1. Il n'y avait donc rien
> à découper ; le vrai défaut était l'inverse — N estimateurs indépendants
> sondant chacun vers le lien entier. Voir l'annotation du § « Ce que le chantier
> D devra régler » (chantier C volet 1) et la section « Sous-bloc D6 ».
>
> ✅ **La dernière phrase, elle, reste VRAIE, et D6 lui ajoute un plafond** : une
> fenêtre endormie porte toujours son propre contrôleur de congestion, que D6 ne
> supprime pas davantage que D5 — mais son plafond vaut désormais
> `PART_DORMANTE_BPS` (**256 000 bps**), et le relevé de recette est que son
> trafic vidéo entrant est de **0,000 Mb/s sur 30 s**, aux six exécutions où le
> cas est exercé.

**D6** — partage de la capacité réseau entre N flux, et audio par fenêtre. La
dette est nommée depuis le chantier C volet 1 : `Event::EgressBitrateEstimate`
est une estimation **de session**, pas de piste ; `audio_bps` est un budget
unique à retirer une fois et non N fois ; le filtre de `MediaEgressStats`
s'appuie sur un `video_mid` unique. **Une fenêtre endormie continue de porter son
propre contrôleur de congestion, que D5 ne touche pas.**

**D7** — plein écran et Keyboard Lock. ⚠️ **Renuméroté en D8** (voir l'encadré en
tête de ce §⑥), **fait le 5 août 2026** — sauf Keyboard Lock, non mesuré.

---

## 🔀 Sous-bloc D6 — le budget de session, et la réfutation de sa propre prémisse (3 août 2026)

Résultats complets :
`docs/superpowers/plans/2026-08-03-multifenetres-partage-capacite-resultats.md`
(**le §4.1 est le paragraphe à lire si l'on n'en lit qu'un**).
Conception : `docs/superpowers/specs/2026-08-03-multifenetres-partage-capacite-design.md`.
Journaux : `docs/superpowers/plans/journaux-multifenetres-d6/` — **49 fichiers suivis par git** (40 au
premier niveau, plus 9 dans `instrument/`), et **TROIS familles d'encodage
distinctes**, contrairement à celles de D4 et D5 qui se
`grep`aient toutes à plat (voir « Note de lecture des journaux » plus bas).

D6 devait empêcher N fenêtres de se disputer un lien saturé. **La première tâche
du sous-bloc a mesuré le lien, et il n'était pas saturé.** Le chantier a été
retourné sans être abandonné : le mécanisme est resté, sa justification a changé,
et sa constante de réglage est devenue une décision de conception au lieu d'une
constante dérivée.

### Le verdict, en trois faits qui ne se simplifient dans aucun sens

> ❌ **① LA PRÉMISSE EST RÉFUTÉE.** Le pont porte **≥ 1,44 Gb/s en TCP**
> (**3 exécutions**, minimum relevé 1 448,74 Mb/s, les deux bouts comptant le
> même nombre d'octets) et **1,64 Gb/s en UDP** (**1 exécution sur 3 — la seule
> instrumentée au noyau** ; les deux autres émettent 1 615 et 1 665 Mb/s côté VM
> sans qu'aucun compteur n'établisse leur arrivée). Huit fenêtres visant chacune
> 12 Mb/s font 96 Mb/s cumulés : **15 à 27 fois moins**. Et
> `packetsLost` vaut **0** — pas « négligeable », zéro — aux **quatre**
> exécutions du banc de décrochage **et** aux **sept** de la recette. **Le
> goulot est le DÉCODEUR DU NAVIGATEUR**, pas le réseau : sur **une** fenêtre à
> 82,257 Mb/s, le navigateur jette **53,6 %** des images reçues en passant
> **90,2 % du temps mural dans le seul décodage vidéo**.
>
> ✅ **② LE MÉCANISME CORRIGE POURTANT QUELQUE CHOSE.** À huit fenêtres :
> **18,03 %** d'images jetées au barreau plein, **7,99 %** un barreau plus bas
> (1024×576), **1,47 %** trois barreaux plus bas (640×360) — une exécution par
> point. Et le produit complet, budget arbitré par le capteur, relève **3,94 %**
> au barreau **852×480**, celui que personne n'avait mesuré (une exécution
> comparable).
>
> ⚠️ **③ MAIS CE N'EST PAS LE DÉBIT QUI SAUVE, C'EST LA RÉSOLUTION — et c'est
> le fait le plus contre-intuitif du sous-bloc.** Le témoin à **surface
> quasi constante** — bits divisés par 2,2, **7 fenêtres sur 8 restées en
> 1280×720**, la huitième seule ayant franchi un seuil — rend **23,08 %**
> d'images jetées, soit **PAS MEILLEUR** que les 18,03 % du barreau plein.
> ⚠️ **Une exécution chacun : l'écart entre ces deux nombres n'est pas
> nécessairement significatif. Ce qui l'est, c'est l'ABSENCE DE TOUTE
> AMÉLIORATION**, à opposer aux baisses nettes obtenues dès qu'un barreau est
> franchi. **`BUDGET_BPS` n'agit que par l'intermédiaire de l'échelle, en
> marches discrètes** : une valeur qui réduirait les bits sans faire changer de
> barreau **ne corrigerait rien**. Elle doit donc être choisie
> pour **franchir un seuil de barreau au N visé**, jamais pour « laisser de la
> marge ».

### Les cinq critères, avec le nombre d'exécutions dans chaque énoncé

**Douze exécutions du produit en tout** : une au §1 du document de résultats
(la mesure du lien), quatre à son §2 (le banc de décrochage), sept à la recette
de son §3. **Aucun taux n'est revendiqué nulle part.**

| # | Critère | Verdict | Exécutions |
| --- | --- | --- | --- |
| ① | `framesDropped` < 7,99 % à 8 fenêtres | **TENU sous charge d'hôte de référence — NON REPRODUIT autrement** (3,94 %) | **1 sur 5** à 12 Mb/s ; **1 sur 2** parmi celles dont l'échelle s'est posée |
| ② | la taille d'encodage a bougé | **TENU** — 852×480 à 12 Mb/s, 640×360 à 8 Mb/s, contre 1280×720 sans budget | **7 sur 7** |
| ③ | somme des parts ≤ `BUDGET_BPS` | **TENU** — jamais dépassé, dans aucune phase d'aucune exécution | **7 sur 7** |
| ④ | la focalisée est strictement au-dessus des autres éveillées | **TENU en majorité, PAS SYSTÉMATIQUE** — **11 déplacements sur 14** | **7** (2 déplacements chacune) |
| ⑤ | une endormie reste au plancher | **TENU** — 256 000 bps exactement, **0,000 Mb/s sur 30 s** | **6 sur 6** (une exécution n'atteint pas sa phase 3) |

⚠️ **Le critère ① porte sa réserve DANS son verdict, et il ne faut pas la perdre
en recopiant le tableau.** Les quatre autres exécutions à 12 Mb/s relèvent
**8,03 %, 16,70 %, 59,32 % et 75,47 %**. La dégradation covarie avec le
`loadavg` de l'hôte **et** avec le non-établissement de l'échelle d'encodage ;
**les deux ne sont pas départagées.** **Le produit n'est pas démontré robuste
sous la charge d'hôte réellement rencontrée pendant la campagne**, et la
performance mesurée varie **d'un facteur 19** sur le taux d'images jetées à
binaire, budget et protocole identiques. **Le montage de recette mesure son hôte
au moins autant que le produit.**

✅ **Les trois échecs du critère ④ sont imputés au PROTOCOLE, sur pièces** — voir
le premier piège plus bas. ⚠️ **Aucune exécution n'a été rejouée avec un palier
plus long** : que 45 à 60 s suffiraient est **plausible et non vérifié**.

### Ce que le code livre, et trois faits de conception qui lui survivront

| Étage | Fichier | Nature |
| --- | --- | --- |
| la règle de part | `agent/src/capteur/repartiteur.rs` (147) | **pur, aucun `cfg`**, trois régimes documentés et testés sur l'hôte |
| l'arbitrage et l'émission | `agent/src/capteur/sommeil/parts.rs` (~~348~~ **349** au 6 août 2026) | lit `BUDGET_BPS`, n'émet que les parts **qui changent** |
| le transport | `agent/src/capteur/pont_media.rs` (~~188~~ **263** au 5 août 2026), `capteur/distante.rs` (~~288~~ ~~375~~ **400** au 6 août 2026) | la part voyage sur la connexion **média**, écrasement du dernier reçu |
| l'application | `agent/src/transport/part.rs` (~~274~~ **301** au 6 août 2026) | `set_desired_bitrate` **toujours** ; `changer_plafond` **seulement si la fenêtre est éveillée** |

> ✅ **Tailles relevées PAR LA COMMANDE le 3 août 2026, à la vague de correction
> finale de branche** — `repartiteur.rs` et `part.rs` avaient grossi depuis le
> relevé initial (119 → 147, 138 → 274). **Aucun fichier de ce sous-bloc
> n'approche le plafond de 500** : le plus gros est `capteur/sommeil.rs` à
> **432**, et `windows_source.rs` (~~638~~ **628** depuis D9, dette gelée) n'a pas été touché.

### ⚠️ Le défaut que la revue finale a trouvé, et qui frappait CHAQUE réveil

**Une part d'endormie ne doit JAMAIS atteindre `Controleur::changer_plafond`.**
La chaîne, entièrement lisible dans le code et corrigée le 3 août 2026 :
`PART_DORMANTE_BPS` vaut **256 000**, `changer_plafond` fait
`video_bitrate_bps.min(plafond)` dès qu'une estimation a existé, et **rien ne
défait ce `min`** — la seule réparation est `Controleur::observer`, alimenté par
le bras `MediaEgressStats` que **str0m n'émet pas pour un flux qui n'a rien
envoyé** (`send_stats.rs`, `if self.bytes == 0 { return; }`). Une endormie
n'envoie rien. Au réveil, **le plafond remontait, pas le débit** : la fenêtre
restait figée à 256 kb/s, soit **sous le barreau plancher de l'échelle**
(691 200 bps à 1280×720/60), pour le restant de la session. **Ce n'était pas un
cas limite : c'était l'état de chaque réveil.**

**Le remède** : l'enfant lit son état de sommeil (`VideoSource::est_endormie`,
porté par `SourceDistante` et posé sur les `Sommeil` que le capteur pousse
déjà) et n'applique la part au contrôleur que s'il est éveillé ; le sondage,
lui, la reçoit toujours. **Une endormie a relâché son encodeur (D5) : il n'y a
rien à borner côté encodage.** Couvert par
`une_part_dormante_ne_borne_pas_le_controleur_et_le_reveil_est_suivi`
(`transport/part.rs`), **rouge observé** avant remède (`left: 256000`,
`right: 1333333`).

⚠️ **L'état de sommeil est LU, jamais deviné.** Le déduire d'une comparaison de
la part à `PART_DORMANTE_BPS` couplerait deux processus par une valeur, et ce
couplage se romprait en silence le jour où l'un des deux changerait de
constante ; et il ne se
consomme pas, contrairement à l'annonce `sommeil_a_annoncer` — c'est un état
courant, relu à chaque part.

⚠️ **`SourceDistante` naît `endormie = true`, et se remet à `true` à chaque
rattachement.** Depuis D5 une fenêtre naît endormie côté capteur, sans qu'aucun
`Sommeil` ne l'annonce (il n'y a pas de transition), et sa première part est le
plancher. Partir de `false` réintroduirait le défaut à la naissance et à chaque
reprise de canal.

⚠️ **Un résidu borné subsiste, et il est nommé** : sur le chemin `rompus` de
`distribuer_les_parts`, une session réveillée par la place qu'un mort libère
reçoit son `Reveiller` **après** une part d'endormie déjà périmée, qu'elle
applique donc en étant éveillée. **Borne : `PERIODE_REARBITRAGE`, 250 ms**, au
terme desquelles le tour de roue réémet la part correcte. Le canal unique
garantit l'ordre de **livraison**, jamais l'ordre de **calcul** — deux
commentaires affirmaient le contraire sans réserve, corrigés dans la même vague.

⚠️ **Second défaut de la même vague** : `sommeil::retirer` vidait `focalisee`,
mais ni `distribuer` ni le chemin `rompus` ne le faisaient. Les trois passent
désormais par `sommeil::oublier`, **point de passage unique** du registre. La
conséquence qui mord n'est pas celle qu'on croit : un nom mort ne majore
personne, mais un **rattachement réinscrit le même nom**, qui héritait alors du
focus sans que le client l'ait jamais réémis.

### 🔎 La recette d'entrée de D7 — un `grep`, et le seul défaut muet est levé

**Le mode de défaillance que cette vague introduit est SILENCIEUX, et il est
total.** Si `est_endormie()` restait bloqué à `true` — un `Sommeil
{ endormie: false }` perdu, un rattachement dont le réveil n'arrive jamais —,
**aucune fenêtre n'appliquerait plus jamais de part à son contrôleur, et D6 ne
ferait plus rien du tout**, sans un `WARN`, sans une erreur, sans un seul
symptôme hors la résolution qui ne descend plus.

Le champ `endormie` de la trace le rend observable **pour rien** :

```bash
grep 'part de budget appliquee' agent.log | grep -c 'endormie=true'
grep -c 'part de budget appliquee' agent.log
```

**Si les deux comptes sont égaux, le défaut est là.** En marche nominale à N
fenêtres, la très grande majorité des parts appliquées porte `endormie=false` :
seules les fenêtres réellement au-delà de `vivier::PLAFOND_EVEIL` (8) dorment.
**C'est ce qui fait du changement de format du journal un BÉNÉFICE, pas un
coût** — à jouer en tête de D7, avant toute autre mesure.

✅ **Et ce changement de format ne casse aucun instrument** : un seul lit cette
trace, `docs/superpowers/plans/journaux-multifenetres-d6/instrument/pilote-recette-d6.mjs`,
qui la filtre par sous-chaîne (l. 130, 225) et l'analyse par **regex sur la
clé** — `/session=(\S+)/` et `/part_bps=(\d+)/` (l. 133-134), **jamais par
position**. Vérifié, à ne pas revérifier.

⚠️ **Deux phrases de ce dépôt parlent de « saturer le lien » sans porter la
prémisse de D6, et il ne faut PAS les chasser** : `agent/src/transport.rs:270`
et `agent/src/transport/adaptation.rs:28` décrivent toutes deux le risque d'une
estimation initiale trop haute sur un lien **étroit, au premier instant d'une
session** (`ESTIMATION_INITIALE_BPS`, chantier C volet 1). Elles sont
préexistantes, conditionnelles, et étrangères au partage de capacité. Nommées
ici pour qu'un successeur n'y perde pas une ronde.

1. **`repartir` ne garantit le non-dépassement du budget que dans son régime 1.**
   Deux régimes dégénérés existent — `reste < diviseur`, et
   `budget < endormies × PART_DORMANTE_BPS` — où le `.max(1)` appliqué à chaque
   part **après** la division fait dépasser le budget de quelques bps. **Cet
   invariant est CONDITIONNEL**, et sa documentation a dû être réécrite **deux
   fois** pour cesser d'affirmer un absolu : la première réécriture a remplacé un
   faux par un autre. **Seul le régime 1 a jamais été rencontré en recette** —
   les deux autres exigeraient un budget dérisoire.
2. **`Controleur::changer_plafond` doit distinguer « aucune estimation JAMAIS
   reçue » de « estimation PÉRIMÉE ».** Le témoin correct est
   `premiere_estimation_a` (`agent/src/congestion/controleur.rs`), **monotone et
   jamais effacé** — **pas** `courant.adaptation`, dont l'`Indisponible` couvre
   les deux cas. Sans cette distinction : ou bien une part qui remonte ne relève
   **jamais** le débit et le fige sans terme (défaut passif), ou bien un lien qui
   vient de se taire se voit accorder un **dépassement actif** (pire).
3. **`set_desired_bitrate` EST une mutation de `Rtc`.** L'énoncé d'audit de
   `agent/src/transport/tick.rs` affirmait que les branches concernées n'en
   mutent aucune : **c'était faux**, et il a fallu deux rondes pour le corriger
   **aux deux endroits du même fichier**. Ce qui préserve réellement l'invariant
   de drainage, c'est que l'appel **ne met aucun paquet en file** — pas une
   absence de mutation.

### `BUDGET_BPS = 12 000 000` : un choix ASSUMÉ, pas démontré

Le brief prévoyait de replier à 8 000 000 si le taux dépassait 7,99 % ; **il ne
le dépasse pas** sur l'exécution jouée sous la charge d'hôte de référence.
**Mais la comparaison des deux valeurs est un ARBITRAGE que la mesure ne tranche
pas :**

| Budget | barreau des 7 non focalisées | % jetées | MP/s décodés | focus réussi |
| --- | --- | --- | --- | --- |
| **12 Mb/s** | 852×480 | **3,94 %** (1 exéc. comparable) | **196,40** | 8/10 |
| **8 Mb/s** | 640×360 | **1,46 %** et **3,94 %** (2 exéc.) | 95,54 et 135,57 | 3/4 |

⚠️ **La colonne « focus réussi » ne compare RIEN** : `8/10` contre `3/4`
**mesure surtout la durée des paliers, pas le mécanisme** — les échecs sont
imputés au protocole (palier de 25 s contre `DELAI_REMONTEE` de 20 s, voir le
premier piège). **`FACTEUR_FOCUS` ne départage pas les deux budgets.**

**Plus de pixels livrés, davantage jetés.** Aucune des deux valeurs ne domine
l'autre sur les deux grandeurs, et les effectifs (1 contre 2) n'autorisent
aucune comparaison statistique. **Le choix tient à UNE seule raison : 12 Mb/s ne
coûte rien au cas mono-fenêtre**, là où 8 Mb/s lui retirerait un tiers de son
débit. ⚠️ **Et ce cas mono-fenêtre n'a JAMAIS été mesuré à 12 Mb/s** — les deux
points existants à N = 1 sont 9,479 Mb/s (0 % jetées) et 82,257 Mb/s (53,6 %).
**La valeur protège donc un acquis SUPPOSÉ**, et se réviserait sans embarras si
ce cas était mesuré et se révélait déjà mauvais.

⚠️ **La valeur est DE LABORATOIRE** : navigateur Chrome sans interface,
`--disable-gpu`, donc **décodage logiciel**, sur l'hôte qui porte aussi la VM et
une charge étrangère variable d'un facteur 3,6. **Un client réel, sur une autre
machine, avec décodage matériel, décrocherait ailleurs** — probablement bien
plus haut. **`BUDGET_BPS` n'est pas une constante du produit.**

### ❌ `set_desired_bitrate` n'a AUCUNE couverture — et la voie qui l'aurait donnée n'a pas été jouée

C'est l'appel que la conception désigne comme **le plus important** — celui qui
empêche N fenêtres de sonder chacune le lien entier. Il n'a **ni test unitaire,
ni recette**.

> ⚠️ **« Le plus important » était une conséquence de la prémisse, et la
> prémisse est réfutée (voir ① ci-dessus).** Le sondage cumulé ne saturait
> rien : le lien porte ≥ 1,44 Gb/s et n'a jamais perdu un paquet. **Ce qui agit
> est `changer_plafond`**, par l'échelle, donc par la **résolution** — c'est le
> fait ③. Ce commentaire vivait à l'identique dans trois blocs de code
> (`capteur/repartiteur.rs`, `capteur/protocole.rs`, `transport/part.rs`),
> réécrits à la vague de correction finale de branche. **La lacune de
> couverture, elle, tient intégralement** : l'appel reste sans témoin, et l'A/B
> différentiel qui l'aurait donné n'a toujours pas été joué.

- **Le test unitaire honnête est INFAISABLE**, établi indépendamment par deux
  relecteurs : str0m n'expose **aucun getter**, son `Debug` est un **stub**, et
  `configure_pacer` **ne dépend pas** de cette valeur (même effet synchrone à
  1 000 et à 50 000 000 bps). Deux voies de plus ont été explorées et écartées
  (différentiel de bourrage sur pair local : non déterministe ; couture
  injectable : tautologie payée d'une indirection permanente).
- **La recette n'a pas pu servir de témoin**, et elle dit **pourquoi** : une
  fenêtre endormie a un objectif de sondage de 256 000 bps, n'encode plus rien,
  et émet **0,000 Mb/s sur 30 s** aux six exécutions où le cas est exercé. **Sur
  ce montage, « sondage borné à la part » et « pas de sondage du tout » se
  lisent identiquement.**
- ⚠️ **« Aucun témoin trouvé » n'est PAS « aucun témoin n'était possible ».**
  **L'A/B différentiel sur ce montage même** — neutraliser l'appel, opposer les
  deux trafics cumulés — **n'a pas été joué**, et rien dans les relevés ne dit
  qu'il aurait échoué. C'est exactement la méthode que ce dépôt a payée cher au
  chantier des duplications parallèles : *retirer la variable suspecte et voir
  si le symptôme survit.* **C'est le point ouvert le plus important de D6.**

**Fait annexe utile** : que l'endormie n'émette rien répond au passage à
l'hypothèse déclarée non vérifiée dans la doc de `PART_DORMANTE_BPS` — **sur ce
montage, le plancher ne coûte que sa ligne**. Cela ne dit rien du bourrage émis
quand un média *actif* sonde à la hausse.

### Ce que D6 n'établit PAS

- **Aucun taux, nulle part.** Une exécution par point aux §1 et §2 du document
  de résultats ; sept à la recette de son §3, dont **trois seulement** comparables entre elles et **une seule** à
  12 Mb/s dans ces conditions.
- **`FACTEUR_FOCUS` et `PART_DORMANTE_BPS` restent NON CALIBRÉES**, et **aucune
  constante de ce sous-bloc n'a été jugée par un jugement visuel** — exactement
  la lacune que `BPP_MIN` traîne depuis le chantier C volet 1. La recette montre
  que `FACTEUR_FOCUS` fait franchir un barreau 11 fois sur 14 ; elle ne le
  calibre pas, et **ne permet pas de le dire plus robuste à un budget qu'à
  l'autre** (une première rédaction l'affirmait, sur un sous-ensemble choisi ;
  réfutée par une pièce versée).
- **Aucun travail conservateur** : une fenêtre qui n'use pas sa part **ne la
  rend pas** aux autres. La reprise de l'inutilisé introduirait une seconde
  boucle de rétroaction, hors périmètre — **à nommer, pas à croire faite**.
- **Le critère ③ est tenu sur les PARTS, pas sur le FIL** (M2, revue finale de
  branche). La part est un budget **total**, mais `changer_plafond` la traite
  comme une borne **vidéo** (`observer` retranche `audio_bps` avant de borner)
  quand `set_desired_bitrate` la traite comme un total : le trafic émis dépasse
  la somme des parts de **N × (audio + surcoût RTP)**. L'approximation est
  **préexistante**, mais **D6 la rend un ordre de grandeur plus
  significative** — à 12 Mb/s sur huit fenêtres, `audio_bps` (128 000) pèse
  **≈ 10 % d'une part** contre ≈ 1 % des 12 Mb/s d'avant. **Aucune mesure du
  dépassement réel sur le fil n'a été prise.**
- **Les régimes 2 et 3 de `repartir` ne sont pas exercés** en conditions réelles.
- **La latence de bout en bout n'est toujours mesurée par AUCUN sous-bloc du
  chantier D**, et D6 ne la mesure pas davantage.
- **Les trois couches inconnues le restent** : celle du plafond de 8 encodeurs,
  celle du plafond de 4 processus, et le mécanisme de l'abandon du mutex DXGI.
- **`BPP_MIN` et le `fps` de `Config`** restent non calibrés et se recalibrent
  ensemble ; **`HYSTERESIS` et `REPIT_APRES_ECHEC`** du vivier de D5 aussi.
- **Le chemin d'extinction propre du superviseur n'a toujours jamais été
  exercé**, et **la mort d'un enfant pendant que les autres diffusent** pas
  davantage.
- **Rien au-delà de dix fenêtres** : `CAPACITE` vaut 10, la montée s'y arrête —
  donc sur le produit et non sur un plafond du système.
- **La visibilité ET le focus sont IMPOSÉS par le pilote de recette**, page par
  page, parce qu'un Chrome sans interface rapporte `document.hidden = true` pour
  toute fenêtre d'arrière-plan. **Aucune minimisation de vraie fenêtre, aucun
  clic réel.** Limite héritée de D5, et la plus lourde de ce montage.
- **La composante qui jette les images n'est pas identifiée** : l'hôte n'est pas
  saturé (43 à 52 % de temps CPU inactif au rang où 18 % des images sont
  jetées), aucun décodeur individuel ne l'est, le réseau ne perd rien. **Et les
  deux bouts se dégradent ensemble** — à N = 8 le capteur lui-même retombe de 85
  à 47,8–79,4 i/s par fenêtre : **ce montage ne départage pas** la part imputable
  à la VM de celle imputable au navigateur.
- Une seule application, une seule animation, aucun clavier, aucune souris,
  aucun audio, aucun redimensionnement, aucun recouvrement, aucun déplacement de
  fenêtre. Palier le plus long : 40 s ; exécution la plus longue : 8 min 47 s.

### Pièges neufs — à connaître avant de toucher à ce terrain

- ⚠️ **UN PALIER DE MESURE DOIT ÊTRE PLUSIEURS FOIS PLUS LONG QUE LA
  TEMPORISATION DU MÉCANISME QU'IL OBSERVE.** Celui du focus valait **25 s** pour
  un `DELAI_REMONTEE` de **20 s** (`agent/src/congestion/hysteresis.rs`) : 25 %
  de marge, où devaient encore tenir l'annonce de focus, la redistribution des
  parts et la montée de l'estimation. **Trois promotions sur quatorze sont
  arrivées APRÈS la fin du palier** — relevées 63 et 66 s plus tard sur la même
  fenêtre — et une quatrième a été **préemptée** par le déplacement suivant.
  **L'échec s'imputait au produit alors qu'il venait du protocole.** Le remède
  est gratuit (45 à 60 s). **Lire les constantes de temporisation du code AVANT
  de dimensionner un palier.**
- ⚠️ **UN SOUS-ENSEMBLE SANS RÈGLE DE SÉLECTION ÉNONCÉE EST UN SOUS-ENSEMBLE
  CHOISI**, même quand on ne l'a pas choisi. « 4 déplacements de focus sur 4 »
  en cachait **14**, dont 11 réussis — et l'exécution écartée était **précisément
  celle qui échoue**. La conclusion fausse qui en sortait partait vers un
  commentaire de code, donc vers la mémoire longue du dépôt. **Énoncer la règle
  de sélection AVANT de compter, et vérifier qu'elle est pertinente pour la
  grandeur qu'on juge** — celle appliquée ici l'avait été sur la charge d'hôte,
  qui ne dit rien de la promotion de focus.
- ⚠️ **« Seule X change » se vérifie contre son PROPRE tableau.** L'affirmation
  « entre ces deux exécutions, seule la charge de l'hôte change » était réfutée
  par une ligne imprimée **trois lignes plus haut** du même document (l'échelle
  de l'une ne s'était jamais posée, et la charge étrangère **nommée** était
  identique à 0,1 point près).
- ⚠️ **Une trace non attribuable coûte une ré-imputation.** Tous les enfants
  partagent le même `agent.log` depuis D4 : une trace sans champ `session` y est
  un nombre dans un multiensemble anonyme. **Deux traces ont dû recevoir leur
  `session` en pleine recette** (`f7557d3`, `99e5641`), et **une troisième manque
  encore** — les trois `warn!` d'`agent/src/transport/adaptation.rs` (99, 155,
  166). Le remède de fond est un **span `tracing` porteur de `session` sur le fil
  de fenêtre du capteur** (consignation n°2 pour D7).
- ⚠️ **Un compteur de journal peut compter des LIGNES et non des ÉVÉNEMENTS.**
  Chaque changement de barreau produit **deux** lignes au même horodatage, à
  ~70 µs d'intervalle : une d'`agent::windows_source::encodage` côté capteur, une
  d'`agent::transport::adaptation` côté enfant. Tous les compteurs bruts de la
  recette valent **le double**.
- ⚠️ **Corriger une affirmation fausse peut en PRODUIRE une autre.** La
  rectification de `windows_source.rs` a échangé une prémisse fausse contre une
  **conclusion** fausse, dans le même commentaire, à la ronde suivante. Le remède
  qui a fini par tenir est d'**inscrire dans le commentaire les trois `grep` qui
  l'établissent**, pour que le prochain lecteur refasse le contrôle sans croire
  personne.
- ⚠️ **Un contrôle anti-piège qui se déclenche trop tôt ne contrôle rien.** Le
  contrôle « la variable est-elle arrivée ? » a rendu `[]` aux sept exécutions :
  la trace vient d'un `OnceLock` initialisé à la **première fenêtre**, et le
  contrôle courait six secondes après le lancement du superviseur. **Il aurait
  masqué une variable réellement manquante.** Vérifier qu'un contrôle **peut
  échouer** avant de s'y fier.
- ⚠️ **Mesurer un palier avant que l'échelle d'encodage ne se pose mélange deux
  régimes** — et disqualifie l'exécution. **Attendre le FAIT** (douze secondes
  sans aucun changement de barreau), pas une durée.
- ⚠️ **La déduplication d'annonce de `client/src/visibilite.ts` peut faire
  DISPARAÎTRE le focus.** Une page tout juste ouverte annonce `focalisee=true` si
  `main.ts` s'attache avant l'amorce du pilote ; le `blur` envoyé ensuite vide le
  champ côté capteur, et la page qu'on **veut** focalisée, dont l'état n'a pas
  changé, ne réémet rien. Résultat mesuré : plus aucune fenêtre focalisée du
  tout. **Faire passer la cible par `blur` puis `focus`.**
- ⚠️ **Une part majorée qui atterrit à 0,47 % d'un seuil de barreau est un tirage
  au sort, pas une majoration.** Vérifier la marge d'une constante **contre
  l'échelle** avant de croire qu'elle produit l'effet voulu. *(Ce n'était
  cependant pas la cause des échecs de focus ici — c'était la durée du palier.)*

**Note de lecture des journaux — D6 a TROIS familles**, contrairement à D4 et D5
qui se `grep`aient tous à plat :

| Famille | État | Ce qu'il faut faire |
| --- | --- | --- |
| `agent-mesure-lien.log`, `agent-decrochage-*.log` | UTF-8, **ANSI retirées** | rien |
| les **sept** `agent-critere-*.log` | UTF-8, **séquences ANSI PRÉSENTES** | `sed 's/\x1b\[[0-9;]*m//g'` avant tout `grep` |
| les journaux de **pilote** (`mesure-lien.log`, `decrochage-*.log`, `critere-*.log`) | classés « data » : **octets de contrôle isolés à la place des accents** (`Op\x02ration r\x02ussie`), renvoyés par le PowerShell de `run-agent.sh` | `grep -a`, et **ne jamais y chercher un mot accentué** |

⚠️ La troisième ligne est le **défaut à deux réglages** déjà documenté (D4),
rencontré tel quel : `build-agent.sh` / `run-agent.sh` ne posent pas
`[Console]::OutputEncoding`. **Toujours non corrigé.**

### La suite : D7, puis D8 — et les quatre consignations

**La numérotation a changé, et le §⑥ de D5 est annoté en conséquence** : D6 n'a
fait que le partage de la capacité.

- **D7** — **l'audio par fenêtre**, avec son inconnue d'API Windows. ✅ **Fait le
  3 août 2026.**
- **D8** — **le plein écran et Keyboard Lock** (l'ancien D7). ✅ **Fait le 5 août
  2026 — sauf Keyboard Lock, non mesuré par décision.**

**Les quatre consignations, à porter dans le plan de D7 plutôt qu'à
redécouvrir :**

1. **Rendre `TICKS` / `CAPTURED` / `PRODUCED` par session** — et, ce faisant, les
   **extraire** vers `agent/src/windows_source/telemetrie.rs`, ce qui rend au
   fichier la marge que D6 lui a prise (+7, voir le tableau de dette).
2. **Poser un span `tracing` porteur de `session` sur le fil de fenêtre du
   capteur.** Préalable à toute recette qui chronomètre des promotions de
   barreau ; il couvre aussi les trois `warn!` anonymes d'`adaptation.rs`.
3. **Rejouer le critère ④ avec un palier de 45 à 60 s** — la seule façon de
   savoir si la promotion de focus est systématique.
4. **Couvrir `set_desired_bitrate` par l'A/B différentiel**, non joué ici.

✅ **La consignation n°2 est absorbée par D7** (tâche 10, `1ae295d`) : le span
`tracing` porteur de `session` vit désormais sur le fil de fenêtre du capteur —
voir la section suivante. Les n°1, 3 et 4 **restent des suites de D6**, non
traitées par D7, et attendent toujours.

❌ **Et elles attendent TOUJOURS après D8 (5 août 2026) : aucune des trois n'a
été traitée.** La n°1 est celle qui coûte le plus à laisser dormir — c'est elle
qui rendrait à `windows_source.rs` (638, dette gelée) la marge que D6 lui a
prise, et le fichier reste sous la condition « la prochaine addition exige une
extraction ». ⚠️ **Deuxième sous-bloc consécutif qui les reporte** ; elles sont
reprises dans les legs de D8.

✅ **D9 (6 août 2026) en a traité DEUX sur trois.** La n°1 est FAITE :
`windows_source.rs` retombe de 638 à **628**, `windows_source/telemetrie.rs`
(**72**, pur, deux tests d'hôte) est né, et la condition « la prochaine addition
exige une extraction » est **LEVÉE**. La n°3 (le critère ④ à palier long) a été
jouée — **3 promotions sur 4 déplacements, 2 exécutions**. ❌ **La n°4 (l'A/B
sur `set_desired_bitrate`) a été jouée MAIS N'ÉTABLIT RIEN** : l'écart entre
bras (+23,2 %) est **plus petit que la variance intra-bras** (+83,1 %). Elle
reste due.

---

## 🔊🪟 Sous-bloc D7 — l'audio par fenêtre (3 août 2026)

Résultats complets :
`docs/superpowers/plans/2026-08-03-multifenetres-audio-par-fenetre-resultats.md`.
Conception : `docs/superpowers/specs/2026-08-03-multifenetres-audio-par-fenetre-design.md`.
Journaux : `docs/superpowers/plans/journaux-multifenetres-d7/` — **UTF-8, CRLF,
séquences ANSI de `tracing` PRÉSENTES** : `sed 's/\x1b\[[0-9;]*m//g'` avant tout
`grep`, y compris sur la recette d'entrée de D8 (voir plus bas).

D7 remplace le mix de session unique, réservé à la première fenêtre détectée et
jamais rendu, par l'**isolation stricte** : chaque fenêtre porte le son de **son**
application et de rien d'autre, via l'API Windows de *process loopback*
(`AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK`), dont seule la moitié
« activation » avait été mesurée au chantier A (28 juillet 2026) — `Initialize`,
`GetService`, `Start()` et le moindre octet capturé restaient une inconnue
éliminatoire.

### Le verdict : la voie est REÇUE, et quatre critères sur cinq sont tenus

**§1 — la mesure qui gouverne, REÇUE.** Quatre relevés, **une exécution
chacun**. Le relevé décisif n'est **pas** « des octets arrivent » — un flux qui
rendrait en réalité le mix global passerait ce test-là aussi — mais **un
voisin silencieux qui lit 1 LSB de crête pendant que le joueur en lit 11679**
(séparation calculée : `20·log₁₀(11679/1)` = **81,3 dB**). **Un quatrième
relevé, non planifié, a resondé le voisin une fois le joueur ARRÊTÉ** : la
crête vaut encore **1**. Ce 1 est donc un **plancher du chemin lui-même**, pas
une fuite du voisin — sans ce quatrième relevé, la question serait restée
ouverte.

**§2 — la recette produit, quatre critères sur cinq tenus** :

| # | Critère | Verdict | Exécutions |
| --- | --- | --- | --- |
| ① | Isolation — deux applications, deux tonalités | **TENU**, séparation 82 à 94 dB | **2** |
| ② | Arbitrage par PID — deux fenêtres d'un même processus | **TENU** | **1** (2 phases de focus) |
| ③ | L'audio survit au sommeil | **NON EXERCÉ** | **0** |
| ④ | Le budget suit l'arbitrage | **MESURÉ** | **1** |
| ⑤ | Aucune régression mono-fenêtre | **TENU** | **1**, plus un témoin A/B |

⚠️ **Le critère ③ n'a PAS été exercé, et c'est la lacune la plus lourde de
cette recette.** L'audio d'une fenêtre endormie (au-delà des huit éveillées du
vivier de D5) n'a jamais été observé tourner : le chemin existe et est
raisonné — le sommeil ne touche pas `emet` —, mais rien ne le prouve.

### Le fait de conception que la recette a révélé, pas la conception

⚠️ **Une fenêtre qui porte le son entend le MÉLANGE de tout le groupe de PID de
son application, et c'est structurel, pas un défaut d'implémentation.**
`PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE` capture l'**arbre de
processus**, jamais la fenêtre. **Deux fenêtres d'une même application ne
peuvent donc jamais avoir un son séparé** — le focus ne choisit que **laquelle
des deux reçoit le flux partagé**. La conception (§4.3) ne le disait pas ; le
critère ② l'a révélé (1 exécution, 2 phases) : la fenêtre portante entend les
440 Hz **et** les 880 Hz simultanément, l'autre −1000 dB sur les deux.

Fait annexe qui justifie l'instrument choisi : sur cette même exécution,
`bytesReceived` de la fenêtre muette **a continué de croître** (274 832 →
275 798 octets) pendant que son spectre était à −1000 dB — un compte d'octets
seul aurait conclu à tort qu'elle entend encore quelque chose. **C'est pourquoi
le critère se juge à la fréquence dominante (`AnalyserNode`), jamais au compte
d'octets.**

### Trois défauts que seule la recette pouvait trouver

1. **`scripts/run-agent.sh` ne transmettait pas `AUDIO`.** La tâche 9 a promu
   `AUDIO=0` en interrupteur global qu'un agent lancé à la main doit pouvoir
   employer, mais `run-agent.sh` **est** la façon de lancer un agent à la main
   sur cette VM. Implémenteur et relecteur avaient vérifié la propriété **en
   traçant le code** — le tracé était juste, la valeur ne pouvait simplement
   pas atteindre le processus. Corrigé (`a891062`).
2. **La recette d'entrée de D8 telle que la conception l'écrivait rend 0 sur un
   journal brut** — voir la sous-section suivante.
3. **Ce même contrôle rend 0 sur toute session de moins de 30 s**, bénin, et
   indiscernable du défaut qu'il existe pour révéler — voir la sous-section
   suivante.
4. ❌ **Et un TROISIÈME défaut, que la recette n'a PAS trouvé** — la revue
   finale de branche, si : la condition de ce contrôle était **insatisfiable**,
   parce que le code ne pouvait pas émettre `actif=false`. Voir la
   sous-section suivante.

### ⚠️ La recette d'entrée de D8 : le `grep` de la conception a TROIS défauts, corrigés ici

La conception (§6) prescrivait, comme premier geste de D8 :

```bash
grep -c 'compteurs audio' agent.log
grep 'compteurs audio' agent.log | grep -c 'actif=true'
```

**Les deux lignes ont été exécutées, et les deux portent un défaut trouvé par
l'exécution, pas par la relecture** — puis un troisième, que l'exécution n'a pas
pu trouver :

1. **Elle rend 0 sur un `agent.log` brut.** Les séquences ANSI de `tracing`
   séparent le nom du champ de sa valeur (`[3mactif[0m[2m=[0mtrue`) :
   `grep 'actif=true'` ne matche jamais littéralement. Le
   `sed 's/\x1b\[[0-9;]*m//g'` est **obligatoire, pas optionnel** — exactement
   la même classe de piège que les journaux `banc-*` du 30 juillet 2026.
2. **Elle rend 0 sur toute session de moins de 30 s**, parce que
   `compteurs audio` est **périodique** (`REPORT_INTERVAL = 30 s`,
   `agent/src/windows_audio.rs`). Sur l'ensemble de la recette D7 — sessions
   toutes < 1 min — cette ligne n'a **jamais** été émise : un zéro **bénin**,
   indiscernable du défaut que le contrôle existe pour révéler. **C'est le
   propre piège du dépôt — « un contrôle qui se déclenche trop tôt ne contrôle
   rien » (D6) — rejoué sur un contrôle écrit précisément pour l'éviter.**
3. ❌ **LA CONDITION ÉTAIT INSATISFIABLE — le code ne pouvait pas émettre la
   ligne que le second `grep` cherche à ne PAS trouver** (F1, revue finale de
   branche). Dans `agent/src/windows_audio.rs`, le bloc `REPORT_INTERVAL`
   vivait **après** le `if !emettait { … continue; }` de la branche muette : la
   trace n'était atteignable que quand `emettait` valait vrai, donc son champ
   `actif` valait **structurellement `true`**. Le second compte était
   **toujours égal** au premier. Et le défaut que ce contrôle existe pour
   révéler — plus aucune fenêtre ne porte le son — rend `0` et `0`, que le
   point 2 ci-dessus classe **bénin** : l'opérateur lisait 0/0, concluait
   correctement que rien n'allait mal, **dans l'état exact où tout allait
   mal**. ✅ **Corrigé** : le bloc est remonté au-dessus du gate, une fenêtre
   muette rapporte elle aussi toutes les 30 s, avec `actif=false`.

⚠️ **La leçon de méthode est le point 3, pas les deux premiers.** Ce contrôle
avait été écrit *précisément* pour éviter un défaut muet, relu par treize revues
par tâche, et **exécuté** — et il ne pouvait pas échouer. Ce qui l'a laissé
passer est que personne n'a vérifié qu'il **PEUT** échouer : les sessions de la
recette durant toutes moins de 30 s, la ligne n'a jamais été émise, et le fait
qu'`actif` ne prenne qu'une seule valeur ne pouvait pas se voir. **Le piège
« vérifier qu'un contrôle peut échouer » (D6) a été payé DEUX fois de suite sur
ce seul paragraphe** — d'abord sur la durée de session, ensuite sur
l'atteignabilité de la trace.

**La forme corrigée, à employer en tête de D8** :

```bash
sed 's/\x1b\[[0-9;]*m//g' agent.log > agent-plat.log
grep -c 'compteurs audio' agent-plat.log                      # A
grep 'compteurs audio' agent-plat.log | grep -c 'actif=true'  # B
```

**et seulement sur une session vivante depuis au moins `REPORT_INTERVAL` (30 s)**
— sous cette durée, un premier compte à 0 ne dit rien. Le §6 de la conception
porte l'annotation de ces défauts ; ne pas y relire le `grep` non corrigé.

**Grille de lecture, sur un binaire portant le correctif F1** — chaque fenêtre
vivante émet une ligne par période, qu'elle porte le son ou non :

| Relevé | Lecture |
| --- | --- |
| `A = 0` | **Le contrôle n'a rien à dire.** Aucune session n'a vécu 30 s : ce n'est pas un verdict, c'est une mesure non prise |
| `A > 0`, `B = 0` | **Le défaut est là** : des fenêtres vivent, aucune ne porte le son |
| `A > 0`, `B > 0` | L'arbitrage désigne bien un porteur. Sur `N` fenêtres d'un même PID, attendre `B ≈ A / N` |
| `B == A`, plusieurs fenêtres d'un même PID | ⚠️ **Suspect** — c'est la signature exacte du code d'avant F1 |

> ✅ **CE `grep` A ÉTÉ JOUÉ, et F1 EST CONFIRMÉ** (D8, tâche 2, 4 août 2026,
> **une exécution — aucun taux**) : **A = 14, B = 7**, soit exactement la ligne
> `A > 0, B < A` de cette grille, avec `B = A / 2` sur **deux** fenêtres d'un
> **même** PID. La trace `compteurs audio` a donc bien été observée portant
> `actif=false`, ce que D7 déclarait n'avoir jamais vu. **La FORME du relevé
> prouve à elle seule que le montage discrimine** : sous le code d'avant F1 on
> attendrait `A ≈ 7` et `B == A`, incompatible avec 14/7.
>
> ⚠️ **DÉFAUT DU PLAN DE D8, trouvé avant dispatch, et qui est LE MÊME PIÈGE
> ENCORE UNE FOIS** : le brief prescrivait « deux fenêtres Bloc-notes (même
> exécutable, donc même groupe de PID) ». **C'est faux — deux `notepad.exe` sont
> deux PID distincts**, et à deux PID distincts `B == A` est le comportement
> **correct** : le contrôle n'aurait alors pas pu échouer. **Le défaut F1 rejoué
> sur la mesure écrite pour le révéler**, pour la troisième fois dans ce dépôt.
> Montage retenu : **deux fenêtres Chrome `--app` partageant un
> `--user-data-dir`** — un seul `chrome.exe`, PID 44600 vérifié par deux voies
> indépendantes (`EnumWindows` dans la session interactive, et les deux lignes
> `audio activé … pid=44600` de l'agent lui-même). **Relever les PID AVANT de
> conclure**, jamais les supposer d'un nom d'exécutable.
>
> ⚠️ **Découverte de méthode, coûteuse et réutilisable : une session WebRTC
> VIVANTE est requise pour que cette mesure dise quoi que ce soit.**
> `Session::run()` est la seule boucle qui consomme l'ordre audio du capteur ;
> un répondeur de viewport nu fait créer les sorties et lancer les enfants,
> mais **aucune session ne s'établit et les deux fenêtres restent `actif=false`
> à jamais** — un faux négatif de méthode, indiscernable du défaut. Et **le
> navigateur pilote doit tourner sur l'HÔTE, jamais sur la VM** : sur la VM la
> fenêtre de la page-shell est elle-même capturée par le superviseur, ce qui
> boucle en cascade d'ouvertures.
>
> ⚠️ **Réserve du relevé, à ne pas perdre** : **une seule exécution**, et **une
> bascule de porteur a eu lieu en cours de mesure**.

### ⚠️ La revue finale de branche — cinq défauts que seule une lecture TRANSVERSE pouvait voir

Treize revues par tâche étaient passées. Les cinq défauts ci-dessous ont en
commun de **franchir une frontière de tâche** : chacun est correct des deux
côtés pris séparément.

| # | Défaut | Remède |
| --- | --- | --- |
| **F1** | **Le détecteur de panne muette de la branche ne pouvait pas se déclencher** — voir la sous-section ci-dessus. Écrit tâche 7, prescrit tâche 12, jamais confronté | Le bloc `REPORT_INTERVAL` remonte au-dessus du gate `!emettait` |
| **F2** | **Un rattachement ne rendait PAS l'enfant muet**, contre la conception §4.4 : seul l'ordre EN ATTENTE était effacé (`None` = « rien à changer »), le drapeau `emet` gardait sa valeur d'avant la rupture | `self.audio = Some(false)` dans `capteur/distante.rs`, plus son test |
| **F3** | **Une erreur de lecture WASAPI silençait DÉFINITIVEMENT tout un groupe de PID**, en se déclarant en bonne santé | Tolérance à N erreurs (`audio.rs`, pur et testé) + témoin `capture_morte` au journal |
| **F4** | **`audio_bps` réservait 128 kb/s pour une piste inexistante** quand la source audio manque (échec d'ouverture — le « repli est le silence » de la spec §6 — ou `AUDIO=0`) | Le budget se conditionne à `actif && audio_source.is_some()` |
| **F6** | Un commentaire d'ancienneté promettait ce que le chemin réel ne tient pas (`oublier` vide `arrivees` avant qu'un rattachement ne réinscrive) | Le commentaire dit sa portée réelle ; **comportement inchangé** |

**Le cas de F2 mérite d'être retenu pour sa forme** : deux fenêtres A et B d'un
même PID, A porteuse, le capteur redémarre. B se rattache la première, son
groupe est vide côté capteur, elle est élue et démarre. A se rattache quelques
centaines de millisecondes plus tard **en émettant toujours** — les deux jouent
le même mix, désynchronisé : un **écho audible**, jusqu'à ce que l'ordre
d'extinction destiné à A arrive.

> ✅ **UN de ces cinq remèdes a depuis été exercé sur la VM : F1** (D8, tâche 2,
> **une exécution**, A=14 / B=7 — voir l'encadré du `grep` ci-dessus). **Les
> QUATRE autres — F2, F3, F4, F6 — n'ont TOUJOURS jamais tourné sur la VM**, et
> D8 ne les a pas approchés : sa recette n'a ni rompu de canal capteur→enfant
> (F2), ni provoqué d'erreur de lecture WASAPI (F3), ni exercé le cas sans
> source audio (F4). **Ne pas lire « F1 mesuré » comme « la vague de D7
> mesurée ».**

⚠️ **AUCUN de ces cinq remèdes n'a été exercé sur la VM.** Ils sont raisonnés,
compilés (`cargo check --target x86_64-pc-windows-gnu`, 10 avertissements
`dead_code` préexistants) et couverts par 403 tests d'hôte — dont **trois neufs
dont un a été vu ROUGE avant remède** (F2). Mais `windows_audio.rs` est
`#[cfg(windows)]` et **aucun test d'hôte ne peut couvrir F1 ni F3** : la
correction de F1 en particulier repose sur un **argument de flot de contrôle**,
pas sur une observation. **Le premier geste de D8 — le `grep` ci-dessus, sur une
session de plus de 30 s — est donc AUSSI la première mesure de F1** : il doit
rendre `A > 0` avec `B < A` dès que deux fenêtres d'un même PID vivent.

**F5 est délibérément HORS de cette vague** : l'identité d'une session par son
seul nom porte une course au `retirer` (préexistante, antérieure à D7). La
fermer demande une génération monotone par session, c'est-à-dire un changement
de conception du registre — **consignée pour le sous-bloc suivant, avec le
signal enfant→capteur que F3 laisse ouvert.**

### Ce que D7 n'établit PAS

- **Aucun taux nulle part** : 4 relevés au §1 (un par point), 2 exécutions du
  critère ①, 1 des critères ② ④ ⑤.
- **Les cinq correctifs de la revue finale de branche n'ont jamais tourné sur la
  VM** (voir l'encadré ci-dessus). En particulier, **la trace `compteurs audio`
  n'a JAMAIS été observée portant `actif=false`** — c'est ce que le premier
  `grep` de D8 établira, ou réfutera.
  ✅ **RÉPONDU par D8 (4 août 2026, une exécution) : `actif=false` A été
  observé — A=14, B=7, F1 CONFIRMÉ.** ⚠️ **Les quatre autres correctifs (F2,
  F3, F4, F6) n'ont toujours jamais tourné sur la VM.**
- **La promotion de la fenêtre voisine quand une capture meurt n'existe pas** :
  le capteur ne voit pas le témoin `capture_morte`, qui vit dans l'enfant. F3
  rend l'état observable au journal, il ne le rend pas actionnable.
- **Le critère ③ (l'audio survit au sommeil) n'a pas été exercé** — la lacune
  la plus lourde, à traiter en priorité si D8 en dépend.
  ✅ **EXERCÉ par D8, et TENU** (son critère ⑤, **une exécution**, celle du
  rejeu) : une session endormie reçoit toujours l'audio de **sa** fenêtre, la
  **fréquence dominante** reçue (522 Hz) étant celle qui lui est assignée
  (520 Hz, à la résolution du bin FFT près). ⚠️ **La première mesure de ce même
  critère était INVALIDE** — elle portait sur une fenêtre dont le contenu était
  inconnu, faute d'identité de session résolue.
- **Le plafond d'activations *process loopback* concurrentes reste inconnu** :
  deux au plus ont coexisté. C'est la forme exacte de l'inférence que D3 a dû
  payer sur DXGI, nommée ici plutôt que découverte.
- **Rien au-delà de deux fenêtres**, rien des applications UWP (dont le rendu
  audio peut ne pas vivre dans l'arbre du processus propriétaire —
  `INCLUDE_TARGET_PROCESS_TREE` ne les couvrirait pas), aucun jugement
  d'écoute — même lacune que `BPP_MIN` traîne depuis le chantier C volet 1.
- **Le plancher de 1 LSB n'est pas confronté au DTX d'Opus.** Le §« DTX » de la
  conception affirmait qu'une fenêtre porteuse dont l'application se tait
  « ne coûtera quasiment rien » ; la recette **ne l'a pas tranché** — ses
  sources jouaient toutes un son continu, et le cas d'une fenêtre porteuse
  **silencieuse** n'a jamais été monté. **Reste une inférence, jamais
  mesurée**, et le §2.2 apporte au contraire un indice qu'elle mérite d'être
  posée (`bytesReceived` qui croît sur un spectre à −1000 dB). L'annotation
  correspondante vit dans la conception, §2 « État des lieux, vérifié » — ne
  pas relire cette affirmation comme acquise.
- **Rien de la latence**, rien de la durée (session la plus longue < 1 min),
  aucun redimensionnement, aucun recouvrement, aucun déplacement de fenêtre,
  aucun clavier.
- **Les trois couches inconnues le restent** : le plafond de 8 encodeurs,
  celui de 4 processus, et le mécanisme de l'abandon du mutex DXGI.

### Pièges neufs — à connaître avant de toucher à ce terrain

- **`[Console]::Beep` reste un faux négatif audio** (chantier A, rejoué ici) :
  il passe par `kernel32!Beep` et ne traverse pas le périphérique de rendu de
  cette VM. Employer `Media.SoundPlayer` ou toute API multimédia réelle.
- **Une trace qui « prouve » un cycle `Stop()`/`Start()` doit compter des
  échantillons, pas seulement relire un `HRESULT` de succès.** Une revue
  antérieure à la mesure avait déjà signalé que `"cycle Stop puis Start
  accepte"` ne prouve que le retour de `Start`, jamais que l'audio a repris —
  corrigé avant la mesure par l'ajout d'un compte d'échantillons post-cycle.
- **Un `grep` de recette écrit pour révéler un mode de défaillance silencieux
  doit lui-même être exécuté avant d'être prescrit** — voir la sous-section
  ci-dessus : les deux premiers défauts du `grep` de D8 n'ont été trouvés qu'en
  le faisant tourner.
  ⚠️ **ET L'EXÉCUTER NE SUFFIT PAS : le troisième défaut, le seul qui annulait
  le contrôle, a survécu à son exécution** (F1). Le `grep` avait tourné, rendu
  0/0, et ce 0/0 avait été correctement diagnostiqué comme « session trop
  courte » — ce qu'il était. Ce qu'aucune exécution ne pouvait montrer, c'est
  que le champ observé **n'avait qu'une seule valeur atteignable**. **La règle
  complète est donc en deux temps : exécuter le contrôle, PUIS provoquer
  délibérément l'état qu'il doit dénoncer et vérifier qu'il le dénonce.** Un
  contrôle qu'on n'a jamais vu ROUGE n'est pas un contrôle — c'est exactement
  la doctrine que ce dépôt applique déjà à ses tests.
- ⚠️ **Un défaut peut être invisible à treize revues et sauter à la
  quatorzième, sans que personne n'ait mal lu** : les cinq défauts de la revue
  finale de branche franchissent tous une **frontière de tâche** (F1 : écrit
  tâche 7, prescrit tâche 12 ; F2 : conception §4.4 contre le rattachement de
  D4 ; F4 : le repli silencieux de la spec §6 contre le budget de la tâche 8).
  Chacun est correct des deux côtés pris séparément. **Une revue par tâche ne
  peut structurellement pas les voir** — ce n'est pas un défaut de rigueur,
  c'est une propriété du découpage, et cela justifie à soi seul la revue
  transverse.

### La suite : D8, et les consignations

- **D8** — **le plein écran et Keyboard Lock**, précédé de son geste d'ouverture
  (le `grep` ci-dessus, qui est **aussi la première mesure de F1**).
  ✅ **FAIT le 5 août 2026**, geste d'ouverture compris (**F1 CONFIRMÉ**,
  A=14 / B=7, une exécution). ⚠️ **Keyboard Lock n'a PAS été mesuré**, par
  décision : l'instrument n'entre pas réellement en plein écran. Voir la section
  « Sous-bloc D8 » plus bas.

**Ce que D7 lègue, à porter dans le plan de D8 plutôt qu'à redécouvrir :**

1. ✅ **Exercer le critère ③** (l'audio d'une fenêtre endormie) — la lacune la
   plus lourde de la recette D7. **FAIT par D8 (son critère ⑤), TENU, une
   exécution.**
2. ❌ **Le signal enfant→capteur quand une capture meurt** (F3, hors périmètre) :
   sans lui, la fenêtre voisine n'est jamais promue et le groupe reste muet.
   **NON TRAITÉ par D8 — toujours dû.**
3. ❌ **L'identité d'une session par génération monotone, pas par son seul nom**
   (F5, préexistant) : changement de conception du registre. **NON TRAITÉ par
   D8 — toujours dû.**
4. ❌ **Les trois consignations de D6 non traitées par D7** : `TICKS`/`CAPTURED`/
   `PRODUCED` par session, le critère ④ à palier de 45 à 60 s, et l'A/B
   différentiel sur `set_desired_bitrate`. **AUCUNE des trois n'a été traitée
   par D8 — toutes trois toujours dues.**
- **`agent/src/wasapi.rs` a un jumeau `#[cfg(windows)]` neuf, sans test** :
  `agent/src/wasapi/process_loopback.rs` (402 lignes). Toute la machinerie COM
  du *process loopback* y vit désormais — activation, complétion asynchrone,
  sonde — hors de `wasapi.rs`, qui retombe à 352 lignes et sort de la dette
  gelée. Voir le tableau de dette en tête de ce fichier.

---

## 🖼️⛶ Sous-bloc D8 — le plein écran par fenêtre (5 août 2026)

Résultats complets :
`docs/superpowers/plans/2026-08-04-multifenetres-plein-ecran-resultats.md`.
Conception : `docs/superpowers/specs/2026-08-04-multifenetres-plein-ecran-design.md`.
Journaux : `docs/superpowers/plans/journaux-multifenetres-d8/` — **16 fichiers
suivis par git**, et **QUATRE familles de lecture** — trois d'encodage comme
D6, plus une distinction de fin de ligne que la rédaction précédente avait
tranchée à tort :

| Famille | État | Ce qu'il faut faire |
| --- | --- | --- |
| `agent-recette.log`, `agent-rejeu-2-5.log`, `dxgi-controle-preparation.log`, `mode-sortie-*.log` | UTF-8, **ANSI déjà retirées**, CRLF | rien |
| `p2-instrument.log`, `p0-f1.log` | UTF-8 (`p0-f1.log` pur ASCII), **ANSI déjà retirées**, **LF — aucun `\r`** | rien. ⚠️ *Ils étaient rangés « CRLF » avec les précédents : c'est faux, et corrigé ici.* |
| `p0-agent.log`, `p1-mode-sortie.log`, `p1bis-mode-sortie.log` | UTF-8, **séquences ANSI PRÉSENTES** | `sed 's/\x1b\[[0-9;]*m//g'` avant tout `grep` |
| `critere-recette.log`, `critere-rejeu-2-5.log` (journaux de **pilote**) | classés « data » — **une seule ligne** y porte 17 `\x02` et 3 `\x03`, résidu du PowerShell de `run-agent.sh` | `grep -a`. ⚠️ **Contrairement à D6, les accents sont INTACTS** et se `grep`ent normalement : le défaut à deux réglages n'a mordu qu'une ligne |

D8 livre le sens **Windows → navigateur** du plein écran, que le cadrage jeux
laissait au chantier D : quand une application Windows passe en plein écran, sa
fenêtre navigateur y entre au geste suivant — **et elle seule**. Il devait
livrer aussi la **résolution qui suit** (la sortie virtuelle change de mode) et
éprouver **Keyboard Lock**.

### ⛔ CE QUI EST RÉELLEMENT LIVRÉ : la détection et l'annonce, pas le changement de mode

**Décision du propriétaire du dépôt à la revue finale de branche (5 août
2026).** La moitié « changement de mode de sortie » **n'a JAMAIS été sollicitée
par la recette** — critère ② NON EXERCÉ, `mode_sortie_demande=0` aux deux
exécutions, **zéro tentative** de `ChangeDisplaySettingsExW` en conditions de
produit — et la revue y a trouvé **deux Critiques**. Elle reste dans le code,
**désarmée par défaut**, derrière `PLEIN_ECRAN_MODE_SORTIE=1` (voir le tableau
des variables). **C'est le repli que le §4 de la conception avait écrit
d'avance : « ①, ③, ④ et ⑤ tiennent sans ② ».**

> ❌ **ELLE NE RESTE PLUS DANS LE CODE. Le sous-bloc D9 l'a MESURÉE puis
> RETIRÉE (6 août 2026).** L'inconnue éliminatoire est tranchée dans le sens
> favorable — le pilote **accepte**, duplication ouverte, 2/2 — mais le
> changement **ne survit pas** à la création de la sortie suivante (`n = 4`
> exécutions propres, sous `CDS_UPDATEREGISTRY` **comme** sous `flags = 0`), et
> `CDS_UPDATEREGISTRY` **pollue le registre** (attribuable par GUID, 3
> transitions probantes). **`PLEIN_ECRAN_MODE_SORTIE`, `changement_de_mode_arme`,
> `reconstruire_sur_la_sortie` et `windows_source/redimensionnement/mode_sortie.rs`
> n'existent plus.** La colonne « DÉSARMÉ » du tableau ci-dessous est devenue la
> colonne « SUPPRIMÉ » ; la colonne « ACTIF » est inchangée. Voir la section D9.

| ACTIF, livré, mesuré | ~~DÉSARMÉ~~ **SUPPRIMÉ (D9)** |
| --- | --- |
| relecture du style (`capteur/fenetre.rs`, `PERIODE_STYLE`) | ~~`changer_mode_de_sortie`~~ |
| `DepuisCapteur::PleinEcran` → `AgentControl::Fullscreen` | ~~tout `ChangeDisplaySettingsExW` du produit~~ |
| armement client (`client/src/fullscreen.ts`) et Keyboard Lock | — |

⚠️ **Les deux Critiques ne sont PAS corrigées, et c'est délibéré** — le
désarmement les rend inatteignables. Elles vivent auprès du garde
(`agent/src/capteur/plein_ecran.rs::changement_de_mode_arme`) comme **le
premier travail de la recette qui armera ce chemin** :

> ⛔ **IL N'Y AURA PAS DE RECETTE QUI ARME CE CHEMIN : D9 l'a retiré (6 août
> 2026), et le garde avec lui.** C1 et C2 **cessent d'exister comme dettes de
> code**. ⚠️ **Mais C1 laisse une trace vivante que le retrait ne défait pas** :
> la pollution DÉJÀ écrite au registre bloque le produit à **trois** fenêtres
> sur cette VM, et **rien ne nettoie derrière** (legs n°4 de D9). Ce qui suit
> reste le diagnostic de D8, conservé pour lui-même.

- **C1 — le produit bloquerait ses propres ouvertures de fenêtre
  ultérieures.** Il écrit `CDS_UPDATEREGISTRY` à **chaque** plein écran
  réussi, et une sortie **naît à la dernière taille laissée au registre** :
  c'est littéralement le blocage que la préparation de la recette a dû lever à
  la main, et le produit se l'infligerait **en marche normale**.
  ⚠️ **Portée INCONNUE, et les deux branches aggravent** : `agent-recette.log`
  porte **CINQ GUID SudoVDA distincts** (`…677541430001` à `…430005`), un par
  sortie. Ou le mode registre est **par GUID** — et lever le blocage sur un
  seul ne pouvait rien pour les quatre autres, donc **la chaîne causale
  « remède P1 → blocage levé » n'est pas fermée par ses pièces** —, ou il ne
  l'est pas — et **une seule écriture empoisonne TOUTES les sorties futures**.
  *(Le document de résultats parlait du « GUID de sortie virtuelle unique » :
  c'était faux, corrigé.)*
- **C2 — la reprise sur perte d'accès de D2 est court-circuitée.** Après un
  changement de mode, `reconstruire_sur_la_sortie` rend une `Err` sur un échec
  de réouverture **y compris transitoire** — la classe d'échec exacte que la
  fenêtre de reprise de D2 existe pour encaisser (44 pertes `0x887A0026`
  absorbées sans tuer une session). Ici, la session meurt.

**Une troisième raison, qui n'est pas un défaut mais un manque** : ce chemin
**n'a jamais tourné en conditions de produit** — P1 n'ouvre jamais de
`DuplicateOutput`, la production retaille une sortie dont la duplication est
ouverte et détenue jusqu'à 3,1 s. C'est la première des trois inconnues.

### ⛔ Le fait de conception le plus réutilisable : le critère de détection du cadrage est MORT

**C'est ce qu'il faut retenir de D8 si l'on n'en retient qu'une chose, et cela
concerne quiconque relira le cadrage jeux** (§4.1 de
`docs/superpowers/specs/2026-07-28-support-jeux-design.md`).

Le §4.1 prescrit de détecter le plein écran par « comparaison de `GetWindowRect`
avec le rect du moniteur ». **Depuis D1, chaque fenêtre est seule sur sa propre
sortie virtuelle et l'occupe exactement**, et le superviseur le lui réimpose
périodiquement (`controler_le_placement`, **`agent/src/superviseur/boucle/placement_periodique.rs:21`**) :
« rect fenêtre == rect moniteur » est donc **l'état NOMINAL**, pas l'état plein
écran.

⚠️ **Le critère n'est pas seulement inopérant : il est TOUJOURS VRAI.** Une
implémentation fidèle à la lettre du cadrage annoncerait le plein écran **en
permanence, pour toutes les fenêtres**. **Le signal survivant est la perte des
styles de bordure**, pas le rectangle.

Le **troisième** signal du §4.1 est écarté pour une autre raison, à connaître
aussi : `SHQueryUserNotificationState()` rendant `QUNS_RUNNING_D3D_FULL_SCREEN`
est **global à la session interactive, pas par fenêtre** — à N fenêtres il ne
dit pas *laquelle* —, et il ne voit pas le « borderless fullscreen » que la
quasi-totalité des jeux modernes emploient.

### Le verdict, avec le nombre d'exécutions dans chaque énoncé

**Deux exécutions de RECETTE** : la recette complète (`recette`) et un
rejeu ciblé de ② et ⑤ avec un instrument corrigé (`rejeu-2-5`). ⚠️ **Le
sous-bloc en compte TROIS au total** — la troisième est le run de **P0**, la
mesure de F1, dont le verdict vit dans la section D7 (c'est là qu'on va chercher
F1) et **une seule exécution** également. Binaire mesuré :
commit `7032b01`, `agent.exe` **9 248 256 octets**, identique aux deux
exécutions — seul le pilote a été corrigé entre elles. **Aucun taux n'est
revendiqué nulle part.**

| # | Critère | Verdict | Exécutions |
| --- | --- | --- | --- |
| témoin | Non-régression, aucun plein écran | **TENU** | **1** |
| ① | Le plein écran Windows est détecté et annoncé, **à la bonne fenêtre seule** | **CONFIRMÉ** — détection exclusive et symétrique. ⚠️ **La corroboration est PARTIELLEMENT CIRCULAIRE** : `resoudreIdentite()` identifie la session **par la bascule de bordure**, donc **par ① lui-même** — elle corrobore la cohérence de deux emplois du même mécanisme, pas ① par un signal indépendant | **1** mesure + **1** corroboration (voir la réserve ci-contre) |
| ② | Le flux suit le viewport, la sortie garde son nom | **NON EXERCÉ** — zéro tentative de changement de mode, aux **deux** exécutions | **2** |
| ③ | Échap et Keyboard Lock | **NON MESURÉ, par décision** actée en amont | **0** |
| ④ | Les voisines s'endorment par le chemin existant | **TENU** sur ses deux moitiés | **2** |
| ⑤ | L'audio d'une endormie survit (legs de D7) | **TENU** — dominante à la fréquence assignée | **1** mesure corrigée (**1** mesure initiale invalidée) |

**① en détail** : exactement **deux** lignes `plein ecran de la fenetre Windows`
sur tout le run, toutes deux `session=w-4`, une `actif=true`, une `actif=false`.
La bascule de style Windows est confirmée par **relecture directe**
(`GetWindowLongPtrW` avant/après, jamais le seul code de retour) :
`avant=382664704` → `après=369819648`.
⚠️ **CORRIGÉ (revue finale de branche) : le « ~0,5 s » publié ici comme
« latence bascule → détection » est une latence ENVOI → DÉTECTION, et c'est une
BORNE SUPÉRIEURE, pas une caractéristique du produit.** Les deux horodatages
qui la fondaient (~21:03:41,4 et ~21:05:24,1) **ne figurent dans AUCUN
journal** : ils se reconstituent en retranchant le `dodo(4000)` du pilote de
l'horodatage de la commande. Entre l'envoi de la tâche planifiée et le
`SetWindowLongPtrW` réel s'intercalent `schtasks`, le démarrage de PowerShell
et un `Add-Type` — tout ce temps est compté DANS le 0,5 s. **La latence propre
du détecteur est donc inférieure, d'une quantité non mesurée**, et elle est
bornée par ailleurs par `PERIODE_STYLE = 250 ms`. Ce qui reste relevé et
juste : **la même borne se retrouve dans les deux sens** (activation et
restauration).

**④ en détail, et le piège de lecture qu'il porte** : les latences **AGENT**
(ordre reçu → transition) sont **sommeil 37 ms / 28 ms** et **réveil 471 ms /
119 ms** selon l'exécution. ⚠️ **Les « 2,2 s » et « 10,2 s » qu'une rédaction
intermédiaire publiait ne sont PAS des latences produit** : ce sont les délais
de confirmation du **pilote**, dont le 10,2 s est un pur artefact
d'échantillonnage (le test lit la prochaine ligne `cadence du capteur`, de
période **10 s**). C'est le piège maison « un compteur de journal peut compter
des LIGNES et non des ÉVÉNEMENTS », rejoué sur une latence. **Facteur ~86 entre
le chiffre faux et le vrai.**

### 🔴 Les trois inconnues restent ENTIÈRES, et la première commande les deux autres

**Aucun changement de mode n'a été sollicité de toute la recette**
(`mode_sortie_demande=0` aux **deux** exécutions). Les trois inconnues que le
brief posait comme risque n°1 sont donc **exactement aussi ouvertes qu'avant** :

1. ⛔ **Le pilote SudoVDA accepte-t-il un changement de mode sur une sortie DONT
   LA DUPLICATION EST OUVERTE ?** **NON TRANCHÉE.** La sonde P1 n'ouvre **jamais**
   de `DuplicateOutput` — c'est l'écart banc/produit, et c'est le cas du produit.
   **C'est le premier travail de toute recette suivante.**
   ⚠️ **Je la qualifie d'ÉLIMINATOIRE, et ce mot est de moi** : ni la conception
   ni le brief ne le portent. Il se dérive du fait que les deux autres inconnues
   sont **sans objet tant que celle-ci n'est pas tranchée**, et que ② ne peut pas
   être exercé sans elle — c'est un durcissement raisonné, pas une transcription.
2. **Combien de pertes d'accès `0x887a0026` un changement de mode inflige-t-il
   aux voisines ?** **SANS OBJET** : `pertes_acces_voisines: []` reflète
   l'absence de toute tentative, **pas** l'absence de pertes. Les pertes relevées
   (6 au run initial, 1 au rejeu) sont des réouvertures de routine.
3. **La sortie garde-t-elle son nom `\\.\DISPLAYn` ?** **SANS OBJET** pour la
   même raison. ⚠️ **Ne pas lire `conserve: false` comme un refus** : c'est
   l'absence de mesure qui le rend faux.

**Où la chaîne casse, mieux localisé qu'avant, et pas plus loin** : le rejeu
instrumente `window.innerWidth`/`innerHeight` autour des appels CDP et montre
que **le viewport atteint sa cible EXACTEMENT** (1280×720 → 1920×1080, puis
→ 3840×2160), y compris au-delà de l'écran émulé. **L'hypothèse d'un
plafonnement par `--ozone-override-screen-size` est donc RÉFUTÉE par mesure
directe.** La rupture est **entre `window.innerWidth` et l'émission du message
`Resize`** — très probablement le `ResizeObserver` sur l'élément `<video>`
(`client/src/main.ts`), dont `clientWidth`/`clientHeight` ne suivent
`window.innerWidth` que si la mise en page CSS le permet. ⚠️ **Ceci n'est PAS
vérifié plus loin** : `video.clientWidth` n'a pas été instrumenté.

❌ **La « pièce corroborante qui disculpait le transport » est RÉFUTÉE PAR SES
PROPRES JOURNAUX** (C3, revue finale de branche, 5 août 2026). Elle disait :
« l'agent reçoit **22 messages de contrôle** sur le run initial — 20
`Visibility`, 2 `Resize` (les deux à la connexion) ; le canal vit et délivre
pour les cinq sessions **pendant toute la phase ②** ; le défaut est côté
ÉMISSION du client. » **Le 22 est exact, mais il porte sur TOUT LE RUN — et la
phase ② est précisément l'intervalle MUET** : phase `21:03:35.810Z` →
`21:05:23.233Z`, **zéro** ligne `contrôle reçu` dedans, la dernière à
`21:03:31.785`, la suivante à `21:05:52.524`, soit **≈ 141 s de silence**, plus
long que la phase entière. **Idem au rejeu** (phase `21:40:42.013Z` →
`21:41:27.840Z`, zéro dans la fenêtre). Et **« pour les cinq sessions » n'est
étayé par rien** : les lignes `contrôle reçu` sortent d'`agent::demarrage`
**sans champ `session` ni span**.

⚠️ **Conséquence, et c'est ce qu'il faut retenir : LE TRANSPORT N'EST DISCULPÉ
PAR AUCUNE PIÈCE.** C'était le seul argument qui situait la rupture de ② côté
client ; le legs n°8 (instrumenter `video.clientWidth`) vise donc **une
hypothèse parmi d'autres**, plus le maillon désigné, et **le canal de contrôle
reste suspect**. *Le verdict de ② ne bouge pas — « NON EXERCÉ, cause inconnue »
reste vrai, et l'est DAVANTAGE.*

❌ **Le « fait annexe » est réfuté comme ÉNONCÉ (I8)** : « sur 5 sessions, 3
n'ont jamais émis même leur `Resize` initial » **n'est pas dérivable**. Ce qui
est relevé est **2 `Resize` pour 5 sessions** ; ces deux lignes ne portant
**aucun champ `session`**, rien n'établit qu'elles viennent de deux sessions
**distinctes**. Le « donc 3 » était une inférence sous hypothèse tacite,
publiée comme un fait **et inscrite en dette** (legs n°10, corrigé). **Ce qui
reste ouvert est *pourquoi si peu de `Resize`*, pas *lesquelles n'en ont pas
émis*.**

### ⚠️ La conséquence produit du refus net : un client 16:10 ou 3:2 n'obtient RIEN

Mesure de la tâche 9 (commit `d5ce288`, journal `mode-sortie-1728x1080.log`) :
**un mode NON ANNONCÉ ne fait JAMAIS bouger la sortie**, aux trois combinaisons
de drapeaux.

❌ **CORRIGÉ (I5, revue finale de branche) : « refusé NET aux TROIS
combinaisons » est FAUX — c'est vrai de DEUX. La troisième annonce un
SUCCÈS.**

| Combinaison | Code | `api_annonce_succes` | Sortie relue | `conforme` |
| --- | --- | --- | --- | --- |
| `CDS_UPDATEREGISTRY` seul | **−2** (`DISP_CHANGE_BADMODE`) | `false` | 1920×1080, **inchangée** | `false` |
| `CDS_UPDATEREGISTRY \| CDS_RESET` | **−2** | `false` | 1920×1080, **inchangée** | `false` |
| `CDS_UPDATEREGISTRY\|CDS_NORESET` puis `CDS_RESET` seul | `premier=-2`, **`second=0`** | **`true`** | 1920×1080, **inchangée** | `false` |

⚠️ **La troisième ligne est un REFUS DÉGUISÉ EN SUCCÈS** — l'API rend `0` sur
une sortie qui n'a pas bougé d'un pixel. C'est exactement ce contre quoi ce
dépôt a bâti sa doctrine : **juger sur la relecture DXGI, jamais sur le code de
retour.** **Le produit n'est pas affecté** (il n'emploie que
`CDS_UPDATEREGISTRY` seul, et juge sur la relecture), **mais un successeur qui
adopterait l'idiome multi-écran sur la foi de la phrase d'avant croirait avoir
réussi.**

**La sortie n'annonce que NEUF modes** — 640×360, 800×600, 960×540, 1280×720,
1366×768, 1600×900, 1920×1080, 2560×1440, 3840×2160 —, identiques aux
occasions où ils ont été énumérés.
❌ **CORRIGÉ (I6, même revue) : « huit en 16:9 exactement » est FAUX — il y en
a SEPT.** Le décompte juste : **7 en 16:9 exact** (640×360, 960×540, 1280×720,
1600×900, 1920×1080, 2560×1440, 3840×2160), **1 en 4:3** (800×600), et **1 qui
n'est NI l'un NI l'autre — `1366×768`** (1366 × 9 = 12 294 ≠ 768 × 16 =
12 288). Le mot « exactement » interdisait de le lire comme une approximation.
**La conclusion aval SURVIT** : `borner_a_la_taille_max` **préserve le rapport
d'aspect**.

⚠️ **Donc un client 16:10 ou 3:2 — les formats d'ordinateurs portables les plus
courants — en plein écran produit une taille hors liste, refusée net, et
n'obtient AUCUN changement de résolution.** Ce n'est pas un cas limite : c'est
le cas **nominal** pour ces machines. Et le refus coûte **~3 s de gel du fil de
fenêtre à chaque redimensionnement** — c'est le budget entier de l'unique
combinaison de drapeaux que le **code de production** tente. *(Ne pas imputer ce
coût aux « deux tentatives de la sonde » : la sonde P1 en enchaîne deux, le
produit une seule, et c'est le chemin du produit qui est décrit ici.)*

### ⚠️ Le défaut HiDPI, OUVERT côté client, et structurellement invisible à ce montage

Le court-circuit anti-`Resize`-de-routine (celui qui évite un changement de mode
parasite à chaque connexion) **ne tient qu'à `devicePixelRatio == 1`** : la
sortie est créée sur `innerWidth` **sans** dpr, alors que le `ResizeObserver`
rapporte **avec** dpr. **Un client HiDPI déclencherait donc un changement de
mode à CHAQUE connexion de CHAQUE fenêtre**, avec un écart de 25 à 100 %.

**Décision motivée, prise et assumée** : le défaut **reste ouvert côté client**,
parce que le chemin mono-fenêtre `FenetreRecadree` a réellement besoin de la
valeur multipliée par le dpr, et qu'unifier les deux unités risquerait une
régression qu'**aucun test d'hôte ne rattraperait**. Le commentaire du code dit
désormais la condition exacte et nomme **`PLEIN_ECRAN=0` comme seule parade**.

⚠️ **Le montage de recette NE PEUT PAS le voir** : Chrome sans interface,
`deviceScaleFactor: 1` partout. Le défaut n'a donc pu ni se manifester ni être
réfuté — **son absence des relevés n'est pas une information.**

### 🧬 Une sortie ne naît PAS à la taille demandée — confirmé, reproduit, non expliqué

**Le pilote ignore la taille passée à la création et hérite de la dernière taille
laissée au registre** par un `CDS_UPDATEREGISTRY` antérieur : chaîne
`avant(N) = après(N-1)` vérifiée sur **trois transitions consécutives**
(tâche 3bis). **Ce n'est pas une lecture de commentaire, c'est une mesure**, et
elle touche une hypothèse que D1 à D7 tenaient pour acquise.
⚠️ **Réserve de la tâche 3bis, non transportée jusqu'ici et rétablie par la
revue finale de branche** : ces trois transitions reposent sur **UN SEUL
journal brut versé**, donc **une exécution**. `task-3bis-report.md` le dit en
toutes lettres — trois exécutions au total, dont l'une « non versée en brut
(nettoyée par erreur avant copie) », reproduite à plat dans le rapport. **La
chaîne est cohérente et reproduite ; elle n'est pas rejouable sur pièces.**

**Second fait, qui ressuscite le risque de changement PARTIEL** : le pilote peut
**« snapper »** vers une taille intermédiaire — **3840×2160 demandé (pourtant
ANNONCÉ) rend 2560×1440**, `mouvement_observe=true`,
`cible_exacte_atteinte=false`. **Mécanisme non expliqué.** C'est exactement le
cas que le correctif de la tâche 9 traite : **reconstruire dès que la taille a
bougé, quelle que soit la distance à la cible** — comparer la taille relue à
`self.width/height`, jamais à la seule demande.

**Et cette persistance bloque le PRODUIT, pas seulement une sonde** : voir le
piège correspondant en section D2.

### 🩹 La leçon de méthode centrale — le mode de défaillance dominant, rejoué DEUX fois

**C'est le sous-bloc qui existait pour ne plus commettre ce défaut, et il l'a
commis deux fois.**

**① Le premier verdict P1 était INVALIDE — son critère ne pouvait pas rendre
l'autre valeur.** La sonde a demandé à la sortie **la taille qu'elle avait
déjà** (1920×1080, héritée du registre, alors qu'elle croyait avoir créé du
1280×720) : son critère `dernière_taille == cible` était **vrai AVANT toute
tentative**, et `ChangeDisplaySettingsExW` n'a jamais eu l'occasion de changer
quoi que ce soit. **C'est le défaut F1 de D7 rejoué sur l'instrument construit
pour l'éviter** — ni la revue de la tâche 3, ni le pilote du sous-bloc ne l'ont
vu ; le relecteur de la tâche 10 a relevé la « curiosité » sans en tirer la
conclusion. **Et c'était la prémisse de toute la tâche 9.** Rejoué proprement
(tâche 3bis) : la sonde **exclut structurellement** la taille courante
(`P1 NON MESURABLE` si aucun mode n'en diffère) et **juge sur le mouvement, plus
sur une égalité** — mouvement DXGI confirmé **5 fois**. `p1-mode-sortie.log` est
**conservé versé** : c'est la pièce du défaut.

**② Puis le document de recette a porté TROIS affirmations que ses PROPRES
pièces réfutaient**, toutes trois en route vers ce fichier-ci :

- ④ « réveil non confirmé » — **faux** : le réveil a eu lieu en 471 ms, trois
  lignes le prouvent dans la fenêtre de 30 s. Cause : le parseur du pilote
  extrayait la session par `champ(l,'session') ?? sessionDeLigne(l)` ; sur une
  ligne portant le span `fenetre{session=w-2}:`, le premier terme capture
  **`w-2}:`** (accolade et deux-points compris, `\S+`), valeur fausse mais
  **truthy**, donc le `??` ne s'évalue **jamais**. **Troisième fois que ce dépôt
  paie « vérifier qu'un contrôle peut réussir » — et dans la fonction même dont
  le commentaire se félicitait de l'avoir corrigé à la ronde précédente.**
- « 5 sessions pour 3 fenêtres » attribué au fantôme Paint/Bloc-notes — **c'est
  l'explication inverse** : les horodatages disent que **chaque** fenêtre du
  pilote a produit **exactement une** session, et que `w-1`/`w-2`
  **préexistaient de 2,2 s**. Le pilote journalisait « VM sans fenêtre
  éligible » **sans jamais le vérifier**.
- ① parlant d'une « fuite » du message vers une voisine — **inexistante** : deux
  lignes en tout, toutes deux sur la même session.

**Puis une SECONDE ronde de revue, sur le document déjà corrigé, a trouvé deux
casses neuves** (la latence de réveil mêlant mesure agent et artefact
d'échantillonnage ; une pièce d'appui citée **à l'envers** — c'est la session
**focalisée** qui porte « adaptation indisponible », les **quatre** non
focalisées qui portent « Image réduite par le réseau ») et une lacune de
provenance. **Corriger une affirmation fausse en produit une autre : ce dépôt
l'avait déjà écrit après D6, et l'a repayé ici.**

### Ce que le code livre

| Étage | Fichier | Nature |
| --- | --- | --- |
| le prédicat | `agent/src/capteur/plein_ecran.rs` (~~195~~ ~~263~~ **212**, D9) | **pur, aucun `cfg`**, **9** tests d'hôte (`cargo test -p agent capteur::plein_ecran` → `9 passed`). Seule `GetWindowLongPtrW` est `#[cfg(windows)]`. ⚠️ **−51 en D9** : le garde `changement_de_mode_arme` disparaît avec le mécanisme, et l'en-tête devient le **constat de mesure** que cinq commentaires du dépôt citent |
| l'état de référence | idem — `SuiviBordure` | **l'état lu à l'attache fait référence, on n'annonce que les CHANGEMENTS** : une application née sans bordure n'annonce rien |
| la lecture | `agent/src/capteur/fenetre.rs` (~~470~~ **485** au 6 août 2026) | sur le fil de fenêtre, bridée à `PERIODE_STYLE = 250 ms`, **jamais à l'image**. Constante **propre** à ce mécanisme — ne pas la coupler à `PERIODE_REARBITRAGE` |
| le message | `capteur/protocole.rs` (~~369~~ **381** au 6 août 2026) → `proto/src/control.rs` (**419**) | `DepuisCapteur::PleinEcran { actif }` → `AgentControl::Fullscreen { active }`, le trajet exact de `Sommeil` |
| ❌ ~~le changement de mode, **DÉSARMÉ par défaut**~~ **RETIRÉ le 6 août 2026 (D9, tâche 3), sur mesure** | ~~`agent/src/windows_source/redimensionnement/mode_sortie.rs`~~ **le fichier n'existe plus** (~~427~~ ~~446~~ ~~458~~) | ~~`ChangeDisplaySettingsExW` seul.~~ Voir la section D9. ⚠️ **`TAILLE_MAX_SORTIE = (1920, 1080)` et `borner_a_la_taille_max` SURVIVENT, dans `agent/src/windows_source/sortie.rs`** — mais `borner_a_la_taille_max` n'a **plus aucun appelant**, son unique appelant étant parti avec le changement de mode. *(Les numéros de ligne `:99`/`:113` ont dérivé avec le fichier : ne pas les recopier.)* |
| ❌ ~~le garde du changement de mode~~ **DISPARU avec lui (D9)** | ~~`agent/src/capteur/plein_ecran.rs::changement_de_mode_arme`~~ | ~~`PLEIN_ECRAN_MODE_SORTIE=1` **arme** ; désarmé par défaut. Porte C1 et C2 auprès de lui.~~ **`PLEIN_ECRAN_MODE_SORTIE` N'EXISTE PLUS** — ni dans le code, ni dans `scripts/run-agent.sh`. Les legs C1/C2 (12 et 13) **cessent d'exister** au lieu d'être différés. `windows_source/redimensionnement.rs` (~~327~~ ~~345~~ **252**) |
| le client | `client/src/fullscreen.ts` (**174**) | **armement, pas action** : on entre au premier `pointerdown`/`keydown`. `Fullscreen { active: false }` sort immédiatement |

> ✅ **Les six chiffres de ce tableau sont RELEVÉS PAR LA COMMANDE le 6 août
> 2026**, à la vague de correction de la revue finale de branche. **Deux ont
> bougé sous cette vague même** : `plein_ecran.rs` 195 → **263** (le garde et
> ses raisons) et `mode_sortie.rs` 427 → **458** (la correction I5, la note
> HiDPI, puis l'en-tête de module conditionnalisée à la re-revue). Les quatre autres — `capteur/fenetre.rs` ~~470~~ **485** (D9),
> `capteur/protocole.rs` ~~369~~ **381** (D9), `proto/src/control.rs` **419**,
> `client/src/fullscreen.ts` **174** — étaient inchangés et exacts CE JOUR-LÀ ; **deux ont bougé sous D9**. **Aucun
> n'approche 500** ; la marge la plus étroite est celle de `fenetre.rs`, **30**.
>
> ❌ **CETTE ANNOTATION A ÉLLE-MÊME PORTÉ UN FAUX, et c'est le naufrage du
> « 487 » une CINQUIÈME fois** (relevé à la re-revue de la vague, 6 août 2026).
> Elle affirmait que les deux chiffres étaient « barrés **à leur place** plutôt
> que corrigés ailleurs ». **« Leur place » comptait DEUX tableaux** : celui-ci
> et le **récapitulatif de tête** (§ « Conventions de code », « Fichiers que D8
> a fait bouger ») — dont ce fichier écrit trois fois qu'il est le seul qu'on
> lise pour savoir de quelle marge on dispose. **Seul celui-ci avait été
> traité.** Le tableau de tête portait donc **trois** chiffres faux, dont un
> — `redimensionnement.rs` **292 → 327** — que le commit correcteur **n'a même
> pas nommé**, alors que sa valeur juste figurait dans une ligne que **ce même
> commit venait d'ajouter** quelques lignes plus haut.
>
> **Les trois sont désormais corrigés à leur place** : `mode_sortie.rs`
> 427 → **458**, `redimensionnement.rs` 292 → **345**, `plein_ecran.rs`
> 195 → **263**. Les neuf autres lignes du tableau de tête ont été **remesurées
> par la commande** et sont exactes.
>
> ⚠️ **Les deux premiers ont ENCORE bougé pendant la correction elle-même**
> (`446` → **458**, `327` → **345**), la re-revue exigeant par ailleurs de
> conditionner à l'armement les deux en-têtes de module et la phrase
> d'ouverture de `resize`. **Ils sont relevés APRÈS ces éditions, pas avant** —
> une table corrigée sur une mesure prise en début de ronde serait fausse à la
> fin de la même ronde. C'est la forme la plus discrète de la dérive, et la
> plus facile à commettre en croyant bien faire.
>
> ✅ **SIXIÈME occurrence, et elle est ANNONCÉE plutôt que subie (6 août 2026,
> revue transverse de fin de branche D9).** Les six chiffres de ce tableau ont
> tous rebougé sous D9 ou disparu avec leur fichier ; ils sont barrés ici **et**
> dans le récapitulatif de tête, les deux places énumérées AVANT d'écrire quoi
> que ce soit, par `grep -n '<le nombre>' CLAUDE.md`. Le relevé D9, seul faisant
> foi, vit dans le § « Conventions de code ».
>
> ⚠️ **La leçon n'est pas « recompter » — c'est que « corrigé à sa place » est
> une affirmation de COMPLÉTUDE, et qu'une affirmation de complétude se
> vérifie en énumérant les places AVANT de l'écrire.** Le geste qui manquait
> tient en une commande : `grep -n '<le nombre>' CLAUDE.md`.

**Trois faits de conception qui survivront au code :**

1. **`requestFullscreen()` exige une activation utilisateur transitoire, qu'un
   message de canal de données n'est pas.** D'où l'**armement** — le même
   mécanisme que celui déjà validé pour `window.open()`.
2. **Keyboard Lock n'a rien demandé** : `attachFullscreen` verrouille déjà sur
   `fullscreenchange`, **quelle que soit l'origine de l'entrée**. C'est pourquoi
   D8 devait le *mesurer* sans le *coder*.
3. **Le sens est unique, et c'est le mérite du §4.1** : le navigateur ne force
   jamais l'état de la fenêtre Windows. **Aucune oscillation n'est possible.**

**Deux TROUS DU PLAN, trouvés en cours d'exécution et comblés** — tous deux du
même genre, un `match` qu'aucun brief ne nommait :

- **`agent/src/capteur/pont_media.rs` a un bras catch-all `Ok(autre) => return`
  qui TUE le fil `lire_le_media` EN SILENCE.** Sans le bras `PleinEcran`, le
  tout premier message poussé aurait tué la session, **sans panne apparente**.
  **Ce fichier documente ce défaut contre lui-même et il a déjà été payé
  TROIS fois** — D5 (`Sommeil`), D6 (`Part`), D7 (`Audio`). Il est hors
  `#[cfg(windows)]` et porte des tests **précisément** pour que ce bras soit
  couvrable. **Quatrième fois. À vérifier systématiquement pour tout message
  neuf du capteur.**
- **`agent/src/transport/controle.rs` porte un `match` EXHAUSTIF sur
  `AgentControl`** qui ne compile pas sans son bras — celui-là se signale tout
  seul, mais le plan ne le mentionnait pas non plus.

### Ce que D8 n'établit PAS

- **Aucun taux nulle part** : une exécution complète (témoin, ①, ④) et une
  seconde ciblée (②, ⑤, plus corroboration de ① et ④). **Aucune ligne ne porte
  de fréquence de succès.**
- **② n'a rien sollicité, donc les trois inconnues restent ENTIÈRES**, dont le
  risque n°1 — le changement de mode sur une sortie à duplication ouverte.
- **RIEN N'EST DISCULPÉ dans la chaîne qui casse à ②** — ni le client, ni le
  canal de contrôle, ni l'agent. La pièce qui prétendait disculper le transport
  est réfutée par les journaux (C3) : **141 s sans une seule ligne
  `contrôle reçu` pendant la phase ②**, aux deux exécutions.
- **Le changement de mode de sortie n'a JAMAIS tourné en conditions de
  produit**, et il est **désarmé par défaut** depuis la revue finale de branche.
  Son verdict n'est donc pas « il marche » ni « il ne marche pas » : **il n'a
  pas été essayé.**
- **③ n'est pas mesuré**, et sa raison est elle-même mesurée (sonde P2) : Chrome
  `--headless=new` **n'entre pas réellement en plein écran**
  (`document.fullscreenElement` reste `null` 800 ms après un `requestFullscreen()`
  par ailleurs invoqué) et **n'expose pas `navigator.keyboard`**. ⚠️ **Le
  consentement d'installer `Xvfb` + `xdotool` avait été DONNÉ par le
  propriétaire de la machine, et l'installation n'a pas eu lieu** (`which Xvfb` :
  introuvable). Si elle se fait un jour, **les mesures qui en sortiront ne se
  compareront à AUCUNE campagne antérieure** — la conception le dit sans compte,
  et je n'en ajoute pas.
- **`TAILLE_MAX_SORTIE = (1920, 1080)` n'est PAS calibrée**, et **aucun jugement
  visuel n'a été porté** — la lacune exacte que `BPP_MIN` traîne depuis le
  chantier C volet 1, et que D6 traîne sur `FACTEUR_FOCUS` et
  `PART_DORMANTE_BPS`.
- **Le cas HiDPI est structurellement invisible à ce montage** (dpr = 1 partout).
- **La visibilité ET le focus sont IMPOSÉS par le pilote, page par page** —
  limite héritée de D5/D6, toujours la plus lourde du montage : **aucune
  minimisation de vraie fenêtre, aucun focus par clic réel.**
- **Un seul rang (N = 3 fenêtres)** aux deux exécutions ; rien au-delà, rien du
  recouvrement, du déplacement, ni du redimensionnement manuel d'une fenêtre.
- **Le repli `NORESET` → `RESET` de la sonde P1 n'a JAMAIS fait bouger une
  sortie** : `CDS_UPDATEREGISTRY` seul a toujours suffi quand quelque chose
  bougeait. ❌ **CORRIGÉ (I5) : « les combinaisons de repli restent écrites et
  non éprouvées » est FAUX — elles SONT éprouvées**, dans
  `mode-sortie-1728x1080.log`, et l'une d'elles **annonce un succès sur une
  sortie inchangée** (voir le tableau de la section « refus net » ci-dessus).
  Ce qui reste vrai : **aucune n'a jamais fait bouger une sortie.**
- **Le prix du chemin « refus » n'est pas exercé** : le correctif de la tâche 9
  détruit l'ancien encodeur avant de construire le neuf (ordre de D5) et rend le
  bras `Recovered` **fatal** plutôt qu'un faux repli à région périmée — mais
  aucun refus n'ayant eu lieu, **ce chemin n'a jamais couru**.
- **Rien de la latence de bout en bout**, qu'aucun sous-bloc du chantier D n'a
  mesurée à ce jour.
- **Les trois couches inconnues le restent** : le plafond de 8 encodeurs, celui
  de 4 processus, et le mécanisme de l'abandon du mutex DXGI.
- **Le chemin d'extinction propre du superviseur n'a toujours jamais été
  exercé.**
- **Aucun journal séparé par critère** : le pilote est monolithique par
  construction et produit **un** journal combiné par exécution, pas les
  `critere-{1..5}-*.log` que le cahier des charges nommait. **Déclaré, pas
  silencieux.**

### Pièges neufs — à connaître avant de toucher à ce terrain

- ⚠️ **Combiner deux variables `MULTIFENETRE_*` dans le même lancement
  n'enchaîne PAS deux sondes** : l'aiguillage de
  `agent/src/diagnostics/multifenetre.rs` **retourne après la première
  reconnue** (chaque branche fait `return Ok(true)`). Une variable de
  préparation (`MULTIFENETRE_VDD_PURGE`) et une de mesure
  (`MULTIFENETRE_MODE_SORTIE`) doivent être **deux lancements séparés** — et
  rien ne le signale, la seconde est simplement ignorée.
- ⚠️ **Un pilote qui laisse un superviseur vivant bloque SILENCIEUSEMENT la
  tentative suivante — rencontré TROIS fois sur trois.** Le nouveau
  `StreamWriter` ne peut pas ouvrir `agent.log` déjà tenu par l'ancien
  processus, et **la copie relue est celle, périmée, de la tentative
  précédente** : on mesure le run d'avant en croyant lire le sien.
  **`Get-Process agent` doit être revérifié après CHAQUE tentative, y compris
  échouée** — pas seulement avant la toute première. Le `finally` d'un pilote ne
  tue que son Chrome hôte, jamais le superviseur côté VM.
- ⚠️ **Plus de sessions que de fenêtres ouvertes a DEUX causes qui produisent la
  même observation de surface**, et elles ne se distinguent **que** par
  l'horodatage relatif : le fantôme d'une fenêtre qui se multiplie (Paint,
  Bloc-notes — D2, D4) **ou** des fenêtres **préexistantes** non nettoyées. Ici
  c'était la seconde, et la première avait été invoquée à tort. **Ne jamais
  conclure sans comparer la création des sessions en trop à la première
  ouverture du pilote.**
- ⚠️ **Résoudre cible/voisine par rang de nom (`noms[0]`, `noms[1]`) n'est pas
  fiable, MÊME sur une VM nettoyée** — c'est ce qui a fait mesurer ⑤ sur une
  fenêtre dont on ignorait ce qu'elle jouait. Remède employé, réutilisable :
  **une balise d'identité** qui réemploie le mécanisme de ① lui-même (bascule de
  bordure, puis observation de la session qui l'annonce), jouée **avant** toute
  mesure qui en dépend.
- ⚠️ **Le seuil `audio_survit` du pilote ne peut quasiment pas échouer** : il
  compare le niveau à la fréquence assignée au **plancher de bruit** (−158 dB),
  **jamais à la dominante**. Sur le run initial, le niveau à 520 Hz (−115 dB)
  était **79 dB sous** la dominante mesurée (409 Hz, −36 dB) — indiscernable
  d'une fuite spectrale, et le seuil passait quand même. **Le verdict ⑤ ne tient
  PAS grâce à ce seuil** : il a fallu le déplacer sur la dominante. **Ouvert,
  non corrigé.**
- ⚠️ **`window.__pleinEcran` (le tampon côté page de l'instrument) n'est jamais
  vidé ni borné dans le temps** — il persiste pour toute la vie de la page et
  n'est purgé que par dépassement de capacité (50 entrées). Toute bascule
  antérieure, y compris une balise d'identité, y laisse une trace que le pilote
  relit **sans filtrer par horodatage**, produisant un **faux positif de
  « fuite »**. **Ouvert, non corrigé.**
- ⚠️ **`verdict_partie_mesurable` (champ du JSON de ①) vaut `false` aux deux
  exécutions**, là où le document dit ① CONFIRMÉ — il n'a pas été mis à jour
  pour refléter la lecture corrigée de la « fuite ». **Un lecteur qui ouvrirait
  le JSON seul y lirait l'inverse du verdict.** Ouvert.
- ⚠️ **Un argument d'exclusion peut être CIRCULAIRE sans en avoir l'air** : la
  première rédaction écartait l'hypothèse « `PLEIN_ECRAN=0` désarme le chemin »
  au motif qu'aucune trace `redimensionnement ignoré : PLEIN_ECRAN=0`
  n'apparaît — **or le run tourne avec `PLEIN_ECRAN=1`**, cette trace ne pouvait
  structurellement pas être émise, et son absence ne prouve **rien**, sur cette
  hypothèse ni sur aucune autre.
- ⚠️ **Une « latence » lue dans un journal de pilote peut être une PÉRIODE
  D'ÉCHANTILLONNAGE.** 10,2 s pour un réveil de 119 ms : le test attendait la
  prochaine ligne d'un compteur de période 10 s. **Toujours prendre la latence
  du côté qui la subit** (ordre reçu → transition, dans le journal d'agent),
  jamais du côté qui la confirme.

### Ce que D8 lègue — rien n'a été retiré de la pile, et cinq legs s'y ajoutent

**Les cinq legs de D6 et D7 que D8 n'a PAS traités restent dus, en entier :**

1. **Le signal enfant→capteur quand une capture audio meurt** (F3 de D7, hors
   périmètre) : sans lui, la fenêtre voisine n'est **jamais** promue et le groupe
   reste muet. Son point de chute est nommé — `agent/src/windows_audio/`.
2. **L'identité d'une session par génération monotone, pas par son seul nom**
   (F5 de D7, **préexistant**) : une course au `retirer`, qui demande un
   changement de conception du registre.
3. **Les compteurs `TICKS` / `CAPTURED` / `PRODUCED` par session** et leur
   extraction vers `agent/src/windows_source/telemetrie.rs` (D6 n°1) — **c'est
   ce qui rendrait à `windows_source.rs` (638, dette gelée) la marge que D6 lui
   a prise.** ✅ **FAIT par D9 (tâche 11) : 638 → 628, `telemetrie.rs` (72) est
   né, pur et testé sur l'hôte, et la condition d'extraction est LEVÉE.**
4. **Le critère ④ de D6 rejoué à palier de 45 à 60 s**, seule façon de savoir si
   la promotion de focus est systématique.
5. **L'A/B différentiel sur `set_desired_bitrate`** (D6 n°4), **jamais joué** —
   le point ouvert le plus important de D6.

**Et les legs propres à D8 :**

6. ⛔ **Les trois inconnues**, dont la première commande les deux autres
   (« éliminatoire » est un qualificatif de cet index, **absent de la conception
   et du brief** — voir l'annotation du § « Les trois inconnues » plus haut) :
   **le pilote accepte-t-il un changement de mode sur une sortie DONT LA
   DUPLICATION EST OUVERTE ?** Rien ne l'établit à ce jour, et P1 n'ouvre jamais
   de duplication.
   Les deux autres — pertes de mutex infligées aux voisines, conservation du nom
   `\\.\DISPLAYn` — sont sans objet tant que celle-ci n'est pas tranchée.
   ⚠️ **Elles sont désormais le PRÉALABLE À L'ARMEMENT du chemin, pas une suite
   parmi d'autres** : depuis la revue finale de branche, le changement de mode
   de sortie est **désarmé par défaut** (`PLEIN_ECRAN_MODE_SORTIE=1` l'arme), et
   ces trois inconnues sont ce que la recette qui l'armera doit trancher
   d'abord.
   ✅ **D9 les a tranchées, et il n'y aura pas d'armement : le chemin est
   RETIRÉ.** La première est **répondue OUI** (le pilote accepte sur duplication
   ouverte, 2/2), le nom est **conservé** (2/2), et la deuxième reste **non
   mesurée** — le compteur de pertes voisines a **saturé au plafond de
   l'instrument**. Ce qui a décidé n'est aucune des trois : c'est la
   **non-persistance** (`n = 4`) et la **pollution du registre**. Voir la
   section D9.
7. **Le défaut HiDPI, ouvert côté client par décision motivée** : un client à
   `devicePixelRatio > 1` déclencherait un changement de mode à chaque connexion
   de chaque fenêtre. ~~Seule parade actuelle : `PLEIN_ECRAN=0`.~~ ✅ **Il est
   INATTEIGNABLE en configuration livrée depuis le désarmement** — il n'y a plus
   de changement de mode à déclencher. **Le défaut reste ouvert côté client**, et
   ~~redevient mordant le jour où `PLEIN_ECRAN_MODE_SORTIE=1` sera posé~~.
   ✅ **CORRIGÉ par D9 (tâche 5) : l'annonce de viewport passe désormais dans la
   MÊME UNITÉ que le `Resize` (pixels périphériques), et le contrôle a été vu
   ROUGE** (annonce 1280×720 contre `Resize` 2560×1440 sur le client d'avant).
   ⚠️ **Sa conséquence produit avait de toute façon disparu avec le mécanisme,
   un commit plus tôt — la raison écrite dans le code par la tâche 5 était donc
   fausse au moment où elle était écrite**, et la revue transverse de D9 l'a
   corrigée. ⚠️ **Et le correctif en ouvre un autre** : à `dpr = 2`, la sortie
   virtuelle naît quatre fois plus grande, et **rien ne borne cette demande**
   (legs n°5 de D9).
8. **Instrumenter `video.clientWidth`/`clientHeight`**, pour savoir où casse la
   chaîne entre le viewport CDP et l'émission du `Resize` — et donc pouvoir
   enfin exercer ②.
   ⚠️ **Ce legs visait le CLIENT sur la foi d'une pièce RÉFUTÉE (C3)** : « le
   canal de contrôle vit et délivre pendant toute la phase ② » est faux — **141 s
   sans une seule ligne `contrôle reçu`**, aux deux exécutions. **Le transport
   n'est donc disculpé par rien**, et `video.clientWidth` n'est qu'**une
   hypothèse parmi d'autres** — le canal de contrôle en reste une. Instrumenter
   le client garde son intérêt ; **le désigner comme LE maillon fautif n'a plus
   de pièce.**
9. **Trois défauts d'instrument, tous ouverts** : le seuil `audio_survit` qui ne
   peut pas échouer, `verdict_partie_mesurable` qui contredit le verdict publié,
   et `window.__pleinEcran` non borné dans le temps.
10. **Le fait que la recette n'ait relevé que DEUX `Resize` pour CINQ
    sessions**, sans explication versée.
    ❌ **CORRIGÉ (I8) : ce legs disait « 3 sessions sur 5 n'ont jamais émis leur
    `Resize` », et ce « 3 » N'EST PAS DÉRIVABLE** — les deux lignes
    `contrôle reçu Resize` ne portent **aucun champ `session`**, donc rien
    n'établit qu'elles viennent de deux sessions distinctes. Une inférence sous
    hypothèse tacite était **inscrite en dette comme un fait**. **La question
    ouverte est *pourquoi si peu de `Resize`*, pas *lesquelles n'en ont pas
    émis*** — et l'attribution par session exige d'abord que la trace porte sa
    session (legs n°2 de D7).
11. ⛔ **ANNOTER LE §4.1 DU CADRAGE JEUX LUI-MÊME** —
    `docs/superpowers/specs/2026-07-28-support-jeux-design.md`, l. **267-274**
    *(publié `266-273` : le paragraphe commence une ligne plus bas).*
    Il propose toujours ses « trois signaux possibles, à arbitrer à
    l'implémentation », **le premier étant la comparaison `GetWindowRect` /
    rect du moniteur, sans aucune marque**. Une implémentation fidèle à cette
    page annoncerait le plein écran **en permanence, pour toutes les fenêtres**.
    ⚠️ **La réfutation n'existe pour l'instant QUE dans ce fichier-ci** : qui
    ouvre le cadrage sans passer par `CLAUDE.md` y lit encore le critère comme
    valide. **Décision de périmètre assumée** — un index durable ne se commite
    pas avec une spec —, **et donc dette, pas disparition.**
    ✅ **FAIT le 5 août 2026, commit `2856f2a`** — le §4.1 porte désormais son
    encadré : prémisse conservée, conclusion sur le premier signal réfutée,
    `SHQueryUserNotificationState` écarté, et le signal survivant nommé.
    ⚠️ *Cette ligne a d'abord été écrite au futur (« fait au commit suivant »)
    dans un commit où le geste n'avait pas encore eu lieu — une affirmation
    au-delà de son relevé, corrigée ici en nommant le hachage.*
12. ⛔ **C1 — la pollution du registre par le produit lui-même**, et
    **l'alternative des cinq GUID qui en rend la portée inconnue** (voir
    l'encadré « Ce qui est réellement livré » plus haut). **Premier travail de
    la recette qui posera `PLEIN_ECRAN_MODE_SORTIE=1`**, avec le legs n°13.
    Inatteignable tant que le chemin est désarmé — donc **dette, pas défaut
    actif**.
    ⛔ **CE LEG CESSE D'EXISTER COMME DETTE DE CODE (D9)** : le produit n'écrit
    plus jamais au registre. 🔴 **MAIS LA POLLUTION DÉJÀ ÉCRITE MORD, et D9 l'a
    observée** : elle bloque le produit à **TROIS fenêtres** sur cette VM, aux
    six exécutions de la recette ③ sans exception (20/16/12 `ERROR` du type
    `sortie créée mais introuvable dans la topologie DXGI`). **Rien ne nettoie
    derrière**, et **la portée reste inconnue**. C'est le legs n°4 de D9.
13. ⛔ **C2 — la reprise sur perte d'accès de D2 court-circuitée** par une `Err`
    sur un échec **transitoire** de réouverture après changement de mode. Même
    statut que le n°12 : inatteignable par défaut, **à traiter avant tout
    armement**.
    ⛔ **CE LEG CESSE D'EXISTER (D9)** : `reconstruire_sur_la_sortie` n'avait
    qu'un seul appelant — le changement de mode — et **il est parti avec lui**.
    Contrairement au n°12, **rien ne survit** : il n'y a pas de trace laissée
    dans l'environnement.

> ✅ **CE QUE D9 A FAIT DE CETTE LISTE (6 août 2026), point par point.** Les
> n°6, 7, 12 et 13 supposaient tous un chemin qui n'existe plus : **D9 a mesuré
> le changement de mode de sortie et l'a RETIRÉ**, il ne l'a pas armé.
>
> | Leg | Sort sous D9 |
> | --- | --- |
> | 1 — signal enfant→capteur (capture audio morte) | **FERMÉ SUR PIÈCES côté code, NON EXERCÉ sur la VM** — et la branche « réélection » du remède est **INERTE** |
> | 2 — identité par génération monotone | **FERMÉ SUR PIÈCES** (tests d'hôte) **sur le registre de sommeil SEUL** ; le jumeau de `serveur.rs` reste |
> | 3 — `TICKS`/`CAPTURED`/`PRODUCED` par session | **FERMÉ SUR PIÈCES** — 638 → 628, `telemetrie.rs` né |
> | 4 — critère ④ à palier long | **FERMÉ SUR PIÈCES** — 3/4, 2 exécutions |
> | 5 — A/B `set_desired_bitrate` | **JOUÉ, N'ÉTABLIT RIEN** — +23,2 % entre bras contre +83,1 % de variance intra-bras. **Reste dû** |
> | 6 — les trois inconnues | **DEVENUES SANS OBJET** : la première est TRANCHÉE (le pilote accepte, duplication ouverte), et le mécanisme est retiré |
> | 7 — HiDPI | **CORRIGÉ sur l'unité, et sa CONSÉQUENCE a disparu avec le mécanisme** |
> | 8 — chaîne viewport → `Resize` | **FERMÉ SUR PIÈCES côté code** ; le rejeu différé **NON EXERCÉ** sur la VM |
> | 9 — trois défauts d'instrument | **FERMÉ SUR PIÈCES** |
> | 10 — pourquoi si peu de `Resize` | **RENDU DÉCIDABLE, et la réponse contredit le rapport de la tâche qui l'a mesuré** |
> | 11 — annoter le §4.1 du cadrage jeux | déjà fait par D8 (`2856f2a`) |
> | 12 — C1, pollution du registre | ⛔ **CESSE D'EXISTER comme dette de code** — le produit n'écrit plus au registre. ⚠️ **MAIS la pollution DÉJÀ ÉCRITE bloque le produit à TROIS fenêtres sur cette VM, et rien ne nettoie derrière** |
> | 13 — C2, reprise court-circuitée | ⛔ **CESSE D'EXISTER** — `reconstruire_sur_la_sortie` n'avait qu'un appelant, et il est parti |
>
> **Détail, réserves et pièces : section « Sous-bloc D9 » ci-dessous.**

---

## 🧾 Sous-bloc D9 — solder la dette, et ce qu'on retire au lieu de le réparer (6 août 2026)

Résultats complets :
`docs/superpowers/plans/2026-08-06-multifenetres-solder-la-dette-resultats.md`.
Conception : `docs/superpowers/specs/2026-08-06-multifenetres-solder-la-dette-design.md`.
Journaux : `docs/superpowers/plans/journaux-multifenetres-d9/` — **118 fichiers
suivis par git**, et **TROIS familles de lecture** :

| Famille | État | Ce qu'il faut faire |
| --- | --- | --- |
| tous les `*-plat.log` | UTF-8, **ANSI déjà retirées**, CRLF | rien |
| les journaux d'agent bruts (`agent-*.log`, `p-*.log`, `p2-*.log`) | UTF-8, CRLF, **séquences ANSI PRÉSENTES** | `sed 's/\x1b\[[0-9;]*m//g'` — ou lire le `-plat` jumeau, versé pour chacun |
| les journaux de **pilote** (`critere-*.log` **sans** préfixe `agent-`) | classés « data » — **20 octets `\x02`/`\x03`** par fichier, résidu du PowerShell de `run-agent.sh` | `grep -a`. ⚠️ **Les accents sont INTACTS** et se `grep`ent normalement, comme en D8 et contrairement à D6 |

D9 devait fermer les **douze** legs ouverts au sortir de D8. Il en a fermé une
partie sur pièces, en a fait **disparaître deux**, en a laissé plusieurs **non
exercés faute de pouvoir les provoquer**, et il en a **découvert de nouveaux**.
Le tableau leg par leg vit à la fin de la section D8, juste au-dessus.

### ⛔ Le fait n°1 : le changement de mode de sortie est RETIRÉ, pas armé

**C'est l'inverse de ce que le sous-bloc allait chercher.** D8 avait livré ce
mécanisme désarmé, faute d'avoir jamais pu l'exercer, et D9 s'ouvrait sur une
phase de mesure pour décider s'il fallait l'armer. La mesure a tranché contre :

- ✅ **L'inconnue « éliminatoire » de D8 est TRANCHÉE, et dans le sens
  favorable** : le pilote SudoVDA **accepte** un changement de mode sur une
  sortie **dont la duplication DXGI est ouverte et tenue** — mouvement relu par
  DXGI, **2 exécutions sur 2**. Le nom `\\.\DISPLAYn` est **conservé**, 2/2.
- ❌ **Mais le changement NE SURVIT PAS.** La sortie revient à sa taille de
  création **dès qu'une sortie virtuelle de plus est créée** — c'est-à-dire à
  **chaque ouverture de fenêtre**, donc en marche nominale du produit.
  **`n = 4` exécutions propres**, à cibles toutes distinctes de la taille de
  création (R6 1920×1080, R7 2560×1440, R8 et R9 1600×900), **sous
  `CDS_UPDATEREGISTRY` comme sous `flags = 0`**.
- ❌ **Et `CDS_UPDATEREGISTRY` POLLUE le registre** — **confirmé et attribuable
  par GUID**, sur **3 transitions probantes** : les GUID jamais visés par
  l'API ne sont jamais pollués, sur 9 exécutions.

**Décision du propriétaire du dépôt : retirer le code.** La forme retenue n'est
pas un second désarmement mais une **suppression** —
`windows_source/redimensionnement/mode_sortie.rs` (458 lignes) disparaît,
`PLEIN_ECRAN_MODE_SORTIE` disparaît, `reconstruire_sur_la_sortie` disparaît avec
son unique appelant. **Les legs 12 (C1) et 13 (C2) CESSENT D'EXISTER au lieu
d'être différés** — ils décrivaient un chemin qui n'est plus là.

⚠️ **Le mécanisme de la non-persistance N'EST PAS EXPLIQUÉ.** Il est séparé en
deux régimes observables, et un confondeur covarie exactement avec leur
frontière (la duplication de la sortie sous test est ouverte aux créations de
voisines, fermée à la création du témoin). **Séparé, pas expliqué.**

⚠️ **Ce qui reste livré du plein écran, et qui n'a pas bougé** : la **détection**
par le style de fenêtre (`capteur/fenetre.rs`, `PERIODE_STYLE = 250 ms`) et
l'**annonce** `PleinEcran` → `Fullscreen` → armement client. Le sens « la
résolution suit » n'existe plus. Le constat de mesure qui le justifie vit **en
tête de `agent/src/capteur/plein_ecran.rs`**, et **cinq commentaires du dépôt y
renvoient** : c'est le point de référence à ne pas déplacer.

### 🔴 Le fait n°2 : la pollution de registre BLOQUE le produit à TROIS fenêtres

**C'est la Critique C1 de D8 — celle que le désarmement devait rendre
inatteignable — observée EN TRAIN DE MORDRE, en conditions de produit.**

`superviseur/boucle.rs` exige une correspondance exacte avec la taille demandée
(1280×720, à `placement::TOLERANCE_PX = 4` près) et **rend la sortie au pilote**
sinon. Or, sur cette VM, les sorties **naissent à 3840×2160** — la dernière
taille laissée au registre par une mesure antérieure. Résultat, aux **six**
exécutions de la recette ③ **sans exception** : **trois** sessions établies, et
toutes les tentatives au-delà de la troisième échouent sur
`sortie créée mais introuvable dans la topologie DXGI … apparues=["\\.\DISPLAY8 3840x2160"]`
— **20** `ERROR` sur `focus-1`, **16** sur `focus-2`, **12** sur `ab-desarme-2`,
**toutes du même type**.

⚠️ **« Inatteignable » vaut pour le PRODUIT, qui n'écrit plus au registre. Cela
ne vaut pas pour ce qui y a DÉJÀ été écrit, et rien ne nettoie derrière.** Le
remède opérationnel reste la sonde elle-même :
`MULTIFENETRE_MODE_SORTIE=1280x720`, **dans un lancement à elle seule**.

⚠️ **Portée toujours INCONNUE** : l'alternative « le mode registre est par GUID »
contre « une seule écriture empoisonne toutes les sorties futures » n'est
**toujours pas tranchée** — D9 a établi l'attribution par GUID de la
**pollution**, pas la portée du **blocage**.

⚠️ **Conséquence de méthode, à ne pas perdre** : **toutes les mesures de
capacité de D9 portent sur TROIS fenêtres**, pas huit ni dix. Elles ne se
comparent à aucune campagne de D4 à D6 sur un seul chiffre absolu.

### 🔴 Le fait n°3 : la réélection après répit est INERTE

Le leg 1 (le signal enfant→capteur quand une capture audio meurt) est livré en
trois étages — la règle pure (`capteur/audio.rs`, champ `inapte`), le registre
(`capteur/sommeil.rs` : `REPIT_REARMEMENT_AUDIO = 5 s`, `REARMEMENTS_MAX = 5`),
et le message `VersCapteur::AudioMort` de bout en bout. **La branche PROMOTION
est valide** : une voisine du même groupe de PID a sa propre capture, sur un fil
qui n'a jamais échoué, et l'élire lui donne réellement le son.

❌ **La branche RÉÉLECTION APRÈS RÉPIT, elle, ne restaure RIEN — et c'est le cas
MAJORITAIRE (une application, une fenêtre, aucune voisine).** Vérifié sur le
code : `set_audio_source` n'est appelée **qu'une fois**
(`demarrage/audio.rs`, sans boucle) ; le fil de capture (`windows_audio.rs`)
exécute un `return` **définitif** une fois `capture_morte` posé ; et réélire la
même session ne fait que pousser `Audio { actif: true }` → `set_actif(true)`,
**qui n'écrit qu'un booléen atomique que ce fil mort ne relira jamais**. Rien,
nulle part, ne reconstruit la source.

**Décision : corriger l'affirmation, léguer le remède.** Le point de chute est
nommé — `demarrage/audio.rs` construit, `transport/piste_audio.rs` porte,
`windows_audio.rs` tient le fil.

⚠️ **Corollaire trouvé par la revue transverse** : `REARMEMENTS_MAX` **ne peut
pas mordre dans ce même cas majoritaire**. `sommeil/porteurs.rs` remet le
compteur à zéro dès qu'une session est **décidée** porteuse ; pour une fenêtre
seule de son groupe de PID, la sortie de répit la rend automatiquement porteuse.
Le garde-fou ne s'applique donc en pratique qu'aux groupes à **plusieurs**
fenêtres. **Non corrigé, délibérément** : le remède juste est de refermer le
cycle sur une **preuve** de son, pas sur une décision — et cette preuve viendra
avec le legs ci-dessus.

### 🔵 Le fait le plus réutilisable : le *process loopback* suit l'ARBRE DE PROCESSUS

**La capture *process loopback* n'est liée ni au service audio, ni au
périphérique de rendu.** Toute disruption au niveau service ou endpoint est donc
**structurellement le mauvais levier** — ce qui explique d'un seul coup les
**quatre** échecs de déclenchement de la recette ② : `Restart-Service Audiosrv
-Force` (celui du brief), `Stop-Service` + attente + `Start-Service`,
`Stop-Process audiodg -Force`, et `Disable-PnpDevice`/`Enable-PnpDevice` ne
produisent **aucune** ligne `lecture audio échouée` sur **9 exécutions versées**.

**Corroboration** : `compteurs audio actif=true` et `paquets_recus` passant de
194 à 6156 établissent que `read()` **était bien appelée** — le zéro n'est pas
l'artefact d'une fenêtre qui ne lit jamais.

**Cinquième déclencheur nommé et JAMAIS ESSAYÉ** : tuer le `chrome.exe` **CIBLE**
du process loopback.

### Les quatre passes, avec le nombre d'exécutions dans chaque énoncé

**Aucun taux n'est revendiqué nulle part.**

| | Objet | Verdict | Exéc. |
| --- | --- | --- | --- |
| **P** | l'éliminatoire × 4 combinaisons, témoin sans duplication | **REÇU, et il tranche CONTRE l'armement** | 2 (éliminatoire) + 9 (persistance) |
| **① a** | rejeu du `Resize` différé | **NON EXERCÉ** — le réseau local est trop rapide pour provoquer la course ; forcer par latence a cassé la reconnexion | 2 vertes + 1 tentative |
| **① b** | chaque `contrôle reçu Resize` porte son `session` | **TENU** | 2 + 1 rouge |
| **① c** | même unité annonce/`Resize` à `deviceScaleFactor = 2` | **TENU sur vert, RÉFUTÉ sur rouge** (annonce 1280×720 / `Resize` 2560×1440) | 2 + 1 rouge |
| **① d** | détection plein écran **exclusive** | **TENU** — 3 bascules, 3 sessions distinctes, jamais de fuite | 2 |
| **② A** | réarmement | **NON DÉMONTRABLE, et INSATISFIABLE PAR CONSTRUCTION** | 2 |
| **② B** | promotion d'une voisine | **NON DÉMONTRABLE** — aucun `AudioMort` jamais signalé, donc aucune promotion à provoquer | 2 |
| **② rouge** | le contrôle sans le remède | **NON DISCRIMINANT** — le même silence se reproduirait sur un binaire au remède parfait | 1 + 3 validations |
| **② nr** | non-régression du rattachement (D4) | **TENU** — capteur relancé en **0,05 s**, canal rattaché **0,25 s** après, **0** clôture, **0** enfant terminé | 1 |
| **③ focus** | critère ④ de D6 à palier **60 s** | **3 promotions sur 4 déplacements** | 2 |
| **③ A/B** | `set_desired_bitrate` armé / désarmé | **N'ÉTABLIT RIEN** | 4 (2 par bras) |

⚠️ **Le critère ① c porte une réserve de méthode qui n'a pas été corrigée** :
« seul le client diffère » entre rouge et verts est **FAUX** — le rouge a tourné
avec une géométrie **double** (1280×720 CSS contre 640×360). Le verdict survit
(le critère est intra-run), **l'A/B n'est pas propre**, et la ligne de commande
du rouge n'est versée nulle part.

⚠️ **Le critère ② A était insatisfiable INDÉPENDAMMENT du problème de
déclencheur** : même avec une disruption qui aurait fait échouer
`capture.read()`, la réélection ne reconstruit rien (fait n°3). **Deux causes
d'échec, pas une** — et seule la seconde était connue avant la revue.

### L'A/B de `set_desired_bitrate` : joué, et il n'établit rien

Le leg n°4 de D6 — « le point ouvert le plus important » — a enfin son bras
désarmé (`PART_SONDAGE=0`). **Il ne tranche pas** :

| Bras | Exéc. | Mb/s cumulés | `packetsLost` |
| --- | --- | --- | --- |
| ARMÉ | 2 | 11,439 et 6,247 — moyenne **8,843** | 0 |
| DÉSARMÉ | 2 | 7,546 et 6,813 — moyenne **7,180** | 0 |

**Écart entre bras : +23,2 %, dans le sens attendu. Variance INTRA-bras :
+83,1 %** (11,439 contre 6,247 sur le même bras armé). **Le bruit dépasse le
signal** : rien n'autorise à imputer la différence de moyenne à `PART_SONDAGE`
plutôt qu'à la variabilité de l'hôte, déjà nommée en D6 comme facteur dominant.
**Le leg reste dû**, et il faudra plus d'exécutions, pas un autre montage.

⚠️ **Trois fenêtres mesurées aux quatre exécutions**, pas huit — le plafond du
fait n°2. La base de comparaison est identique aux quatre, ce qui rend l'A/B
légitime **sur un effectif de 3**.

### ⚠️ Le leg 10 est DÉCIDABLE, et les journaux le tranchent — contre le rapport qui l'a mesuré

D8 demandait « pourquoi si peu de `Resize` ? » et laissait la question
indécidable, faute de champ `session` sur la trace. **La tâche 5 de D9 a posé ce
champ ; la tâche 14 l'a mesuré ; et son rapport conclut en sens inverse de ses
propres journaux.**

Inventaire réel, `agent-critere-1-1.log` : `w-2` = 3 `Visibility` / **0
`Resize`** ; `w-3` = 3/3 ; **`w-5` = 1 `Visibility` / 0 `Resize`** ; `w-7` =
13/1. **Deux sessions sur quatre, canal de contrôle DÉMONTRÉ VIVANT, zéro
`Resize`.**

⚠️ **Le canal vivant ÉCARTE l'hypothèse « canal mort » pour ces deux sessions ;
il ne désigne PAS le client.** C'est exactement la nuance que la correction C3
de D8 avait payée. Le maillon fautif reste **non identifié**.

⚠️ **Le rapport de la tâche 14 déclare ce leg clos en sens inverse, et il n'a
pas été corrigé** (interruption assumée de la ronde de correction). **Le
contredire est le premier travail de qui le relira.**

### ⚠️ La revue transverse de fin de branche — six défauts, tous franchissant une frontière de tâche

Elle a trouvé **cinq** défauts en D7 et **trois** Critiques en D8. Elle en trouve
**six** ici, et **tous ont la même forme** : corrects des deux côtés pris
séparément.

| # | Défaut | Sort |
| --- | --- | --- |
| **1** | `capteur/fenetre/commandes.rs` affirmait encore que « `resize` change désormais le mode de la sortie virtuelle » et annonçait comme conséquence assumée qu'un plein écran demandé pendant le sommeil serait **perdu**. La tâche 3 avait retiré le mécanisme ; la tâche 9 a édité ce fichier **cinquante lignes plus haut** sans le voir | **CORRIGÉ** |
| **2** | `client/src/main.ts` justifiait le correctif HiDPI par « sinon chaque connexion déclencherait un changement de mode » — **une conséquence déjà supprimée un commit plus tôt**. Le correctif est juste (symétrie d'unité) ; **la raison écrite dans le code était fausse dans la branche même qui la livre** | **CORRIGÉ** — et la vraie raison est écrite |
| **3** | `capteur/audio.rs` affirmait encore « le réarmement après répit est le seul remède ». Réfuté par la tâche 15, corrigé dans `sommeil.rs`, **pas ici** — le naufrage du « 487 », **sixième occurrence** | **CORRIGÉ** |
| **4** | La taille de création d'une sortie **n'est bornée par personne**, et D9 la fait **doubler** sur HiDPI (viewport en pixels périphériques) pendant que `borner_a_la_taille_max` (1920×1080) **perd son dernier appelant** dans le même sous-bloc. À `dpr = 2`, une fenêtre 1280×720 CSS demande **quatre fois** les pixels | **DOCUMENTÉ, non corrigé — legs** |
| **5** | Le leg 2 est fermé sur le registre de **sommeil** seul. `capteur/serveur.rs::oublier` porte **la même course F5** sur le registre d'attentes, `remove` inconditionnel — le brief de la tâche 10 ne nommait que `sommeil` | **DOCUMENTÉ, non corrigé — legs** |
| **6** | `REARMEMENTS_MAX` ne mord pas dans le cas majoritaire (voir le fait n°3) | **DOCUMENTÉ, non corrigé — legs** |

**Les trois premiers sont des affirmations de code devenues fausses dans leur
propre branche.** Aucune revue par tâche ne pouvait les voir : la tâche qui
écrit la phrase et celle qui la réfute ne se relisent jamais l'une l'autre.

### Ce que le code livre

| Étage | Fichier | Nature |
| --- | --- | --- |
| la règle d'inaptitude | `agent/src/capteur/audio.rs` (**228**) | **pur, aucun `cfg`** — le champ `inapte`, 4 tests neufs (12 en tout) |
| le registre | `agent/src/capteur/sommeil.rs` (**269**) + `sommeil/registre.rs` (**331**) | `REPIT_REARMEMENT_AUDIO`, `REARMEMENTS_MAX`, la génération monotone. **Extrait sur exigence de revue** après une compression que ce dépôt interdit |
| le message | `capteur/protocole.rs` (**381**) — `VersCapteur::AudioMort` | poussé, non répondu par `Fait` ; **hors du bras catch-all de `pont_media.rs`**, vérifié |
| la détection locale | `agent/src/transport/tick.rs` (**343**), branche **a1sexies** | verrou `audio_mort_signale`, remis à zéro par `VideoSource::rattachement_survenu` |
| la télémétrie | `agent/src/windows_source/telemetrie.rs` (**72**) | **pur, aucun `cfg`**, par session ; lue par `capteur/fenetre/trace.rs` (**43**) sous le span `session` de D7 |
| le rejeu du `Resize` | `client/src/resize.ts` (**45**) | **pur, sans DOM**, 5 tests |
| la sonde P | `diagnostics/multifenetre/mode_sortie.rs` (**383**) + 5 enfants | duplication tenue, témoin, 4ᵉ combinaison, épreuve de persistance |

**Vérifications de fin de branche** : `cargo test -p agent` → **440 passed, 0
failed** (422 en début de branche, **+18**) ; `cargo check --target
x86_64-pc-windows-gnu` → **sortie 0, 11 avertissements**, tous `dead_code`, dont
**deux délibérés et justifiés dans le code** (`TAILLE_MAX_SORTIE` et
`borner_a_la_taille_max`, conservés sans appelant) ; `npx vitest run` côté client
→ **107 passed**.

### Ce que D9 n'établit PAS

- **Aucun taux, nulle part.** Deux exécutions par critère au mieux, une pour
  plusieurs.
- **Le MÉCANISME de la non-persistance du changement de mode** : séparé en deux
  régimes, **pas expliqué**, et un confondeur covarie avec leur frontière.
- **La PORTÉE du blocage par pollution de registre** : par GUID, ou global ?
  Non tranchée. Et **rien dans le produit ne nettoie le registre**.
- **Le leg 1 n'a JAMAIS été exercé de bout en bout sur la VM** : aucun
  `AudioMort`, aucun `réarmement programmé`, aucun `abandon définitif` n'apparaît
  dans un seul journal. Le remède est raisonné, compilé et couvert par des tests
  d'hôte — **pas mesuré**.
- **Le rejeu du `Resize` différé (leg 8) n'a pas été observé en conditions
  réelles**, seulement par ses tests unitaires.
- **La course du leg 2 n'a pas été provoquée** — c'était écrit d'avance (§11 de
  la spec) : le correctif se prouve par tests d'hôte, la VM n'établit que la
  non-régression du rattachement.
- **Le maillon fautif du leg 10 reste non identifié** : le canal est disculpé
  pour deux sessions, **rien n'est désigné**.
- **`REPIT_REARMEMENT_AUDIO` et `REARMEMENTS_MAX` ne sont pas calibrées** —
  elles rejoignent `BPP_MIN`, `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`,
  `HYSTERESIS`, `REPIT_APRES_ECHEC` et `TAILLE_MAX_SORTIE`. **Aucun jugement
  visuel ni d'écoute n'a été porté sur aucune constante.**
- **Rien au-delà de TROIS fenêtres**, à cause du fait n°2 — et donc rien qui se
  compare aux campagnes de D4 à D6.
- **Le critère ③ focus a tourné à `BUDGET_BPS = 8 000 000`, pas à la valeur
  livrée (12 M)** : rejouer à 12 M exige d'abord de lever le plafond à trois
  sessions.
- **La latence de bout en bout**, qu'aucun sous-bloc du chantier D n'a jamais
  mesurée.
- **Les trois couches inconnues le restent** : le plafond de 8 encodeurs, celui
  de 4 processus, et le mécanisme de l'abandon du mutex DXGI.
- **Le chemin d'extinction propre du superviseur n'a toujours jamais été
  exercé**, depuis D1.
- **Aucun client réel, aucun HiDPI réel** : le montage reste un Chrome sans
  interface, à décodage logiciel, sur l'hôte qui porte la VM ; `deviceScaleFactor
  = 2` n'a été exercé que sur la **symétrie d'unité**, jamais sur le **coût**.
- **La visibilité et le focus restent IMPOSÉS par le pilote de recette**, page
  par page — limite héritée de D5, la plus lourde du montage, qu'aucun sous-bloc
  n'a levée.

### Pièges neufs — à connaître avant de toucher à ce terrain

- ⚠️ **Une variable de disruption peut viser la mauvaise couche entière, et cela
  se lit comme une panne du produit.** Quatre déclencheurs audio, neuf
  exécutions, zéro erreur de lecture : ce n'était pas le remède qui ne marchait
  pas, c'était le levier qui n'était pas relié. **Vérifier À QUOI un mécanisme
  est lié avant de choisir comment le casser** — ici, l'arbre de processus, pas
  le service ni l'endpoint.
- ⚠️ **Un critère peut être insatisfiable pour DEUX raisons, et trouver la
  première fait manquer la seconde.** Le montage A de la recette ② n'aurait pas
  pu réussir même avec un déclencheur correct. **Chercher la seconde cause après
  avoir trouvé la première.**
- ⚠️ **Un compteur de fenêtres côté PILOTE compte des popups, pas des
  sessions.** Le rapport de la tâche 16 a annoncé **8 puis 6** fenêtres ; il y en
  avait **3**, aux six exécutions. La source de vérité est
  `enfant lancé`/`fenêtre attachée au capteur` dans `agent.log`, jamais le
  navigateur. C'est le piège maison « compter les fenêtres, jamais les
  lancements » sous une troisième forme.
- ⚠️ **Une promotion peut arriver 0,83 s APRÈS la fin de la mesure** — c'est
  littéralement le phénomène que le palier de 60 s existait pour éliminer, et il
  s'est reproduit. **Chercher activement l'événement juste après la fenêtre**
  avant de compter un échec.
- ⚠️ **Une sonde qui demande à une sortie la taille qu'elle a déjà ne peut pas
  échouer.** Rejoué en D9 (le confondeur « cibler 1280×720 », trouvé par
  l'implémenteur lui-même, run conservé et **étiqueté**). Le remède est celui de
  D8 : **exclure structurellement la taille courante des cibles**, et juger sur
  le **mouvement**, jamais sur une égalité.
- ⚠️ **`survit=true` peut être rendu par une sortie qui a DISPARU** (sentinelle
  `(0,0) == (0,0)`), et `mouvement_observe=true` par un tour vide. **Un verdict
  positif doit exiger que la chose mesurée existe encore.**
- ⚠️ **Un plafond de fenêtres peut venir d'un état laissé par une mesure
  antérieure, pas du produit.** Trois sessions au lieu de huit, aux six
  exécutions : ce n'était ni de l'instabilité VM ni une limite du système, c'est
  du registre pollué. **`grep 'sortie créée mais introuvable'` avant de conclure
  à un plafond.**
- ⚠️ **Extraire pour rester sous 500 lignes, ce n'est pas COMPRESSER.**
  `sommeil.rs` a franchi 500, a été ramené à **499 en resserrant des
  commentaires** — geste que `CLAUDE.md` interdit nommément — et la revue a
  exigé l'extraction, qui l'a ramené à **269**. Même scénario sur
  `capteur/fenetre.rs` (508 → 496 par compression → **485** par extraction).
  **Deux fois dans le même sous-bloc.**
- ⚠️ **Une revue peut être conforme à la LETTRE d'un brief et manquer son
  OBJET.** La première version du leg 2 frappait la génération **au lancement du
  processus**, quand la course est à l'**attache** : `retirer_est_perime` ne
  pouvait structurellement pas rendre `true` en production. **Le défaut était
  dans la conception, pas dans l'exécution.**

### Ce que D9 lègue — douze points, dont trois neufs (plus deux ajoutés par la vague de correction)

**Repris de D8 et de D6, toujours dus :**

1. ⛔ **Reconstruire la capture audio après sa mort** — c'est le remède réel du
   leg 1, dont D9 n'a livré que la moitié qui marche (la promotion d'une
   voisine). **Le cas majoritaire — une application, une fenêtre — reste sans
   remède.** Point de chute nommé : `demarrage/audio.rs` construit,
   `transport/piste_audio.rs` porte, `windows_audio.rs` tient le fil.
2. ⛔ **Le leg 2 sur le SECOND registre** : `capteur/serveur.rs::oublier` porte
   la même course F5, `remove` inconditionnel. Même patron de remède que
   `sommeil/registre.rs`.
3. ⛔ **L'A/B sur `set_desired_bitrate` (D6 n°4)** — **joué, n'établit rien** :
   le bruit intra-bras (+83,1 %) dépasse le signal (+23,2 %). Il faut **plus
   d'exécutions**, pas un autre montage — et de préférence à plus de trois
   fenêtres, donc après le legs n°4.

**Neufs, propres à D9 :**

4. 🔴 **Nettoyer la pollution de registre, ou s'en rendre immunisé.** Elle bloque
   le produit à **trois** fenêtres sur cette VM, aujourd'hui, et **rien ne
   nettoie derrière**. Deux voies, non arbitrées : purger le registre au
   démarrage du superviseur, ou **tolérer** une sortie née à la mauvaise taille
   plutôt que la rendre au pilote (`superviseur/boucle.rs`). ⚠️ **Sa portée
   reste inconnue** — par GUID, ou global ?
5. 🔴 **Borner la taille de sortie demandée** : le viewport passe désormais en
   pixels périphériques, donc ×4 les pixels à `dpr = 2`, et
   `borner_a_la_taille_max` (1920×1080) n'a plus d'appelant. **Aucun client
   HiDPI réel n'a été mesuré.** ⚠️ **Deux conséquences mordent, et ni l'une ni
   l'autre n'est nommée ci-dessus** :
   - **le plafond de 8 encodeurs concurrents n'a JAMAIS été mesuré qu'à 720p**
     (1280×720/60, toutes les campagnes du 31 juillet au 3 août 2026) — NVENC
     borne en macroblocs par seconde, pas en nombre de sessions : huit fenêtres
     HiDPI en 1440p ou plus (le ×4 ci-dessus) peuvent très bien être refusées
     là où huit fenêtres 720p passaient ;
   - et depuis le remède du sous-bloc D5, `set_encode_size` **détruit l'ancien
     encodeur avant de construire le neuf** — si la construction du neuf
     échoue, la source n'a plus d'encodeur du tout. Ce chemin d'échec est
     déclaré **« jamais couru »** dans ce fichier (sections D4/D5) : un HiDPI
     qui ferait franchir le plafond de 8 (jamais mesuré au-delà de 720p) serait
     la première charge réelle à l'emprunter.
6. ⛔ **`REARMEMENTS_MAX` ne mord pas dans le cas majoritaire** — le compteur est
   remis à zéro sur une **décision** d'arbitrage, pas sur une preuve de son.
   Se referme avec le legs n°1, pas avant.
7. ⛔ **Le maillon fautif du leg 10 reste non identifié.** Deux sessions sur
   quatre n'émettent aucun `Resize` **avec un canal démontré vivant** ; le canal
   est disculpé pour elles, **rien n'est désigné**. ⚠️ **Et le rapport de la
   tâche 14 conclut l'inverse et n'a pas été corrigé.**
8. ⛔ **Le cinquième déclencheur d'une mort de capture audio, jamais essayé** :
   tuer le `chrome.exe` **cible** du process loopback. Sans lui, le leg 1 restera
   non exercé sur la VM.
9. ⛔ **`agent/src/survie_verdict.rs` est posé à la RACINE du crate** alors que le
   dépôt a deux précédents (`capture_reprise`, `windows_source_sortie`) qui
   gardent le fichier chez le parent et n'y hissent que la déclaration par
   `#[path]`. Un seul appelant. ⚠️ **« Déviation non justifiée » est INEXACT** :
   l'en-tête du fichier la justifie, en citant un AUTRE précédent
   (`geometry.rs`, `sortie_dxgi.rs`, tous deux posés directement à la racine,
   sans `#[path]`). ❌ **Et « la branche D9 a TRIPLÉ la convention `#[path]` »,
   écrit ici par la vague de correction finale, est FAUX à son tour** — relevé
   par `git log -S` à la re-revue de cette même vague : `windows_source_sortie`
   date de **D1** (`0529651`) et `capture_reprise` de **D2** (`9438e33`), tous
   deux fusionnés sur `main` AVANT que cette branche ne diverge. **D9 en ajoute
   UN SEUL** — `windows_source_telemetrie` (tâche 11). C'est 2 → 3, pas un
   triplement. *Corriger une affirmation fausse peut en produire une autre : ce
   fichier l'écrit depuis D6, et la vague qui corrigeait le leg l'a repayé.*
   Ce qui reste vrai, c'est que la branche a ajouté un troisième emploi de la
   convention par ailleurs
   (`windows_source_telemetrie`, tâche 11, s'ajoutant à `capture_reprise` et
   `windows_source_sortie`) sans réconcilier les deux conventions ni choisir
   entre elles pour ce fichier-ci.
10. ⛔ **Neuf constats de revue PARQUÉS sur la tâche 14** (recette ①), dont
    l'A/B rouge/vert non propre, la pièce du critère (a) qui ne couvre qu'une
    page sur treize, et un « défaut d'instrument » qui est en réalité un
    **comportement du produit** (le shell réémet `fenetre-ouverte` pour une
    fenêtre déjà ouverte, mécanisme non élucidé). **Ils sont toujours dans le
    rapport versé.**

**DEUX de plus, trouvés par la vague de correction unique de la revue finale
de D9 (6 août 2026), et délibérément NON corrigés — legs, pas défauts actifs.**
*(Ce sous-titre annonçait « trois » pour deux entrées numérotées, quand
l'en-tête du même commit en comptait deux — corrigé à la re-revue. Le n°11
porte deux tests, ce qui explique probablement le glissement : on compte ici
des LEGS, pas des problèmes individuels.)*

11. ⛔ **Deux tests faibles, sans être morts, incapables de rendre l'autre
    valeur** :
    - `agent/src/windows_source/telemetrie.rs:68`,
      `une_telemetrie_neuve_est_a_zero` : n'éprouve que `#[derive(Default)]`,
      jamais la logique propre de `Telemetrie` (`tick`/`capturee`/`produite`),
      déjà couverte par ailleurs par `deux_telemetries_ne_se_melangent_pas`.
    - `client/src/resize.test.ts:18`, dont le titre annonce « REJOUE la
      dernière taille quand le canal était fermé au moment du geste » :
      `RejeuResize` (`client/src/resize.ts`) n'a AUCUNE notion de canal ni de
      `readyState` — le test se contente d'omettre l'appel à `confirmer()`, ce
      qui rend le même verdict pour n'importe quelle autre raison de
      non-confirmation. Il annonce un état de canal qu'il n'exerce pas.
12. ⛔ **Un invariant non écrit, `client/src/main.ts:345`** : le rejeu du
    `Resize` (`session.controlChannel.addEventListener('open',
    emettreSiPossible)`) tient parce que le `.then()` qui le pose (ligne 204)
    s'exécute intégralement de façon SYNCHRONE, sans `await` intercalé entre
    la construction de `rejeu`/du `ResizeObserver` et cet `addEventListener`.
    Un `await` glissé là romprait le rejeu EN SILENCE si le canal s'ouvrait
    pendant l'attente. Ni écrit dans un commentaire du code (au-delà de la
    ligne 345 elle-même, qui ne dit que le QUOI, pas le POURQUOI de l'ordre),
    ni testé.

---

## 🚀 Commandes de Développement Essentielles

### Build & Run

```bash
# Démarrage complet
docker-compose up -d

# Rebuild après changement Dockerfile/package.json
docker-compose up -d --build

# Redémarrer (recompile les assets)
docker-compose restart web

# Arrêter
docker-compose down
```

### Logs & Debugging

```bash
# Suivre les logs en temps réel
docker-compose logs -f web

# Logs guacd seulement
docker-compose logs web | grep guacd

# Logs JavaScript client (ouvrir DevTools navigateur)
# F12 → Console

# Vérifier la liste des apps découvertes
curl http://localhost:3445/apps | jq

# Vérifier une app spécifique
curl http://localhost:3445/apps | jq '.[] | select(.short=="firefox")'
```

### Filesystem & Permissions

```bash
# Vérifier le mount /media/vm
ls -lah /media/vm/

# Vérifier les .lnk shortcuts
ls -lah /media/vm/Users/guacamole/Desktop/*.lnk

# Permissions FUSE (après lancement session)
ls -lah /mnt/ftp-*/

# Démontage manuel FUSE si blocké
sudo fusermount -u /mnt/ftp-<UUID>
```

### Tests WinRM

```bash
# Depuis le container
docker exec -it guacamole-web-1 node test_winrm_nodejs.js

# Tester PowerShell command
docker exec -it guacamole-web-1 node -e "
const winrm = require('nodejs-winrm');
winrm.runCommand('Get-ChildItem C:\\', '192.168.3.2', 'Administrator', 'PASSWORD', 5985, true)
  .then(console.log);
"
```

---

## 📋 Checklist de Déploiement Production

### Sécurité

- [ ] Externaliser tous les credentials (Docker secrets, AWS Secrets Manager)
- [ ] Activer HTTPS/WSS pour tous les WebSockets
- [ ] Ajouter authentification utilisateur
- [ ] Rate limiting sur les endpoints
- [ ] CSP headers correctement configurés
- [ ] Audit logging des connexions RDP
- [ ] Rotation régulière des mots de passe
- [ ] Firewall: whitelist IPs autorisées
- [ ] Disable debug mode in Browserify
- [ ] Review et remove console.log statements

### Performance

- [ ] Activer compression gzip/brotli
- [ ] Mettre en place CDN pour assets statiques
- [ ] Caching HTTP headers appropriés
- [ ] Optimiser taille images (WebP pour icônes?)
- [ ] Minifier JavaScript (uglify, terser)
- [ ] Lazy loading pour liste apps si >50 apps
- [ ] Connection pooling pour WinRM
- [ ] Redis pour session storage (scale horizontal)

### Monitoring

- [ ] Prometheus metrics (connexions actives, latence RDP)
- [ ] Alerting sur crash guacd
- [ ] Logs centralisés (ELK, CloudWatch)
- [ ] Healthcheck endpoint (`/health`)
- [ ] Uptime monitoring
- [ ] Dashboard sessions actives
- [ ] Tracking erreurs client (Sentry)

### Infrastructure

- [ ] Multi-instance guacd avec load balancing
- [ ] Auto-scaling basé sur nombre de sessions
- [ ] Backup régulier de la configuration
- [ ] Disaster recovery plan
- [ ] Documentation ops (runbooks)
- [ ] CI/CD pipeline
- [ ] Environnements staging/prod séparés

---

## 🎯 Roadmap et TODO

### Priorité P0 (Critique - À faire immédiatement)

1. ✅ ~~Fixer sessions qui se ferment après 2 secondes~~ (RÉSOLU)
2. ⬜ **Résoudre crash "double free or corruption"**
   - Investiguer avec guacd en mode DEBUG
   - Améliorer shutdown propre du filesystem FUSE
   - Considérer alternatives (WebDAV, SFTP)
3. ⬜ **Externaliser credentials de production**
   - Créer .env.example
   - Docker secrets pour prod
   - Documentation mise à jour

### Priorité P1 (Haute - Cette semaine)

4. ⬜ **Fix recompilation automatique**
   - Ajouter gulp watch task
   - Ou migrer vers Webpack avec HMR
   - Update README avec workflow dev
5. ⬜ **Améliorer qualité icônes**
   - Implémenter extraction haute-res PowerShell
   - Ou pre-générer côté Windows
   - Fallback gracieux si échec
6. ✅ ~~Améliorer design page d'accueil~~ (RÉSOLU)

### Priorité P2 (Moyenne - Ce mois)

7. ⬜ **Gestion erreurs filesystem**
   - Indicateur visuel si WebSocket fail
   - Message informatif utilisateur
   - Retry logic avec backoff
8. ⬜ **Logging structuré**
   - Winston ou Pino
   - Niveaux: ERROR, WARN, INFO, DEBUG
   - Rotation logs (max size)
9. ⬜ **Détection PWA installées**
   - Écouter événement `appinstalled`
   - Vérifier `display-mode: standalone`
   - Sync avec localStorage
10. ⬜ **Authentification basique**
    - Username/password simple
    - Sessions avec express-session
    - Redirection /login si non auth

### Priorité P3 (Basse - Nice to have)

11. ⬜ Tests unitaires et intégration
12. ⬜ Documentation API complète
13. ⬜ Mode développement avec hot reload
14. ⬜ Support multi-utilisateurs simultanés
15. ⬜ Dashboard admin (stats, sessions actives)
16. ⬜ Enregistrement sessions (recording)
17. ⬜ Clipboard avancé (images, fichiers)
18. ⬜ Print to PDF depuis RemoteApp
19. ⬜ Raccourcis clavier personnalisables
20. ⬜ Thèmes clairs/sombres

---

## 📚 Ressources et Références

### Documentation Officielle

- [Apache Guacamole](https://guacamole.apache.org/doc/gug/)
- [guacamole-lite](https://www.npmjs.com/package/guacamole-lite)
- [MS-SHLLINK Specification](https://docs.microsoft.com/en-us/openspecs/windows_protocols/ms-shllink/)
- [File System Access API](https://developer.mozilla.org/en-US/docs/Web/API/File_System_Access_API)
- [PWA Documentation](https://web.dev/progressive-web-apps/)

### Librairies Utilisées

- [Sharp](https://sharp.pixelplumbing.com/) - Image processing
- [FUSE Native](https://github.com/fuse-friends/fuse-native) - Filesystem
- [WinRM Node.js](https://github.com/mshock/nodejs-winrm) - PowerShell remote
- [ColorThief](https://lokeshdhakar.com/projects/color-thief/) - Color extraction

### Outils de Debug

- Chrome DevTools Protocol pour debugging RDP canvas
- Wireshark pour analyser trafic RDP (port 3389)
- `fusermount -u` pour démontage FUSE propre
- `docker exec -it <container> bash` pour debug interne

---

**Dernière mise à jour**: ~~21 octobre 2025 (Session de bugfixing complète)~~ —
⚠️ **cette ligne dormait depuis huit sous-blocs et se réfutait elle-même** : le
fichier a été écrit tout du long jusqu'au **6 août 2026** (sous-bloc D9). Elle ne
date que le pied de page hérité du Guacamole historique, ci-dessous, qu'aucun
chantier du projet agent n'a touché.

**Contributeurs**:

- Développement initial: [Original dev name]
- Documentation et bugfixes: Session Claude (21 Oct 2025)

**Version du projet**: 1.1 (post-bugfixes)

---

> 💡 **Rappel Important**: Toute nouvelle découverte, bug résolu, configuration importante, ou décision architecturale DOIT être ajoutée à ce fichier pour référence future.
