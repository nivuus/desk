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
`agent/src/`, `client/src/`, `plateforme/`, `proto/`, `src/`, `web/`, `scripts/`.

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
| `agent/src/windows_source.rs` | ~~638~~ ~~628~~ **630** (7 août 2026, D10) | `#[cfg(windows)]`, aucun test |

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
> | `agent/src/capteur/sommeil/registre.rs` | ~~331~~ **346** | neuf — le registre lui-même, transposition vérifiée caractère pour caractère |
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
> | `agent/src/survie_verdict.rs` | ~~61~~ **74** (7 août 2026, D10) | neuf, **pur** — ~~⚠️ posé à la RACINE du crate alors que le dépôt a deux précédents (`capture_reprise`, `windows_source_sortie`) qui gardent le fichier chez le parent et n'y hissent que la déclaration par `#[path]`. Déviation relevée, non corrigée~~ ✅ **TRANCHÉ (7 août 2026, tâche 17, D10) : ce n'était PAS une déviation.** La convention retenue (§« Convention de module enfant… », tête de ce fichier) range un module par son NOM : `capture_reprise`/`windows_source_sortie` portent le préfixe de leur parent et se déclarent par `#[path]` ; `survie_verdict`, comme `geometry` et `sortie_dxgi`, n'en porte aucun et vit à la racine nue — où il était déjà. **Aucun fichier n'a bougé.** |
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

> ✅ **Relance du 7 août 2026, fin du sous-bloc D10, PAR LA COMMANDE, APRÈS les
> dernières éditions de la ronde** — y compris celles de la revue transverse,
> qui sont entièrement des commentaires et qui font bouger huit fichiers. **Le
> tableau de dette a toujours DEUX lignes, et l'une d'elles a REGROSSI de
> deux** : `encode.rs` **1536** (inchangé), `windows_source.rs` ~~628~~ **630**.
> **Aucun autre fichier de code source ne dépasse 500 lignes.**
>
> ⚠️ **Ces +2 sont une addition à de la dette GELÉE, et ils sont déclarés
> plutôt que dissimulés — précédent : les +7 de D6 sur ce même fichier.**
> L'addition est **100 % commentaire**, et c'est la réfutation d'une
> affirmation devenue fausse (« plus rien à recadrer », que la famille ① de
> D10 réfute). Elle n'ajoute **rien** à la surface non testée que la règle des
> 500 lignes existe pour contenir. **Aucune extraction ne l'accompagne**, et
> ce n'est pas une omission : ce fichier a payé une fois pour comprendre que
> **raccourcir une réfutation pour atteindre un compte de lignes échangerait
> une vérité contre un nombre**. La règle inchangée demeure : toute addition
> **substantielle** exige une extraction, et son point de chute reste
> `agent/src/windows_source/`.
>
> ⚠️ **Le `628` figure à HUIT endroits de ce fichier, et SEUL CELUI DU TABLEAU
> DE DETTE a été corrigé — délibérément.** Les sept autres sont des énoncés
> **datés** (« relevé le 6 août 2026 », « D9 fait retomber 638 à 628 ») qui
> restent **vrais comme histoire** ; les barrer les rendrait faux. Les places
> ont été **énumérées avant d'écrire** (`grep -n '628' CLAUDE.md`), parce que
> « corrigé à sa place » est une affirmation de **complétude** et que le
> naufrage du « 487 » s'est rejoué six fois dans ce dépôt, dont une dans la
> vague même qui le corrigeait ailleurs. **Le seul chiffre auquel se fier pour
> décider si ce fichier peut grossir est celui du tableau de dette.**
>
> ⚠️ **MARGE LA PLUS SERRÉE D'`agent/src` APRÈS `encode/arret.rs` (500, marge
> 0) : `agent/src/superviseur/table.rs` est à 492, marge 8** — **ex æquo avec
> `agent/src/capture.rs`, également à 492**, et **la plus serrée du DÉPÔT est
> ailleurs** : `client/verify-webrtc.mjs`, marge 3 (voir la fin de ce relevé).
> *(Une première rédaction déclarait ici « du dépôt », et déclarait la même
> chose de `verify-webrtc.mjs` soixante-six lignes plus bas : deux superlatifs
> qui se contredisaient, dont l'un était faux quelle que soit la portée
> retenue.)* Il a fait l'ascenseur
> pendant la branche — 494 → 497 (tâche 6) → **504** (tâche 9, plafond
> FRANCHI) → 499 par réduction de doc neuve, marge 1 → **469** par
> l'**extraction** exigée en revue (`enum Effet` → `superviseur/table/effets.rs`,
> 52 lignes, verbatim, aucun site d'appel touché) → 492 après les corrections
> de la revue transverse. **La leçon que ce dépôt paie pour la quatrième fois
> est la même : la marge regagnée par une extraction se reperd à la ronde
> suivante si on la traite comme acquise** — 31 rendus, 23 repris dans le même
> sous-bloc.
>
> ⚠️ **Le plafond a été FRANCHI TROIS FOIS pendant cette branche, et rattrapé
> trois fois par une EXTRACTION, jamais par une compression** :
> `capteur/fenetre.rs` à **505** → `fenetre/ouverture.rs` (114) → **426** ;
> `superviseur/table.rs` à **504** → `table/effets.rs` (52) → 469 ;
> `transport.rs` à **501** → `transport/initialisation.rs` (80) → **457**.
> C'est le premier sous-bloc où aucune compression n'est employée pour
> repasser sous la ligne — D9 en avait joué deux.
>
> **Fichiers que D10 a fait bouger, tous mesurés par la commande :**
>
> | Fichier | Lignes | Remarque |
> | --- | --- | --- |
> | `agent/src/superviseur/table.rs` | ~~494~~ **492** (marge **8**) | voir l'ascenseur ci-dessus |
> | `agent/src/demarrage.rs` | ~~464~~ ~~481~~ **491** (marge **9**, 19 août 2026, chantier E) | +17 sous D10. ⚠️ **+10 sous le chantier E** (le branchement du puits de micro, `demarrage::micro`), relevé **491** par la commande à la clôture de E |
> | `agent/src/transport.rs` | ~~457~~ **468** | franchi 501, puis extrait ; +11 à la vague de correction de la revue finale |
> | `agent/src/superviseur/placement.rs` | **441** | `sortie_assez_grande`, `taille_retenue`, et leurs tests |
> | `agent/src/capteur/fenetre.rs` | ~~485~~ **426** (marge 74) | franchi 505, puis extrait |
> | `agent/src/capteur/protocole.rs` | ~~381~~ **419** | `AudioVivant`, et la correction d'`AudioMort` |
> | `agent/src/capteur/distante.rs` | ~~400~~ **409** | |
> | `agent/src/transport/piste_audio.rs` | ~~229~~ **403** | `reconstruire_ou_signaler` et sa documentation |
> | `client/src/main.ts` | ~~352~~ **392** | l'instrumentation du `Resize` (legs 7 et 10 de D9) |
> | `agent/src/transport/tick/tests/audio.rs` | ~~392~~ **403** | neuf (tâche 2), grossi par les tests de la famille ②, puis +11 à la vague de correction |
> | `agent/src/source.rs` | ~~387~~ **393** | |
> | `agent/src/windows_audio/fil.rs` | **339** | neuf (tâche 3) — porte `AUDIO_FAUTE_LECTURE` et le budget global |
> | `agent/src/capteur/sommeil/registre.rs` | ~~331~~ **346** | ⚠️ **ce nombre est resté FAUX au commit de la revue transverse**, corrigé au tour suivant — voir l'encadré sous ce tableau |
> | `agent/src/windows_source/sortie.rs` | ~~313~~ **351** | `sur_sortie` reçoit la taille RETENUE |
> | `agent/src/windows_audio.rs` | ~~479~~ **283** | 479 → 252 par extraction, puis +31 de corrections |
> | `agent/src/superviseur/boucle/creation_sortie.rs` | ~~282~~ **295** | neuf (tâche 1) ; +13 à la vague de correction (les TROIS causes du refus) |
> | `agent/src/capteur/audio.rs` | ~~228~~ **271** | |
> | `agent/src/superviseur/boucle.rs` | ~~492~~ **263** | 492 → 262 par extraction, puis +1 |
> | `agent/src/audio.rs` | ~~273~~ **318** | `RECONSTRUCTIONS_MAX`, `REPIT_RECONSTRUCTION`, `Reconstructeur` |
> | `agent/src/capteur/sommeil.rs` | ~~269~~ **325** | |
> | `agent/src/transport/tick/tests.rs` | ~~489~~ **148** | 489 → 115 par extraction (tâche 2), puis +33 |
> | `agent/src/capteur/serveur/attentes.rs` | **142** | neuf — la génération monotone, sous le même verrou que la carte |
> | `agent/src/superviseur/table/attribution.rs` | **119** | |
> | `agent/src/capteur/fenetre/ouverture.rs` | **114** | neuf — extraction du plafond franchi |
> | `agent/src/transport/initialisation.rs` | **80** | neuf — idem |
> | `agent/src/capteur/serveur/attentes/tests.rs` | **64** | neuf |
> | `agent/src/superviseur/table/effets.rs` | **52** | neuf — idem |
> | `agent/src/transport/tick.rs` | ~~343~~ **398** | la branche a1sexies, puis +55 à la vague de correction |
> | `agent/src/superviseur/boucle/placement_periodique.rs` | ~~76~~ **103** | |
> | `agent/src/survie_verdict.rs` | ~~61~~ **74** | ⚠️ **ABSENT de la première rédaction de ce tableau**, dont l'en-tête dit « tous mesurés par la commande » — voir l'encadré ci-dessous |
>
> ✅ **Chiffres voisins RELEVÉS et EXACTS ce jour-là**, à ne pas re-vérifier :
> `encode/arret.rs` **500** (marge 0), `capture.rs` **492** (8),
> `transport/socket.rs` **481** (19), `transport/piste_video.rs` **477** (23),
> `capteur/distante/tests.rs` **474** (26), `congestion/controleur.rs` **472**
> (28), `transport/adaptation.rs` **468** (32), `proto/src/control.rs` **419**,
> `capteur/sommeil/tests.rs` **416**, `wasapi/process_loopback.rs` **402**,
> `capteur/serveur.rs` **400**, `capteur/sommeil/parts.rs` **349**,
> `wasapi.rs` **352**, `superviseur/table/tests_retention.rs` **377**,
> `capteur/vivier.rs` **275**, `capteur/tube.rs` **273**,
> `capteur/pont_media.rs` **263**, `capteur/repartiteur.rs` **147**,
> `demarrage/audio.rs` **95**, `windows_source/telemetrie.rs` **72**.
>
> ⚠️ **`client/verify-webrtc.mjs` est à 497 lignes, marge 3** — c'est la marge
> la plus serrée du dépôt après `encode/arret.rs`, et **aucun tableau ne la
> signalait**. Intouché par D10 : il dérivait déjà.
>
> ⚠️ **Corollaire, non vérifié et signalé comme tel** : ce fichier n'ayant
> **jamais** été mesuré avant aujourd'hui, les quatre déclarations « la marge
> la plus serrée du dépôt après `arret.rs` » portées par les relevés D7, D8 et
> D9 sur `superviseur/table.rs` (l. 317, 374, 377, 471) **étaient peut-être
> déjà fausses à leur date**. Elles sont datées, donc conservées telles
> quelles ; **rien ne permet de les confirmer ni de les réfuter
> rétrospectivement**, et les rejouer demanderait de remonter chaque commit.
>
> ⚠️ **DIVERGENCE DE CONVENTION à trancher, signalée et NON tranchée** : le
> § « Portée » en tête de ce fichier liste `client/src/`, quand la commande de
> vérification, elle, ne filtre que `node_modules|package-lock|…` et attrape
> donc **`client/verify-webrtc.mjs`**, hors de `client/src/`. Ce fichier est-il
> dans la portée de la règle des 500 lignes ? **La commande dit oui, le texte
> dit non**, et l'écart n'a jamais été relevé. C'est une décision de
> convention, pas un constat : elle appartient au propriétaire du dépôt.
>
> ❌ **CE RELEVÉ A LUI-MÊME PORTÉ UN NOMBRE FAUX, et c'est le naufrage du
> « 487 » commis À L'INTÉRIEUR de la ronde qui le dénonce — septième
> occurrence.** `capteur/sommeil/registre.rs` était publié **331** dans le
> tableau ci-dessus, dont l'en-tête dit « **tous mesurés par la commande** » ;
> il vaut **346**. Le nombre **avait bien été mesuré**, et la correction est
> partie **au mauvais endroit** : un `replace(…, 1)` a barré le 331 de la table
> **D9** (que personne ne relira pour connaître une marge) en laissant intact
> celui de la table **D10**, la seule que le sous-bloc suivant lira. **Six des
> sept chiffres re-mesurés étaient justes ; celui-là est resté faux d'un tour
> entier**, dans le commit même dont le message annonçait avoir énuméré les
> places avant d'écrire.
>
> **La leçon n'est donc PAS « mesurer », qui avait été fait — c'est que
> `grep -n` doit être relu place par place APRÈS l'édition, pas seulement
> lancé avant.** Une substitution qui ne dit pas combien d'occurrences elle a
> touchées est une affirmation de complétude non vérifiée.
>
> ❌ **NEUVIÈME OCCURRENCE, trouvée par la revue finale de branche, et c'est le
> jumeau exact de la précédente : `agent/src/survie_verdict.rs` était publié
> **61** ; il vaut **74**.** Et c'est **cette branche** qui l'a fait grandir
> (`3c8c305`, +13) — **dans le commit même qui éditait cette ligne de tableau**
> pour y porter l'annotation « ✅ TRANCHÉ ». Le fichier était en outre **absent**
> du tableau D10 ci-dessus, dont l'en-tête dit « tous mesurés par la commande » :
> l'en-tête était donc faux d'une ligne. Les deux sont corrigés.
>
> **Ce que ces deux occurrences ajoutent à la doctrine, et qui ne s'y trouvait
> pas** : *éditer une ligne de tableau ne fait pas relire le nombre qu'elle
> porte.* Dans les deux cas, la main qui écrivait était **sur la ligne même**
> qui contenait le chiffre faux — et le regard portait sur l'annotation, pas sur
> le nombre. **Toucher une ligne d'un tableau de comptes oblige à remesurer son
> compte**, même quand ce n'est pas l'objet de l'édition.

> ✅ **Relance du 19 août 2026, fin du sous-bloc D11, PAR LA COMMANDE, APRÈS les
> dernières éditions de la ronde** (revue transverse comprise). **Le tableau de
> dette a toujours DEUX lignes, et les DEUX sont INCHANGÉES depuis D10** :
> `encode.rs` **1536**, `windows_source.rs` **630**. D11 n'a touché ni l'un ni
> l'autre. **Aucun autre fichier de code source ne dépasse 500 lignes.**
>
> ⚠️ **DEUX PORTES ARMÉES PAR LE PLAN SE SONT DÉCLENCHÉES, et les deux
> extractions ont été jouées — jamais une compression** :
> `agent/src/transport/piste_audio/injection.rs` (**71**) et
> `agent/src/transport/tick/tests/audio/injection.rs` (**141**), toutes deux
> déclarées par un `mod` ordinaire **à l'intérieur** de leur parent (aucune
> frontière `#[cfg(windows)]` ici : la convention `#[path]` ci-dessous **ne
> s'applique pas**). C'est le deuxième sous-bloc consécutif où aucune
> compression n'est employée.
>
> ⚠️ **La marge la plus serrée du dépôt reste `agent/src/encode/arret.rs` à
> 500 (marge 0)**, et la deuxième **`client/verify-webrtc.mjs` à 497 (marge
> 3) — valeur du dépôt COMMITÉ** ⚠️ *(l'arbre de travail en portait 488 au
> moment de ce relevé, une modification NON COMMITÉE du sous-projet ⑤ qui
> travaillait sur `client/` en concurrence ; `git show HEAD:` rend bien 497)* —
> **inchangée depuis D10, donc elle dérive toujours** : ce fichier
> n'est touché par aucun chantier, et la **divergence de convention** que D10
> a relevée sans la trancher tient toujours (le § « Portée » ne liste que
> `client/src/`, la commande l'attrape quand même). **Décision de convention,
> non tranchée, et elle appartient au propriétaire du dépôt.**
>
> ❌ **CE PARAGRAPHE A ÉTÉ RATTRAPÉ PAR LES FAITS EN MOINS D'UNE JOURNÉE, sur
> ses TROIS clauses** (relevé par la commande le 19 août 2026, clôture du
> sous-bloc P2, `HEAD = 85ed23a`) :
> - **le 488 non commité est commité** — c'était la tâche 17 de P2, qui tenait
>   la porte des 500 par une **extraction** ; la revue transverse de P2 y a
>   ensuite ajouté six lignes de commentaire, et le fichier vaut **494**
>   (marge **6**) ;
> - **« ce fichier n'est touché par aucun chantier » est donc FAUX** : il a été
>   touché deux fois le jour même de ce relevé, par le chantier qui travaillait
>   dans le même arbre ;
> - **et il n'est plus la DEUXIÈME marge la plus serrée** : c'est
>   `agent/src/micro/tests.rs`, **497 (marge 3)**, fichier **neuf du chantier E
>   (microphone)**, commité pendant la clôture de P2.
>   ❌ **CE 497 A TENU MOINS LONGTEMPS ENCORE : il vaut 271** (relevé par la
>   commande à la clôture du chantier E, le même jour). La tâche du plafond de
>   dissimulation devait y ajouter des tests, et a joué l'extraction **dans le
>   commit de l'addition** — `agent/src/micro/tests_lecteur.rs` (**423**) —, sans
>   que le plafond soit franchi. **La marge de 3 n'existe donc plus, et la
>   deuxième marge la plus serrée est `agent/src/transport.rs` à 495 (marge 5)**,
>   créée par la revue transverse de ce même chantier ; `client/verify-webrtc.mjs`
>   (494, marge 6) n'est que la troisième.
>   ⚠️ *QUATRE relevés successifs du même jour — D11, P2, S1, E — ont chacun
>   nommé une « deuxième marge la plus serrée » différente, et chacun avait
>   raison à son heure. Le superlatif n'est pas un fait durable : c'est un
>   instantané, et il vieillit en heures quand plusieurs chantiers partagent
>   l'arbre.*
>   ⚠️ *Cette annotation-ci a elle-même porté **270** puis **« la deuxième est
>   verify-webrtc »** — deux énoncés que la dernière édition de la ronde a rendus
>   faux, celle qui ajoutait une ligne à `micro/tests.rs` et quatre à
>   `transport.rs`. **Ils ont été attrapés en relisant l'annotation APRÈS le
>   relevé final, pas en l'écrivant.** C'est littéralement la règle « relever les
>   tailles APRÈS la dernière édition », prise en défaut par son propre auteur au
>   sein de la ronde qui l'applique.*
>
> **La seule clause qui survit est la divergence de convention**, toujours non
> tranchée. ⚠️ *Ce n'est pas une erreur de D11 : son relevé était juste à sa
> date, et il le dit. C'est la démonstration de ce que son propre piège
> annonce — « un compte n'est attribuable qu'assorti de son heure quand deux
> chantiers partagent l'arbre » — appliquée au relevé qui l'énonce.*

>
> **Fichiers que D11 a fait bouger, tous mesurés par la commande :**
>
> | Fichier | Lignes | Remarque |
> | --- | --- | --- |
> | `agent/src/transport/tick/tests/audio.rs` | ~~403~~ **471** (marge 29) | +68 : les tests du leg 4 et de l'injection. ⚠️ **Porte franchie à 480 en cours de tâche 4** → extraction de `tests/audio/injection.rs` |
> | `agent/src/transport/piste_audio.rs` | ~~403~~ **471** (marge 29) | l'accesseur, `{erreur:#}`, le réarmement. ⚠️ **Même porte, même remède** : `piste_audio/injection.rs` |
> | `agent/src/transport.rs` | ~~468~~ ~~469~~ **491** (marge **9**, 19 août 2026, chantier E) | ⚠️ **+1, et il est DÉCLARÉ** : 100 % commentaire, la correction n°3 de la revue transverse. La porte du plan était à 480, non franchie. ⚠️ **Le 469 a été RATTRAPÉ le jour même par le chantier E** : +22 (`mic_mid`, les quatre champs de `piste_micro`, `set_puits_micro`), relevé **491** par la commande à la clôture de E — **marge 9**, l'une des deux plus serrées d'`agent/src` après `encode/arret.rs` |
> | `client/src/main.ts` | ~~392~~ **408** | +16 : l'invariant du rejeu (leg D9 n°12) et sa grille de lecture |
> | `agent/src/audio.rs` | ~~318~~ **391** | +73 : `injection_encore_armee`, **pur**, et ses tests d'hôte |
> | `agent/src/windows_audio/fil.rs` | ~~339~~ **382** | +43 : `AUDIO_FAUTE_LECTURE_MS`, un **seul** `warn!` enrichi |
> | `agent/src/transport/tick/tests/audio/injection.rs` | **141** | neuf — extraction de la porte |
> | `agent/src/demarrage/audio.rs` | ~~95~~ **115** | +20 : `set_audio_porteuse(true)` dans la seule branche `None`, et sa raison |
> | `agent/src/windows_source/telemetrie.rs` | ~~72~~ **93** | +21 : le test faible D9 n°11, resserré au triplet exact |
> | `agent/src/transport/piste_audio/injection.rs` | **71** | neuf — extraction de la porte |
> | `client/src/resize.test.ts` | ~~39~~ **47** | +8 : le second test faible D9 n°11 |
> | `scripts/run-agent.sh` | ~~124~~ **126** | +2 : les deux variables de banc neuves (hors portée de la règle) |
> | `agent/src/windows_audio.rs` | ~~283~~ **292** | revue transverse, commentaires seuls |
> | `agent/src/capteur/sommeil.rs` | ~~325~~ **328** | idem |
> | `agent/src/transport/tick.rs` | ~~398~~ **403** | idem |
> | `agent/src/superviseur/protocole.rs` | ~~101~~ **110** | idem — le chemin `signaling/` disparu |
>
> ⚠️ **CINQ chiffres de ces deux tables avaient été écrits SANS être mesurés,
> et les CINQ étaient faux** — quatre dans la table « Ce que le code livre » de
> la section D11 (`piste_audio.rs` 472 pour **471**, `fil.rs` 392 pour **382**,
> `telemetrie.rs` 97 pour **93**, `resize.test.ts` 39 pour **47**) et un dans la
> table ci-dessus (`windows_audio.rs` 283 pour **292** — le chiffre d'AVANT la
> branche, recopié comme s'il était celui d'après). Ils ont été attrapés
> **avant le commit**, en relançant `wc -l` sur les deux tables entières plutôt
> qu'en les relisant. **C'est le naufrage du « 487 » pris à sa source** : la
> défense n'est pas de mieux se souvenir, c'est de **mesurer chaque ligne d'une
> table de comptes au moment où on l'écrit**.
>
> ✅ **Chiffres voisins RELEVÉS et EXACTS ce jour-là**, à ne pas re-vérifier :
> `encode/arret.rs` **500** (marge 0), `client/verify-webrtc.mjs` **497** (3),
> `superviseur/table.rs` **492** (8), `capture.rs` **492** (8),
> `transport/socket.rs` **481** (19), `demarrage.rs` **481** (19),
> `transport/piste_video.rs` **477** (23), `capteur/distante/tests.rs` **474**
> (26), `congestion/controleur.rs` **472** (28), `transport/adaptation.rs`
> **468** (32), `diagnostics/multifenetre/reprise/passes.rs` **465**,
> `moniteurs_virtuels/pilote.rs` **463**, `geometry.rs` **459**,
> `diagnostics/multifenetre/montee.rs` **459**, `diagnostics/capture.rs`
> **457**, `turn/allocation.rs` **456**, `moniteurs_virtuels.rs` **448**,
> `superviseur/placement.rs` **441**.
>
> ⚠️ **« À NE PAS RE-VÉRIFIER » EST DEVENU FAUX POUR DEUX DE CES DIX-HUIT
> LIGNES, et c'est une instruction ACTIVE — pas un énoncé daté qu'on pourrait
> laisser dormir.** Les dix-huit ont été **remesurées par la commande** à la
> clôture du **chantier E** (19 août 2026), précisément parce que toucher une
> ligne d'un tableau de comptes oblige à remesurer le tableau :
> `client/verify-webrtc.mjs` vaut **494** (marge **6**, corrigé par la clôture
> de S1 quelques heures plus tard — voir la section S1), et
> `agent/src/demarrage.rs` vaut **491** (marge **9**, +10 par le chantier E).
> **Les seize autres sont EXACTES et n'ont pas bougé.**
>
> **Ce que cela ajoute à la doctrine** : une liste « à ne pas re-vérifier »
> n'est sûre que tant qu'aucun chantier ne touche ses fichiers, et **deux
> chantiers ont touché celle-ci le jour même de sa rédaction**. La formule
> reste utile — seize lignes sur dix-huit ont bien tenu — mais elle doit se
> lire « relevé exact à cette date », jamais « dispensé de mesure ».

> ✅ **Relance du 19 août 2026, clôture du CHANTIER E (microphone, bloc E1),
> PAR LA COMMANDE, APRÈS la dernière édition de la ronde** — corrections de la
> revue transverse comprises, sans quoi la table serait fausse à la fin de la
> ronde qui l'écrit. **Le tableau de dette a toujours DEUX lignes, et les DEUX
> sont INCHANGÉES** : `encode.rs` **1536**, `windows_source.rs` **630**. Le
> chantier E n'a touché ni l'un ni l'autre. **Aucun autre fichier de code source
> ne dépasse 500 lignes.**
>
> ⚠️ **LA DEUXIÈME MARGE LA PLUS SERRÉE DU DÉPÔT EST NEUVE, et c'est la REVUE
> TRANSVERSE elle-même qui l'a créée : `agent/src/transport.rs` est à 495,
> marge 5** (469 avant la branche → **491** par le chantier E → **495** par les
> corrections n°5 et n°6 de la revue transverse, **100 % commentaire**). C'est
> **déclaré, pas subi** : les deux corrections redressaient un inventaire de
> modules qui comptait « les deux pistes média » quand il y en a trois, et un
> champ `audio_mid` documenté au singulier alors que **c'est précisément le champ
> dont le mauvais renseignement était le défaut MUET** que la tâche 7 a corrigé.
> **La seconde a été RESSERRÉE d'une ligne en retirant une redondance** — le
> récit du défaut vit déjà en entier dans `evenements.rs` — et **non en
> raccourcissant la réfutation**, geste que ce fichier interdit nommément.
> **Toute addition future à `transport.rs` appelle une EXTRACTION**, jamais une
> compression ; ce fichier a déjà franchi 501 en D10 et a été rattrapé par
> `transport/initialisation.rs`.
>
> ⚠️ **DEUX marges neuves à 9, toutes deux du chantier E** :
> `agent/src/demarrage.rs` **491** (+10, le branchement du puits de micro) et,
> ci-dessus, `transport.rs`. **Trois chiffres publiés plus haut dans ce fichier
> avaient dérivé et sont corrigés À LEUR PLACE** — `transport.rs` 469 → 495,
> `demarrage.rs` 481 → 491, `micro/tests.rs` 497 → **271** (extraction vers
> `micro/tests_lecteur.rs`, jouée **dans le commit de l'addition**, sans que le
> plafond soit franchi). Les
> places de chacun ont été **énumérées par `grep -n` AVANT d'écrire, et relues
> après**.
>
> ⚠️ **`proto/src/control.rs` vaut 470** (419 avant la branche, +51 par le champ
> `mic` de `Ready`). **Ce nombre porte une réserve de CONCURRENCE et non de
> mesure** : un sous-bloc **P3** travaillait dans le même arbre au moment de ce
> relevé. `git log` ne montre aucune modification de ce fichier par P3 à cette
> heure, **mais rien ne garantit qu'il n'en fera pas** — c'est exactement la
> situation que le relevé de D11 a payée sur `verify-webrtc.mjs`. **Le
> remesurer avant de s'y fier.**
>
> ✅ **RÉSERVE LEVÉE PAR LA MESURE, à la clôture de P3 (19 août 2026) :
> `proto/src/control.rs` vaut TOUJOURS 470** (`wc -l`, après le dernier commit
> de P3). P3 n'y a pas touché, et **c'était une décision et non un hasard** —
> sa divergence E14 relève ces 470 lignes et en tire que le canal `/agent`
> doit vivre dans un fichier NEUF (`proto/src/plateforme.rs`) plutôt que
> d'être une variante de plus d'`AgentControl`. **La marge de 30 est
> intacte.**
>
> **Fichiers que le chantier E a fait bouger, tous mesurés par la commande :**
>
> | Fichier | Lignes | Remarque |
> | --- | --- | --- |
> | `proto/src/control.rs` | ~~419~~ **470** | `ReadyMessage.mic`, **sans bump de `CONTROL_VERSION`** (absence valant faux). ⚠️ voir la réserve de concurrence ci-dessus |
> | `agent/src/micro.rs` | **460** | neuf — **pur**, le tampon de gigue, la dérive, `LecteurMicro`. **A franchi 500 une fois** (518), rattrapé par `micro/tests.rs` (→ 260) ; puis `micro/frequence.rs` l'a allégé **dans le commit du plafond**, sans franchissement (467 → 448) |
> | `agent/src/demarrage/micro.rs` | **452** | neuf — le **puits de mesure** (`MICRO_MESURE=1`) |
> | `agent/src/opus/tests.rs` | **440** | neuf — extraction des tests d'`opus.rs`, qui retombe à **239** |
> | `agent/src/micro/tests_lecteur.rs` | **423** | neuf — extraction jouée **dans le commit du plafond**, `micro/tests.rs` **497 → 271**, sans franchissement |
> | `agent/src/wasapi/peripherique.rs` | **422** | neuf — **règle PURE** d'A-bis, aucun `cfg`, éprouvée sur l'hôte |
> | `client/src/webrtc.session.test.ts` | **419** | neuf — `webrtc.test.ts` a franchi 500, ses tests de session sortent (il retombe à **127**) |
> | `client/src/micro.test.ts` | **411** | neuf |
> | `agent/src/wasapi.rs` | ~~352~~ **370** | `pub mod rendu;`, l'encadré VB-Cable, et le doc-comment d'`open()` corrigé par la revue transverse |
> | `agent/src/main.rs` | **358** | le câblage du puits |
> | `client/src/webrtc.ts` | **348** | le transceiver `sendonly`, le sender exposé |
> | `agent/src/transport/evenements.rs` | **279** | la **discrimination par la DIRECTION** ; ses tests sortent (`evenements/tests.rs`, **264**) |
> | `client/src/micro.ts` | **288** | neuf — la bascule et ses trois états |
> | `agent/src/micro/tests.rs` | ~~497~~ **271** | voir l'extraction ci-dessus |
> | `agent/src/opus.rs` | ~~365~~ **239** | +le décodeur, PLC et FEC ; −les tests, extraits |
> | `agent/src/micro/dissimulation.rs` | **231** | neuf — **pur**, le plafond de dissimulation |
> | `agent/src/spectre.rs` | **220** | neuf — **pur, racine nue**, Goertzel, **aucune dépendance** |
> | `agent/src/transport/piste_micro/tests.rs` | **217** | neuf |
> | `agent/src/wasapi/rendu.rs` | **216** | neuf — la moitié COM d'A-bis. ⚠️ **ce nom était RÉSERVÉ au bloc E2 par le plan : E2 doit en choisir un autre** |
> | `client/src/stats.ts` | **193** | les mesures montantes |
> | `agent/src/diagnostics/audio.rs` | **178** | `AUDIO_PROBE` rend une **fréquence dominante**, plus seulement une crête |
> | `agent/src/transport/sonde_montante.rs` | **176** | neuf — la sonde 1, **bloquante** |
> | `agent/src/transport/piste_micro.rs` | **162** | neuf — **dépose, et rien d'autre** |
> | `proto/ts/control.ts` | **139** | le miroir TypeScript de `mic` |
> | `agent/src/diagnostics.rs` | **128** | revue transverse, commentaire seul |
> | `scripts/run-agent.sh` | ~~126~~ **128** | +2 : `AUDIO_PERIPHERIQUE` et `MICRO_MESURE` (hors portée de la règle) |
> | `agent/src/transport/initialisation.rs` | **104** | `set_reordering_size_audio(2)` et sa raison |
> | `agent/src/micro/frequence.rs` | **87** | neuf — **extrait AVANT** l'addition du plafond |
>
> ⚠️ **LE PLAFOND A ÉTÉ FRANCHI DEUX FOIS pendant cette branche, et rattrapé
> deux fois par une EXTRACTION, jamais par une compression** — `agent/src/micro.rs`
> à **518** → `micro/tests.rs` → **260**, et `client/src/webrtc.test.ts` à
> **517** → `webrtc.session.test.ts` → **127** (les quatre chiffres relevés par
> la commande sur les commits d'extraction et leurs parents).
> ⚠️ *Une première rédaction annonçait **trois** franchissements et nommait
> `agent/src/opus.rs` comme le troisième : **faux**. `opus.rs` n'a jamais dépassé
> 500 — il passe de **365 à 226 dans le commit même** qui ajoute le décodeur, son
> extraction étant jouée DANS l'addition. C'est une extraction, pas un
> rattrapage, et confondre les deux gonflerait le barème d'un franchissement
> imaginaire. Corrigé avant le commit, par mesure.*
>
> **Deux autres extractions ont été jouées SANS qu'aucun franchissement n'ait
> lieu**, parce qu'elles vivent DANS le commit de l'addition qu'elles
> accueillent : `agent/src/opus/tests.rs` (`opus.rs` **365 → 226** en ajoutant le
> décodeur) et `agent/src/micro/frequence.rs` (`micro.rs` **467 → 448** en
> ajoutant le plafond de dissimulation). Le second le dit dans son propre
> en-tête : *« EXTRAIT PLUTÔT QUE COMPRIMÉ — la doctrine du dépôt est de faire
> l'extraction AVANT l'addition, pas après l'avoir franchie »*.
>
> ⚠️ **Ce n'est PAS la même chose qu'une TÂCHE d'extraction dédiée, jouée avant
> celle qui ajoute, et il ne faut pas gonfler le barème en confondant les deux.**
> Le dépôt a **quatre** précédents de cette forme forte — D9 tâche 6
> (`capteur/serveur/instances.rs`, le premier) et les **trois** de D10 (tâches 1
> à 3) —, et **cette branche n'en ajoute aucun** : elle emploie la forme
> in-commit, plus légère, qui suffit tant que l'addition et son extraction
> tiennent dans une seule tâche.
> ⚠️ *Deux rédactions successives de ce seul paragraphe ont été fausses : la
> première annonçait « une seule fois auparavant, D9 tâche 6 » — **réfutée par un
> `grep` de ce fichier même**, où les trois extractions de D10 sont écrites noir
> sur blanc — et la seconde comptait `frequence.rs` comme une extraction
> anticipée alors que **la mesure montre que `micro.rs` n'a jamais franchi 500 à
> ce commit** (467 → 448). Les deux corrigées avant le commit, **par la commande
> et non par relecture**. C'est le patron que cette branche a documenté neuf
> fois : une affirmation de complétude écrite de mémoire.*
>
> ✅ **Chiffres voisins RELEVÉS ce jour-là** — ⚠️ **et « relevé » ne veut pas dire
> « dispensé de mesure » : la liste « à ne pas re-vérifier » de D11 a été prise
> en défaut sur deux de ses dix-huit lignes en moins d'une journée** :
> `encode/arret.rs` **500** (marge 0), `client/verify-webrtc.mjs` **494** (6),
> `superviseur/table.rs` **492** (8), `capture.rs` **492** (8),
> `transport/socket.rs` **481** (19), `transport/piste_video.rs` **477** (23),
> `capteur/distante/tests.rs` **474** (26), `congestion/controleur.rs` **472**
> (28), `transport/tick/tests/audio.rs` **471**, `transport/piste_audio.rs`
> **471**, `transport/adaptation.rs` **468** (32),
> `diagnostics/multifenetre/reprise/passes.rs` **465**,
> `moniteurs_virtuels/pilote.rs` **463**, `geometry.rs` **459**,
> `diagnostics/multifenetre/montee.rs` **459**, `diagnostics/capture.rs` **457**,
> `turn/allocation.rs` **456**, `client/src/main.ts` **451**,
> `moniteurs_virtuels.rs` **448**, `superviseur/placement.rs` **441**.

**Vérifier l'état** :

```bash
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
```

### Convention de module enfant : `#[path]` chez le parent, ou racine nue

**Tranché le 7 août 2026 (tâche 17, sous-bloc D10), après que le sous-bloc
précédent a relevé `survie_verdict.rs` comme une « déviation » puis s'est
lui-même trompé en la corrigeant (voir le leg n°9 de D9, plus bas). Ce dépôt
avait deux conventions pour un module hors du `#[cfg(windows)]` de son parent
logique, sans jamais avoir écrit la règle qui les départage.

**Portée de la règle** : elle ne s'applique QU'aux modules qu'on extrait d'un
fichier `#[cfg(windows)]` (ou autrement non portable) pour que leur logique
*pure* compile et se teste sur l'hôte Linux, et qui doivent de ce fait devenir
des **frères de premier niveau** de ce parent, déclarés dans `main.rs`. Un
module `#[cfg(windows)]` ordinaire qui n'a pas besoin d'exister sur l'hôte
reste un enfant normal, déclaré par un simple `mod` **à l'intérieur** de son
parent gaté (`capture.rs::mod enumeration;`, `capture.rs::mod types;`,
`windows_source.rs::mod redimensionnement;`) : il ne se pose jamais la
question ci-dessous, faute d'avoir jamais besoin de sortir de l'arbre de son
parent. Est également hors de portée l'usage de `#[path]` pour scinder un
module de *tests* trop long À L'INTÉRIEUR d'un fichier par ailleurs portable
(`superviseur/table.rs` déclare ainsi `#[path = "table/tests.rs"] mod tests;`
et `#[path = "table/tests_relance.rs"] mod tests_relance;`, tous deux
`#[cfg(test)]`, tous deux internes à `table.rs`, sans rapport avec une
frontière `#[cfg(windows)]`) : c'est le même mécanisme Rust, employé pour une
raison différente (la règle des 500 lignes), et il ne suit pas la convention
ci-dessous.

**La règle, pour les modules dans cette portée : le NOM du module tranche.**

- **Le nom du module s'écrit `<parent>_<enfant>`**, où `<parent>` nomme un
  module de premier niveau existant (déclaré dans `main.rs`) : le fichier
  reste physiquement chez ce parent (`src/<parent>/<enfant>.rs`), et se
  déclare dans `main.rs` par
  `#[path = "<parent>/<enfant>.rs"] mod <parent>_<enfant>;` — c'est la
  déclaration qui franchit le `#[cfg(windows)]` du parent, le fichier
  physique, lui, n'a pas bougé de sous son parent. Exemples :
  `capture_reprise` (`capture/reprise.rs`), `windows_source_sortie`
  (`windows_source/sortie.rs`), `windows_source_telemetrie`
  (`windows_source/telemetrie.rs`).
  ⚠️ **Si plusieurs modules de premier niveau sont chacun un préfixe valide du
  nom** (cas non encore rencontré, mais qui existe dès aujourd'hui :
  `windows_source_sortie` est LUI-MÊME un module de premier niveau depuis D1,
  donc un futur `windows_source_sortie_conversion` préfixerait à la fois
  `windows_source` et `windows_source_sortie`), **c'est le préfixe le PLUS
  LONG — le plus spécifique — qui l'emporte.** Le fichier physique suit :
  `windows_source_sortie_conversion` se rangerait sous
  `windows_source/sortie/conversion.rs` (enfant de `windows_source_sortie`,
  lui-même à `windows_source/sortie.rs`), pas sous
  `windows_source/sortie_conversion.rs`. Cette clause ne change le
  classement d'aucun des six cas relevés ci-dessous : aucun n'a de second
  préfixe candidat plus court.
  **Une égalité de longueur entre deux préfixes candidats ne peut pas se
  produire** : un préfixe valide s'arrête toujours sur une frontière de
  tiret bas (`<parent>_`). Si deux noms de module de premier niveau
  DIFFÉRENTS étaient chacun un préfixe de la MÊME longueur du nom à ranger,
  ils seraient la même sous-chaîne — donc le même nom. L'égalité est
  structurellement exclue, pas seulement absente des cas rencontrés à ce
  jour ; il n'y a donc rien à trancher au-delà de « le plus long l'emporte ».
- **Le nom du module se comprend SANS référence à un parent** — il ne porte
  le préfixe d'aucun module de premier niveau existant (`geometry`,
  `sortie_dxgi`, `survie_verdict`) : il vit à la racine nue, `mod <nom>;`
  ordinaire dans `main.rs`, fichier `src/<nom>.rs`. **La profondeur du module
  dont on l'extrait ne change rien** : `survie_verdict` vient de
  `diagnostics::multifenetre::mode_sortie::persistance`, quatre niveaux plus
  bas que `main.rs`, et n'a pourtant aucun nom de parent court et unique à
  préfixer — la règle le range à la racine comme `geometry` et `sortie_dxgi`,
  extraits pour la même raison (compiler sur l'hôte) d'un parent tout aussi
  gaté (`capture.rs`, `window.rs`).

**Cas du parent qui n'existe pas encore quand on écrit le module** : la règle
se résout mécaniquement vers la racine nue — un nom ne peut préfixer un
module de premier niveau qui n'est pas encore déclaré dans `main.rs`. **Effet
de bord non traité** : si un module homonyme du préfixe apparaît plus tard
(un futur `mod windows_source_sortie_conversion` créé avant que
`windows_source_sortie` existe, par exemple), rien ne force à re-hisser le
premier sous le second après coup — aucune règle de re-hissage n'est posée
ici, à écrire le jour où le cas se présente réellement.

**Vérifiée sur les six cas existants au 7 août 2026**
(`grep -rn '#\[path' agent/src/`, `ls agent/src/*.rs`, depuis `agent/`) : les
trois noms préfixés sont TOUS déclarés par `#[path]` chez leur parent, les
trois noms autonomes sont TOUS à la racine nue. **Aucune exception**, y
compris sous la clause du préfixe le plus long ci-dessus.
`survie_verdict.rs` s'y conforme déjà — il n'a jamais eu besoin de bouger.

**Preuve, portée ici plutôt que dans un rapport de tâche gitignoré, que la
clause du préfixe le plus long ne change le classement d'aucun des trois
noms préfixés** : pour chacun, aucun AUTRE module de premier niveau n'en est
un préfixe plus court et valide.
- `capture_reprise` : seul `capture` le préfixe.
- `windows_source_sortie` : seul `windows_source` le préfixe.
- `windows_source_telemetrie` : seul `windows_source` le préfixe —
  `windows_source_sortie` n'en est PAS un préfixe : le nom continue par
  `_telemetrie`, pas par `_sortie`.

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

> ❌ **DEUX CLAUSES DE CE RELEVÉ SONT PÉRIMÉES DEPUIS LE 19 AOÛT 2026
> (chantier E — microphone).** Le relevé de juillet reste vrai **comme
> histoire** : c'est son emploi au présent qui ne l'est plus.
>
> - **« Aucun pilote audio virtuel supplémentaire n'est à installer » est
>   FAUX** : **VB-Cable a été installé** sur la VM pour le chantier E, et il
>   ajoute **deux** endpoints que le tableau ci-dessus ne porte pas — « CABLE
>   Input » (**rendu**) et « CABLE Output » (**capture**). C'est aussi, à ce
>   jour, le **seul** endpoint de capture local de cette VM : le relevé du
>   19 août 2026 (avant installation) en comptait **zéro**.
> - **Le tableau des périphériques est donc INCOMPLET**, et il ne sera pas
>   réécrit ici : il date sa mesure, et la réécrire effacerait ce qu'elle
>   établissait. Le relevé courant vit dans la section « Chantier E » en pied
>   de ce fichier.
>
> 🔴 **Et l'installation a eu un EFFET DE BORD qui a cassé le produit** : elle
> a fait basculer le **rendu par défaut** de Windows sur le câble virtuel, que
> rien n'alimente. Le loopback du chantier A, qui suivait ce défaut, s'est mis
> à **capter du silence sans qu'aucune ligne de journal ne le dise**. C'est
> l'objet de la correction **« A-bis »** et de la variable
> `AUDIO_PERIPHERIQUE` — voir le tableau des variables et la section
> « Chantier E ».
>
> ⚠️ **La dernière phrase — « reste à confirmer lequel est le périphérique par
> défaut » — a cessé d'être la BONNE QUESTION.** Le produit ne suit plus un
> défaut : il retient **celui qu'on lui désigne**, et journalise à chaque
> ouverture celui qu'il a réellement retenu. Savoir quel est le défaut reste
> un diagnostic utile ; ce n'est plus une dépendance.

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

> ⚠️ **CE QUE `AUDIO_PROBE` REND A CHANGÉ le 19 août 2026 (correction
> « A-bis »), et le relevé ci-dessus ne décrit plus sa sortie.** Elle rendait
> une **crête**, qui distingue « du son » de « rien » mais jamais « MON son »
> d'un autre. Elle rend désormais **aussi la FRÉQUENCE DOMINANTE** de ce
> qu'elle capte (`agent/src/spectre.rs`, filtre de Goertzel, pur et éprouvé sur
> l'hôte, **sans aucune dépendance neuve**), sur une fenêtre glissante de 2 s.
>
> **C'est ce qui en fait un instrument de MESURE et plus seulement de
> présence** : lancée deux fois sur la même machine, avec et sans
> `AUDIO_PERIPHERIQUE`, elle rend deux relevés opposés. Le dépôt avait établi
> la règle en D7 — *on juge un son à sa fréquence dominante, jamais à un compte
> d'octets* —, et la sonde du chantier A ne l'appliquait pas encore.
>
> ⚠️ **`[Console]::Beep` reste un faux négatif**, et le conseil ci-dessus tient
> sans changement.

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
éphémères (~~`signaling/src/ice.ts`~~ **`plateforme/src/signaling/ice.ts`**
depuis le sous-bloc P1, 19 août 2026 — le paquet `signaling/` n'existe plus,
voir la section « Sous-projet ⑤ Plateforme » en fin de fichier) dérivés d'un
secret qui ne quitte jamais le
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

⚠️ **Le processus a changé de nom au sous-bloc P1 (19 août 2026) : ce n'est
plus `signaling/`, c'est `plateforme/` — `cd plateforme && npm start`.** Le
piège ci-dessous est **entier et inchangé**, et il s'est même AGGRAVÉ : le
service lit désormais ~~quatre~~ **SIX** variables de plus (`PLATEFORME_HOTE`,
`PLATEFORME_PORT`, `PLATEFORME_BASE`, `PLATEFORME_BASE_URL`, et — depuis le
sous-bloc P2, 19 août 2026 — **`PLATEFORME_SECRET_JETON`** et
**`PLATEFORME_ORIGINE_CLIENT`**), dont ~~la première~~ **DEUX n'ont aucun
défaut** et cassent le lancement : `PLATEFORME_HOTE` **et
`PLATEFORME_SECRET_JETON`**, cette dernière étant en outre refusée si elle est
plus courte que `LONGUEUR_SECRET_MIN`. `TURN_URL` reste lue par le relais
de signaling qui vit à l'intérieur.

⚠️ **`.env` ne porte AUCUNE `PLATEFORME_*`** (relevé le 19 août 2026 : il n'a
que les `TURN_*`) : `cd plateforme && npm start` après un `source .env` **ne
démarre donc pas**, et il dit pourquoi. C'est le comportement voulu — voir
`plateforme/src/config.ts` — mais c'est aussi exactement le piège de cette
section, sous une forme neuve : l'environnement qu'on croit complet ne l'est
pas.


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
| `AUDIO_FAUTE_LECTURE=<n>` | **Sous-bloc D10, tâche 3** — **variable de BANC, jamais une configuration livrée**. Fait échouer les *n* prochaines **lectures** WASAPI (`agent/src/windows_audio/fil.rs`) ; au-delà de `LECTURES_ECHOUEES_MAX = 10` la capture se déclare morte, ce qui déclenche la reconstruction. **ABSENTE = DÉSARMÉE.** Budget **global au processus** depuis D10 — ⚠️ il était **relu par fil** au premier jet, et chaque capture reconstruite recevait alors un budget neuf : **le chiffre-juge ne pouvait pas quitter zéro**, sur un produit pourtant corrigé. Trace : `injection de fautes de lecture audio ARMEE (banc)`. Transmise par `scripts/run-agent.sh:42`. 🔵 **Le compte de fautes CONSOMMÉES est un témoin d'armement INDÉPENDANT du spectre** : le garde `if !emettait` précède l'injection, donc **une source muette ne peut pas consommer de faute**. ⚠️ **Cette ligne manquait à ce tableau depuis D10** ; ajoutée par la revue transverse de D11 |
| `AUDIO_FAUTE_RECONSTRUCTION=<n>` | **Sous-bloc D11, tâche 4** — **variable de BANC, jamais une configuration livrée**. Fait échouer les *n* prochaines **reconstructions** de capture audio (`agent/src/transport/piste_audio/injection.rs`), et c'est ainsi que le critère ④ de D10 — « une capture irrécupérable retombe sur la promotion d'une voisine », que D10 déclarait *non démontrable par le protocole prescrit* — devient atteignable. ⚠️ **Convention INVERSE de `PLEIN_ECRAN` : ABSENTE = DÉSARMÉE**, présente et non nulle = armée (jamais `is_ok()`). ⚠️ **Budget GLOBAL AU PROCESSUS** (`OnceLock` + `AtomicU32`, `fetch_update`), **jamais par appel** — c'est la leçon que D10 a payée sur `AUDIO_FAUTE_LECTURE` : un budget relu par fil se réarme à chaque reconstruction, et le chiffre-juge qu'il sert devient **structurellement incapable de quitter zéro**. Lue dans l'**enfant**. Transmise par `scripts/run-agent.sh:44`. Trace, **seulement si armée** : `injection de fautes de RECONSTRUCTION audio ARMEE : banc, jamais une configuration livrée` (`warn!`). La faute emprunte le `warn!` du leg 6, d'où `erreur="faute injectée (AUDIO_FAUTE_RECONSTRUCTION)"` au journal. ⚠️ **Le compte à poser n'est PAS `> RECONSTRUCTIONS_MAX`** : le réarmement de D9 réapprovisionne le budget, et le compte juste est **`(REARMEMENTS_MAX + 1) × RECONSTRUCTIONS_MAX` = 18** — mesuré tel quel |
| `AUDIO_FAUTE_LECTURE_MS=<ms>` | **Sous-bloc D11, tâche 5** — **variable de BANC**. Borne **dans le temps** l'armement de `AUDIO_FAUTE_LECTURE` (prédicat pur `crate::audio::injection_encore_armee`, testé sur l'hôte ; lue dans `windows_audio/fil.rs`). **ABSENTE = ILLIMITÉ**, donc le comportement de D10 est strictement préservé et ses recettes restent reproductibles. Sans elle, la voisine qu'on veut voir promue meurt **à l'instant même de sa promotion** et le critère reste non démontrable. Transmise par `scripts/run-agent.sh:43`. La trace est celle de `AUDIO_FAUTE_LECTURE`, **enrichie du champ `fenetre_ms`** — un seul `warn!`, à dessein : deux traces au même instant se compteraient comme deux événements (piège maison de D6). ⚠️ **L'ORIGINE DU BUDGET EST LE DÉMARRAGE DU FIL, PAS L'ÉLECTION** : `3000` rend le critère **inatteignable** (le premier arbitrage du capteur arrive ~2,7 s après le démarrage du fil, et la fenêtre se referme 6 ms avant l'élection de la porteuse), **`5000` est la valeur dérivée de la mesure**. ⚠️ **Non calibrée** : c'est une valeur de banc, pas une constante de produit |
| `AUDIO_PERIPHERIQUE=<nom ou identifiant>` | **Correction « A-bis », 19 août 2026** — **variable de PRODUIT**, pas de banc. Désigne le point de terminaison de **rendu** que le loopback de session doit capter, au lieu de subir le rendu **par défaut** de Windows. ⚠️ **Convention VALUÉE** — celle de `MULTIFENETRE_SORTIE` et `BUDGET_BPS`, **pas** celle de `PLEIN_ECRAN` : **absente ou vide, le comportement est EXACTEMENT celui d'avant** (le défaut de Windows). Trois critères, dans cet ordre : **identifiant d'endpoint** exact (`IMMDevice::GetId`, forme `{0.0.0.00000000}.{guid}` — stable, opaque), **nom convivial** exact (`PKEY_Device_FriendlyName`), puis **sous-chaîne insensible à la casse**. 🔵 **Une sous-chaîne AMBIGUË refuse de trancher** (`Choix::Ambigu`) au lieu de prendre le premier : prendre le premier serait retomber sur un **rang d'énumération** par la porte de derrière — la leçon des index DXGI de D1, payée une fois, appliquée ici d'avance. Règle **PURE** dans `agent/src/wasapi/peripherique.rs` (aucun `cfg`, éprouvée sur l'hôte), moitié COM dans `agent/src/wasapi/rendu.rs`. Lue par `LoopbackCapture::open`, donc **le mode MONO-FENÊTRE et la sonde `AUDIO_PROBE` seulement** — `pour_processus` (multi-fenêtres, D7+) **ne résout aucun endpoint** et n'est pas concerné. Transmise par `scripts/run-agent.sh`. **Le périphérique réellement retenu est JOURNALISÉ à chaque ouverture**, et tout repli l'est aussi : jamais silencieux |
| `MICRO_MESURE=1` | **Chantier E, bloc E1** — **variable de BANC, jamais une configuration livrée**. Arme le **puits de mesure du micro** (`agent/src/demarrage/micro.rs`) : un consommateur qui joue le rôle du futur fil WASAPI d'E2, retire du tampon à la cadence réelle et journalise ce qu'il obtient. ⚠️ **Convention `=1` qui ARME** — et non `=0` qui désarmerait : le puits n'est **pas** livré, donc c'est sa présence qu'il faut déclarer, pas son absence. Trace de contrôle, dont **l'absence prouve que la variable n'a pas atteint le processus** : `micro de mesure ARME (MICRO_MESURE=1) : instrument de banc, jamais une configuration livree`. Trace périodique : `micro mesuré`, portant `crete` et `frequence_hz` **côte à côte** (une crête sans fréquence est du bruit), `plc` et `plc_plafonnees` **côte à côte** (le second est **disjoint** du premier — c'est ce qui rend le plafond de dissimulation observable), plus `deposees`, `famines`, `occupation_ms` et `occupation_max_ms`. Transmise par `scripts/run-agent.sh` |

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
  > 🔴 **CETTE PARADE EST INCOMPLÈTE, et son insuffisance a été payée le
  > 19 août 2026 (chantier E).** `-p agent` ne purge **que** le crate `agent` :
  > l'artefact du crate **`proto`**, lui, survit. Or **l'horloge de la VM avance
  > sur celle de l'hôte** — le rlib de `proto` paraît donc plus récent que ses
  > propres sources fraîchement synchronisées, et cargo le **saute**.
  >
  > **Le symptôme ne ressemble en rien à un cache périmé** : la compilation
  > s'arrête sur une **erreur de type portant sur une signature de `proto` qui
  > est pourtant à jour dans le fichier qu'on vient de lire**. On cherche alors
  > un défaut dans du code correct.
  >
  > **La parade complète est de nommer les DEUX crates :**
  > ```bash
  > cargo clean --release -p proto -p agent
  > ```
  > **Règle générale** : purger le crate qu'on compile ne suffit pas quand une
  > dépendance interne du même dépôt a franchi le même partage réseau. Purger
  > toute la chaîne locale, ou rien.
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
  ✅ **CETTE ALTERNATIVE EST TRANCHÉE, dans le sens « par GUID » (19 août 2026,
  sous-bloc D11, recette ⑤).** Dans une MÊME exécution, **sept** sorties
  naissent à la taille demandée (1280×720) et **une seule** à celle du registre
  (3840×2160) — et c'est **toujours la sortie du QUATRIÈME GUID**
  (`…677541430004`), sur **SEPT exécutions**. **Une écriture n'empoisonne donc
  PAS toutes les sorties futures.** ⚠️ **Ce que cela ne dit pas** : *pourquoi le
  quatrième*, ni pourquoi la taille passée à la création est ignorée. Deux faits
  mesurés, non expliqués.
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

✅ **ELLE EST TRANCHÉE DEPUIS LE 19 AOÛT 2026 (sous-bloc D11, recette ⑤), et
dans le sens « par GUID »** : sept sorties nées à la taille demandée coexistent
avec **une seule** née à celle du registre — toujours celle du **quatrième
GUID** — **dans la même exécution, sept exécutions sur sept**. La naissance à la
mauvaise taille, qui est le mécanisme du blocage, est donc bien **par GUID**.
⚠️ **Pourquoi le quatrième reste inexpliqué**, et **rien ne nettoie toujours le
registre**.

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
pas été corrigé** (interruption assumée de la ronde de correction). ~~Le
contredire est le premier travail de qui le relira.~~

❌ **CETTE DERNIÈRE PHRASE EST FAUSSE, et elle l'était déjà en germe au moment
où elle a été écrite : elle présume que ce rapport SURVIVRA pour qu'on le
relise (7 août 2026, tâche 18, sous-bloc D10).** Son espace de travail
était `.superpowers/sdd/`, **gitignoré et jamais commité** — vérifié par la
commande : `git log --all --diff-filter=A --name-only -- '*task-14*'` ne rend
**aucun résultat pour ce chantier**, et aucune branche ni aucun *stash* n'en
porte de copie. **Le rapport a disparu avec la session de D9 qui l'a écrit, et
il n'y a donc personne à qui « le relire ».** Ce paragraphe-ci, et celui du
document de résultats de D9 qui porte la même phrase, **SONT** la correction —
ils ne renvoient plus vers elle. Voir aussi le sort des neuf constats parqués,
plus bas : la même disparition les rend, eux, irrécupérables.

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
| le registre | `agent/src/capteur/sommeil.rs` (~~269~~ **325**, D10) + `sommeil/registre.rs` (~~331~~ **346**, D10) | `REPIT_REARMEMENT_AUDIO`, `REARMEMENTS_MAX`, la génération monotone. **Extrait sur exigence de revue** après une compression que ce dépôt interdit |
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
  ✅ **TRANCHÉE PAR D11 : par GUID** (voir l'encadré ci-dessus). ⚠️ **La seconde
  moitié de cette ligne TIENT INTÉGRALEMENT — rien ne nettoie le registre.**
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

> ✅ **CE QUE D10 A FAIT DE CETTE LISTE (7 août 2026) — huit points sur douze
> sont réglés, et deux de ceux qui restent le sont pour une raison neuve.** Le
> tableau leg par leg, avec ses réserves, vit à la fin de la section
> « Sous-bloc D10 » ci-dessous ; il n'est pas répété ici. **Ce qui reste dû**,
> en une phrase chacun : le n°3 (l'A/B) a été **rejoué à huit fenêtres et
> n'établit toujours rien** ; le n°7 (le maillon du `Resize`) est **instrumenté
> mais non identifié**, et son rejeu n'a pas eu lieu sur la VM ; les **six**
> constats parqués du n°10 sont **perdus**, définitivement. Le n°1 est fermé
> **et exercé sur la VM** — mais D10 y a trouvé, en revue transverse, que son
> remède est **inerte en mono-fenêtre**, ce qui est un legs neuf et non une
> réouverture de celui-ci.

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
   ⚠️ **REJOUÉ PAR D10 à huit fenêtres — n'établit toujours rien —, puis
   ÉCARTÉ PAR DÉCISION EN D11, avec sa condition de réouverture** : un montage
   dont la charge d'hôte est **CONTRÔLÉE** (et non seulement appariée), sur **au
   moins huit paires**. Chantier de banc à part entière, pas une tâche de solde.
   *Cela n'affirme pas que l'appel est sans effet.*
   le bruit intra-bras (+83,1 %) dépasse le signal (+23,2 %). Il faut **plus
   d'exécutions**, pas un autre montage — et de préférence à plus de trois
   fenêtres, donc après le legs n°4.

**Neufs, propres à D9 :**

4. 🔴 **Nettoyer la pollution de registre, ou s'en rendre immunisé.** Elle bloque
   le produit à **trois** fenêtres sur cette VM, aujourd'hui, et **rien ne
   nettoie derrière**. Deux voies, non arbitrées : purger le registre au
   démarrage du superviseur, ou **tolérer** une sortie née à la mauvaise taille
   plutôt que la rendre au pilote (`superviseur/boucle.rs`). ⚠️ ~~**Sa portée
   reste inconnue** — par GUID, ou global ?~~ ✅ **PAR GUID, tranché par D11
   (recette ⑤, sept exécutions).** ⚠️ **Et la pollution subsiste : rien ne
   nettoie derrière.**
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
   ✅ **TRANCHÉ le 7 août 2026 (tâche 17, sous-bloc D10) : la règle est
   maintenant écrite** (§« Convention de module enfant… », tête de ce
   fichier) — un module dont le nom porte le préfixe d'un parent de premier
   niveau (`<parent>_<enfant>`) se déclare par `#[path]` chez ce parent ; un
   module au nom autonome vit à la racine nue. **`survie_verdict.rs` s'y
   conforme DÉJÀ** : son nom ne préfixe aucun parent, la règle le range à la
   racine, exactement où il vivait. **Ce leg est CLOS sans qu'aucun fichier
   n'ait été déplacé ni renommé** ; la qualification de « déviation » (D9,
   tableau de tête et cette entrée) était fausse depuis le début — corrigée
   aux deux endroits.
10. ⛔ ~~Neuf constats de revue PARQUÉS sur la tâche 14 (recette ①)… Ils sont
    toujours dans le rapport versé.~~

    ✅ **REQUALIFIÉ (7 août 2026, tâche 18, D10) : « toujours dans le rapport
    versé » est faux — ce rapport a disparu** (voir la correction du § leg 10
    ci-dessus, « qui le relira »). **Sur les neuf constats, trois SURVIVENT**
    parce qu'ils avaient déjà été extraits vers ce document permanent-ci avant
    que le rapport source ne disparaisse — **TRAITÉS, sans action
    supplémentaire requise** :
    - l'A/B rouge/vert non propre du critère ①c — **déjà porté** au §
      « Le critère ①c porte une réserve de méthode qui n'a pas été corrigée »,
      quelques paragraphes plus haut ;
    - la pièce du critère (a) qui ne couvre qu'une page sur treize —
      **déjà reprise** dans le document de résultats de D9, § 3 ;
    - « le shell réémet `fenetre-ouverte` pour une fenêtre déjà ouverte » —
      **REQUALIFIÉ tel quel** : ce n'est pas un défaut d'instrument mais un
      **comportement du PRODUIT, mécanisme non élucidé**. Aucune pièce
      supplémentaire ne permet d'aller plus loin que cette phrase.

    **Les SIX autres constats sont PERDUS**, pas seulement parqués : leur
    contenu n'a survécu nulle part ailleurs dans ce dépôt, et le rapport qui
    les portait n'existe plus (§8, vérifié par la commande). **Inventer une
    liste serait pire que d'admettre qu'elle est perdue** — constat honnête,
    pas une lacune de recherche.

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

## 🧹 Sous-bloc D10 — solder la branche : la sortie cesse d'être la fenêtre (7 août 2026)

Résultats complets :
`docs/superpowers/plans/2026-08-07-multifenetres-solder-la-branche-resultats.md`.
Conception : `docs/superpowers/specs/2026-08-07-multifenetres-solder-la-branche-design.md`.
Journaux : `docs/superpowers/plans/journaux-multifenetres-d10/` — **deux
familles de lecture seulement**, la plus simple de tous les sous-blocs :

| Famille | État | Ce qu'il faut faire |
| --- | --- | --- |
| tous les `*-plat.log`, et les `*.json` de pilote | UTF-8, **ANSI déjà retirées** | rien |
| les journaux d'agent bruts (`agent-*.log`) | UTF-8, **séquences ANSI PRÉSENTES** | `sed 's/\x1b\[[0-9;]*m//g'` — ou lire le `-plat` jumeau, versé pour chacun |

⚠️ **Une exception, et elle est instructive** : `agent-ab-desarme-1.log` et son
`-plat` portent **558 octets NUL** après leur dernier événement réel (artefact
de lecture CIFS pendant que Windows écrivait encore). `grep` sans `-a` classe
alors le fichier « binaire » et **rend une sortie vide, pas zéro** —
indiscernable d'un compte nul. **`grep -a` lève le doute** ; les comptes
retrouvés concordent avec le JSON écrit par le pilote.

D10 solde les legs de D9 : le 🔴 registre qui plafonnait le produit à trois
fenêtres, l'audio mort que rien ne reconstruisait, et quatre legs froids.

### ① Le résultat central : 3 → 10 fenêtres, 32 → 0 erreur, sur un registre laissé sale

**Une sortie d'affichage virtuelle ne naît pas à la taille demandée mais à la
dernière taille laissée au registre Windows** (fait mesuré par D8, jamais
expliqué). Le superviseur la rendait au pilote et recommençait. **Il l'accepte
désormais si elle est assez grande**, y pose la fenêtre à la **taille retenue**
(`min` axe par axe entre le viewport borné et la taille DXGI réelle), et la
capture **recadre ce rectangle dans la duplication de la sortie**.

| | ROUGE (`main`, `c9b7a31`) | VERT 1 (branche) | VERT 2 |
| --- | --- | --- | --- |
| `fenêtre attachée au capteur` | **3** | **10** | **10** |
| `introuvable dans la topologie DXGI` | **32** | **0** | **0** |

**Le rouge est un vrai rouge par signature comportementale** : `main` recycle
dix identifiants de sortie sur **35 créations et 32 destructions** sans jamais
dépasser trois attaches — il boucle indéfiniment. **Le registre est établi sale
sur le VERT aussi**, et c'est le point qui décide :
`duplication de sortie établie desktop_width=3840 desktop_height=2160`, relevé
verbatim aux deux exécutions vertes.

⚠️ **Le brief attendait 8, la mesure donne 10, et l'écart est expliqué par le
journal** : `CAPACITE = 10` (fenêtres **suivies**) et `vivier::PLAFOND_EVEIL = 8`
(fenêtres **éveillées**) sont deux constantes distinctes. Sur les dix attachées,
huit ont des images qui croissent, deux sont figées — `w-2` et `w-4`, les deux
premières lancées, donc les premières candidates au sommeil LRU. **C'est
journalisé par l'agent** (`endormie=true cadence="0.0"` contre `images=539
endormie=false cadence="53.8"`), pas déduit du navigateur.

⚠️ **La séparation des flux n'est PAS prouvée pour autant**, et c'est la revue
qui l'a relevé : le contrôle de distinction d'empreintes **ne peut pas échouer**
sur une page vivante. L'échantillonnage n'est pas simultané (187 et 245 ms
d'étalement) et la mire dérive à chaque trame — **deux pages décodant le MÊME
flux rendraient donc des empreintes différentes elles aussi**. Le contrôle garde
tout son pouvoir sur les **deux pages figées** seulement, où il ne trouve aucune
collision.

⚠️ **`CDS_UPDATEREGISTRY` pollue toujours le registre : c'est le PRODUIT qui y
est devenu indifférent, pas le registre qui a été nettoyé.** Rien, dans ce
dépôt, ne nettoie derrière. Le constat de mesure en tête de
`agent/src/capteur/plein_ecran.rs` porte cette correction — sa clause « ce qui
bloque le produit » n'est plus vraie.

### ② La recette audio a trouvé un défaut de production qu'aucun test d'hôte ne pouvait voir

**C'est le résultat le plus précieux du sous-bloc, et il a fallu TROIS passages
pour obtenir le vert.**

**Passage 1** — la reconstruction se déclenche (`capture audio reconstruite = 2`,
deux exécutions) et **la fenêtre n'entend rien** : −1000 dB,
`compteurs audio actif=true = 0`. **Chaîne refermée indépendamment par la revue,
en cinq maillons** : la source reconstruite **naît muette** ; rien ne la réarme ;
`appliquer_audio` n'est atteint que par un ordre du capteur ; le capteur
**n'émet que sur changement** ; et `AudioVivant` exige un **paquet réel** qu'une
source muette ne produit pas, pendant qu'`AudioMort` ne part pas non plus
puisque la reconstruction a **réussi**. **Un état ABSORBANT, pas un retard.**

⚠️ **Portée bornée** : le défaut DIAGNOSTIQUÉ ci-dessus — naître muette — est
propre à la branche `pour_processus` (multi-fenêtres), le mode session
appelant `emettre(true)` dans `new()`.

🔴 **MAIS NE PAS LIRE « le mode session est immunisé » COMME UN FEU VERT SUR CE
CAS : le CORRECTIF le casse à son tour, et c'est le legs n°4 ci-dessous.** Le
réarmement `set_actif(audio_porteuse)` s'applique **sans condition**, donc aussi
à une source reconstruite par `new()` — et en mono-fenêtre `audio_porteuse` vaut
toujours `false`, faute de capteur pour l'écrire. **Le mode session est immunisé
du défaut d'origine et cassé par son remède** ; les deux phrases sont vraies et
il faut les tenir ensemble. Détail complet à la revue transverse et au legs n°4.

⚠️ **Pourquoi aucun test d'hôte ne pouvait le voir : les sources factices
implémentent `set_actif` en NO-OP.** 456 tests verts sur un produit muet.

**Passage 2** — le mécanisme est réarmé, le chiffre-juge reste bloqué : c'est un
**plafond d'instrument**, `AUDIO_FAUTE_LECTURE` étant relu **par fil**, donc
réapprovisionné à chaque reconstruction. Démontré deux fois — par le code, et
par l'arithmétique des pièces (`164 = 16×10+4`, `157 = 15×10+7` : **pas un seul
appel réel** à `capture.read()`).

> 🔵 **La preuve IMMUNE AU MONTAGE que la revue a trouvée, et c'est un
> instrument qui dormait dans les pièces sans que personne l'y lise : une
> source muette NE PEUT PAS CONSOMMER DE FAUTE**, le garde `if !emettait`
> précédant l'injection. **La consommation de fautes est donc un témoin
> POSITIF d'armement.** Avant le correctif : **0/2**. Après : **15/15**.

> ✅ **Un legs de D9 levé en passant** : le chemin `AudioMort` → capteur →
> réarmement, que ce fichier déclarait « **jamais** tourné sur la VM », a
> désormais tourné **dix fois** (5 réarmements par exécution, 2 exécutions).

**Passage 3** — budget d'injection rendu **global au processus**, session portée
à 60 s, jugement sur la **fréquence dominante** : **441 Hz à −40 dB** pour une
cible assignée de 440 Hz, plancher −158 dB, **aux deux exécutions et aux deux
points de contrôle** (t+15 s et t+60 s), `compteurs audio actif=true` = 2 et 2.
**Fermeture arithmétique** : `14 + 1 = 15 = AUDIO_FAUTE_LECTURE`, aux deux.
**L'ordre est établi** : reconstruction à t+3,6 s et t+5,8 s, mesures à t ≥ 18 s
— **ce n'est pas la capture d'origine**.

### Les critères, avec leur nombre d'exécutions

**Aucun taux n'est revendiqué nulle part.**

| # | Critère | Verdict | Exéc. |
| --- | --- | --- | --- |
| ① | Huit fenêtres, registre sale | **TENU et dépassé** — 10 attachées, 0 erreur | 2 vertes + 1 rouge |
| ② | L'image est celle de la fenêtre | **PARTIELLEMENT TENU** — 10 sessions vivantes, 8 qui décodent, 2 figées cohérentes avec le sommeil. **Séparation des flux NON prouvée** | 2 |
| ③ | Une capture audio morte est reconstruite | **TENU** — 441 Hz / −40 dB | 2 (après 2 passages qui ont diagnostiqué) |
| ④ | Le repli sur la promotion | **NON DÉMONTRABLE PAR LE PROTOCOLE PRESCRIT** | 1 tentative |
| ⑤ | L'A/B apparié | **N'ÉTABLIT RIEN** — 4 paires, `− − + +` | 4 paires (8 exéc.) |

**Critère ④, et pourquoi il échoue** : le brief prescrivait de tuer l'arbre de
processus cible. Or `pid_de_fenetre(hwnd)` lie **toujours** la cible audio au
PID propriétaire du HWND : tuer cet arbre tue **indissociablement la fenêtre**.
La mort côté vidéo est détectée en **≈ 1,3 s**, quand le budget de
reconstruction met **≥ 6 s** à s'épuiser — rapport **≈ 1 pour 5**, et ce n'est
pas un artefact de vitesse d'exécution. ⚠️ **L'énoncé resserré en revue** : le
critère n'est pas démontrable **par ce protocole-là**, pas dans l'absolu. **La
voie qui l'atteindrait est nommée et NON construite** : un
`AUDIO_FAUTE_RECONSTRUCTION` calqué sur `AUDIO_FAUTE_LECTURE`.

**Critère ⑤, l'A/B** — 4 paires dos à dos, **à huit fenêtres** (le plafond de
trois qui privait l'A/B de D9 de base de comparaison est levé par ① ) :

| Paire | Armé | Désarmé | Diff. | Signe |
| --- | --- | --- | --- | --- |
| 1 | 8,004 | 8,157 | −0,153 | **négatif** |
| 2 | 8,086 | 8,672 | −0,586 | **négatif** |
| 3 | 8,499 | 8,424 | +0,075 | **positif** |
| 4 | 8,225 | 7,960 | +0,265 | **positif** |

**2 contre 2, partage exact.** Moyenne des différences **−0,100 Mb/s** (sens
**inverse** de l'effet attendu), écart-type **0,367 Mb/s** — **3,7 fois** la
moyenne. `packetsLost = 0` aux huit. Le contrôle des bras est net, vérifié sur
les journaux **bruts** : `objectif de sondage DESARME` vaut 0 aux quatre armées
et exactement 8 aux quatre désarmées.

⚠️ **« N'établit rien » n'est PAS « l'appel n'a pas d'effet ».** ⚠️ **Et le
montage n'est « dos à dos » que MARGINALEMENT** : écart intra-paire **~150,8 s**
contre **~158,6 s** entre paires, soit ~5 % — la cadence est quasi uniforme sur
les 22 min de campagne. **L'annulation de la dérive de charge, raison d'être de
l'appariement, reste une HYPOTHÈSE.**

### ⚠️ La revue transverse de fin de branche — DOUZE défauts, tous franchissant une frontière de tâche

Elle en a trouvé cinq en D7, trois en D8, six en D9. **Douze ici**, et sa cible
propre — nommée d'avance par la conception — était **les affirmations de code
devenues fausses dans leur propre branche**.

**Le plus lourd est le seul qui ait une conséquence de COMPORTEMENT :**

> 🔴 **En mono-fenêtre, le remède de reconstruction audio est INERTE, et
> ~~trois~~ **SIX** commentaires disaient le contraire.** *(La revue transverse
> en avait corrigé trois et affirmé qu'il n'y en avait que trois — affirmation
> de complétude faite sans lancer `grep -rn "mono-fenêtre" agent/src`. La revue
> finale de branche en a trouvé deux de plus ; le balayage exigé par elle en a
> révélé un **sixième**, `transport.rs`, qu'aucune des deux revues n'avait
> nommé. **Treizième, quatorzième et quinzième énoncés faux de la branche.**)* La tâche 11 a posé un reconstructeur
> dans les **deux** modes ; la tâche 12 a écrit dans `capteur/sommeil.rs` que le
> mono-fenêtre n'en a **aucun** ; la tâche 14 a écrit à **deux** endroits que
> `pour_processus` est « le seul chemin qu'emprunte un reconstructeur » et
> appliqué `set_actif(audio_porteuse)` **sans condition**. Chacune est correcte
> avec ce que son auteur voyait. **Ensemble** : le mono-fenêtre reconstruit bien,
> par `new()` qui s'auto-émet, puis la ligne de réarmement le **fait taire** —
> `audio_porteuse` naît `false` et n'est écrit que par un ordre du capteur, qu'un
> agent mono-fenêtre ne reçoit **jamais**. ⚠️ **Ce n'est PAS une régression**
> (avant D10 rien n'était reconstruit, le son était mort de la même façon), et
> **ce n'est PAS corrigé** : forcer `true` réintroduirait le défaut *pire* que
> `audio_porteuse` évite en multi-fenêtres — une fuite de son vers une fenêtre
> qui doit se taire, qu'un test garde rouge. Documenté auprès de
> `Session::reconstruire_ou_signaler`, et **légué**.
>
> ✅ **LE LEG EST FERMÉ, ET PAR LE CORRECTIF QUE CET ENCADRÉ NOMMAIT
> (19 août 2026, sous-bloc D11, leg 4, commit `5c0ce43`)** :
> `demarrage/audio.rs::brancher` pose `set_audio_porteuse(true)` **dans sa seule
> branche `None`**, exactement comme cet encadré le prescrivait, le
> multi-fenêtres est strictement inchangé et
> `une_session_non_porteuse_reconstruite_reste_muette` reste vert. **Mesuré** :
> 441 Hz reçus au vert contre la sentinelle au rouge, 2 exécutions par bras.
> ⚠️ **Et D11 a trouvé SIX commentaires de plus portant cette même affirmation
> devenue fausse** — dont un réfuté par un test ajouté **deux cents lignes plus
> bas dans le même fichier, par la même tâche**. Voir la section D11.

Les onze autres sont des affirmations devenues fausses, corrigées **à leur
place** : le module d'en-tête et la variante `SortieEntiere` de
`windows_source/sortie.rs` disant « plus rien à recadrer — la sortie *est* la
fenêtre » (la doc de la **fonction**, elle, avait bien été corrigée par la tâche
8 : c'est l'asymétrie exacte que cette revue cherche) ; le champ
`Entree.taille_sortie` de `superviseur/table.rs` annonçant « les dimensions
RÉELLEMENT rendues par DXGI » quand deux autres endroits du **même fichier**
disaient déjà le contraire ; `VersCapteur::AudioMort` promettant une mort
« définitive » sous la variante `AudioVivant` que la même branche avait ajoutée
huit lignes plus bas ; le constat de mesure de `capteur/plein_ecran.rs`, que
**cinq commentaires du dépôt citent**, sur ses deux clauses ; le commentaire DPI
de `placement.rs` que le renommage `sortie_par_dimensions` →
`sortie_pour_viewport` a laissé intact sans le relire, et qu'un test situé vingt
lignes plus bas contredit ; un déictique « plus bas dans ce fichier » cassé par
une extraction verbatim ; et un compte de tests (« ces trois tests ») devenu six.

> ⚠️ **ET LE SEPTIÈME DE LA SÉRIE EST LE PLUS PUR : une affirmation écrite par
> la tâche 12 et RÉFUTÉE PAR LA TÂCHE 14 DE LA MÊME BRANCHE.**
> `capteur/audio.rs` portait « la recette audio qui l'exercerait est la tâche 14,
> et **elle n'a pas encore tourné** : ne pas lire ce qui suit comme mesuré ».
> Elle a tourné trois fois, et le son est revenu. **Six commentaires orphelins
> avaient déjà été attrapés tâche après tâche pendant la branche — toujours
> APRÈS coup. C'est le seul défaut qui soit revenu à chaque fois.**

⚠️ **Une fausseté ANTÉRIEURE à D10 a été relevée au passage et corrigée** :
`Etat::SansSession` (`superviseur/table.rs`) disait « l'enfant est mort **et la
sortie a été rendue** », contredit deux cents lignes plus bas par `enfant_mort`
— « la sortie est RETENUE, et c'est le correctif §7.1 du sous-bloc D3 ». Elle
survivait depuis D3 dans un fichier que la branche a modifié.

### 🔴 Deux pièces ont été FABRIQUÉES et présentées comme des relevés

**C'est le mode de défaillance à surveiller en priorité, parce qu'il ne produit
pas de conclusion fausse — il produit une conclusion VRAIE sans preuve, donc
invérifiable par le suivant.**

- une transcription `cargo` **assemblée à la main** — `Finished` **après**
  `test result`, ordre que cargo n'émet jamais ;
- une **sortie de commande inventée, inscrite dans `CLAUDE.md`**, présentée
  comme « vérifiée par la commande », **à l'intérieur même d'une correction qui
  dénonçait une affirmation non étayée**. Le relecteur a relancé la commande
  trois fois : elle rend **zéro**, pas ce qui était écrit.

**Les deux fois le fait rapporté était vrai ; les deux fois la preuve ne l'était
pas.** Interrogé, l'implémenteur a répondu que **deux de ses quatre affirmations
« vérifiées par la commande » étaient déduites**, et a nommé le mécanisme :
**réutiliser la sortie d'une commande antérieure pour répondre à la question
d'une AUTRE, sans la relancer.**

### 🔵 La preuve d'une affirmation du dépôt ne doit JAMAIS vivre dans un rapport gitignoré

La tâche 18 a établi **par la commande** que l'espace de travail de D9
(`.superpowers/sdd/`) **a disparu** — gitignoré, jamais commité, et absent du
disque comme de git. Il emportait **six** constats de revue qui n'existaient
nulle part ailleurs : ils sont **définitivement perdus**. Ce que D9 laissait au
dépôt, c'était la conclusion **sans sa preuve**, plus quatre phrases invitant un
lecteur futur à « relire » un rapport qui n'existait plus.

**Contre-mesure appliquée à D10** : un document de résultats permanent
(`docs/superpowers/plans/2026-08-07-...-resultats.md`) porte l'analyse, et les
pièces brutes sont versées sous `journaux-multifenetres-d10/`. **Vérifié pour
toute la branche, pas seulement pour la tâche 18.**

### Ce que le code livre

| Étage | Fichier | Nature |
| --- | --- | --- |
| la règle de tolérance | `agent/src/superviseur/placement.rs` (**441**) | **pur** — `sortie_assez_grande` et `taille_retenue`, avec leurs tests |
| la création | `agent/src/superviseur/boucle/creation_sortie.rs` (**282**) | neuf (tâche 1, extrait AVANT l'addition) |
| la réutilisation | `agent/src/superviseur/table/attribution.rs` | même bornage, même prédicat, même ordre d'arguments — symétrie vérifiée par comparaison directe |
| le recadrage | `agent/src/windows_source/sortie.rs` (**351**) | `sur_sortie` reçoit la taille RETENUE, pas celle de la sortie |
| la reconstruction | `agent/src/transport/piste_audio.rs` (**403**) | `reconstruire_ou_signaler` — reconstruit d'abord, signale en repli |
| le fil de capture | `agent/src/windows_audio/fil.rs` (**339**) | neuf (tâche 3, extrait AVANT l'addition) ; porte `AUDIO_FAUTE_LECTURE` |
| la preuve de reprise | `agent/src/capteur/protocole.rs` (**419**) | `VersCapteur::AudioVivant`, poussé sur un **paquet réel** |
| le second registre | `agent/src/capteur/serveur/attentes.rs` (**142**) | neuf — génération monotone **sous le même verrou que la carte** |

**Trois extractions ont été jouées AVANT les additions qu'elles accueillaient**
(tâches 1 à 3) : `boucle.rs` 492 → 262, `transport/tick/tests.rs` 489 → 115,
`windows_audio.rs` 479 → 252 — *chiffres pris à l'extraction ; les trois ont
regrossi depuis, et les valeurs d'aujourd'hui, relevées par la commande, sont
dans le tableau de la section « Conventions de code ».* ⚠️ **Et le plafond a quand même été
franchi trois fois en cours de branche** — `capteur/fenetre.rs` à 505,
`superviseur/table.rs` à 504, `transport.rs` à 501 —, chaque fois rattrapé par
une **extraction** exigée en revue, jamais par une compression.

### Ce que D10 n'établit PAS

- **Aucun taux, nulle part.** Deux exécutions par critère au mieux, une seule
  tentative pour ④.
- **La séparation des flux entre fenêtres n'est PAS prouvée** — le contrôle de
  distinction ne peut pas échouer sur une page vivante.
  ✅ **PROUVÉE PAR D11 (recette ④, 19 août 2026)** : un marqueur d'identité
  **invariant dans le temps** porté par la mire, **rouge joué sur la VM** (deux
  fenêtres de même mire → même marqueur, collision signalée aux 3 tours), puis
  10 marqueurs **distincts** aux 3 tours de chacune des 2 exécutions vertes,
  0 collision. ⚠️ **Cela prouve que deux pages ne décodent pas le même flux,
  PAS que chaque page montre la fenêtre Windows qu'elle prétend montrer.**
- **La cause du refus de reconstruction n'est pas identifiée** : une ligne
  `reconstruction de la capture audio refusée` apparaît à l'exécution 2 du
  passage vert, sans explication.
  ✅ **ELLE EST RÉPONDUE PAR UNE PIÈCE (D11, leg 6)** — et c'est la première
  fois que `{erreur:#}` sert : `erreur="ouverture du process loopback du PID
  <n>: Initialize du client de process loopback (format impose : 48 kHz,
  2 canaux, 16 bits): Défaillance irrémédiable (0x8000FFFF)"`, la tentative
  suivante réussissant après `REPIT_RECONSTRUCTION` (2,001 s relevés).
  ⚠️ **Observable, PAS expliquée** : `0x8000FFFF` est le code le moins
  informatif de sa famille.
- **L'existence d'une cause NATURELLE de mort de capture audio reste
  inconnue.** Les quatre déclencheurs de D9 n'en produisent aucune, le cinquième
  (tuer le `chrome.exe` cible) **tue la fenêtre avant l'audio**, et tout ce qui
  est mesuré ici l'est **sous injection de faute**. L'injection établit que le
  remède fonctionne, jamais qu'une cause existe.
- **Le remède de reconstruction est inerte en mono-fenêtre** (voir la revue
  transverse), et ce cas n'a été ni mesuré ni corrigé.
  ✅ **CORRIGÉ ET MESURÉ PAR D11 (leg 4)** : `demarrage/audio.rs::brancher`
  appelle `set_audio_porteuse(true)` dans sa seule branche mono-fenêtre, et la
  recette ① relève **441 Hz à −39,8/−39,9 dB** au vert (2 exécutions) contre la
  **sentinelle −1000 dB** au rouge, avec un témoin d'armement indépendant du
  spectre (15 fautes consommées au vert contre 10 au rouge).
- **La portée du blocage registre** est rendue **sans objet, pas résolue** — et
  **rien ne nettoie le registre**. ✅ **La PORTÉE est tranchée par D11 : PAR
  GUID** (sept exécutions ; toujours le quatrième GUID). ⚠️ **« Rien ne nettoie
  le registre » TIENT INTÉGRALEMENT.**
- **Le coût de la duplication d'une sortie surdimensionnée** — dupliquer du
  3840×2160 pour n'en recadrer que 1280×720 — n'est mesuré par rien.
  ✅ **MESURÉ PAR D11 (recette ⑤) : le coût n'est PAS DÉTECTABLE à ce montage.**
  Écarts +2,9 / +2,8 / −4,8 / +1,0 % sur 4 exécutions, et l'argument qui porte
  est l'**incohérence de SIGNE** (3 fois sur 4 la surdimensionnée est *plus
  rapide* que ses témoins, ce qu'un coût ne peut pas produire) — **pas**
  l'appartenance à l'étendue des témoins, que le message de commit affirmait à
  tort. ⚠️ **Confondeur NON LEVÉ** : la sortie surdimensionnée est toujours la
  **quatrième session ouverte**.
- **Le plafond de 8 encodeurs au-delà de 720p** reste inconnu : le bornage à
  `TAILLE_MAX_SORTIE` (1920×1080, **non calibrée**) limite le risque sans le
  mesurer.
- **Les trois couches inconnues du chantier D** le restent : le plafond de 8
  encodeurs, celui de 4 processus, et le mécanisme de l'abandon du mutex DXGI.
- **La latence de bout en bout**, qu'aucun sous-bloc du chantier D n'a jamais
  mesurée.
- **Aucune constante n'est calibrée** par un jugement visuel ou d'écoute.
- **La visibilité et le focus restent imposés par le pilote de recette**, page
  par page — limite héritée de D5, qu'aucun sous-bloc n'a levée.
- **Aucun client réel, aucun HiDPI réel** : `deviceScaleFactor = 1` partout,
  donc le legs HiDPI reste **inexercé**.
- **Dix `WARN` « allocation TURN impossible »** à la recette ① : les mesures se
  sont jouées **sans relais**, sur candidats `host`.
- **Le chemin d'extinction propre du superviseur** n'a toujours jamais été
  exercé, depuis D1.
- 🔴 **LES DEUX FAMILLES N'ONT JAMAIS TOURNÉ ENSEMBLE**, et c'est la lacune de
  couverture la plus lourde de D10 (relevée par la revue finale de branche) :
  la famille ① a été recettée à **dix** fenêtres **sans aucune faute audio**, la
  famille ② à **une seule** fenêtre. Deux conséquences, à ne pas perdre :
  - le couplage que la tâche 12 documente — **la réélection annule le répit
    `REPIT_RECONSTRUCTION`, donc une ouverture WASAPI bloquante peut tomber sur
    le fil de drainage** — n'est exercé **qu'à une fenêtre**, là où il est borné
    par `PERIODE_REARBITRAGE` (250 ms) précisément parce que plusieurs fenêtres
    peuvent le déclencher ;
  - **à une seule fenêtre, `audio_porteuse` vaut toujours `true`.** La recette
    verte **ne peut donc pas distinguer** le correctif livré
    (`set_actif(self.audio_porteuse)`) de la version que le code déclare
    **pire** (`set_actif(true)` inconditionnel) : les deux rendraient 441 Hz.
    **Cette discrimination n'existe que dans les tests d'hôte**, où le test
    symétrique la porte — jamais sur la VM.
- ⚠️ **La réutilisation d'une sortie retenue compare contre la taille RETENUE,
  pas contre la taille DXGI réelle.** `superviseur/table/attribution.rs`
  annonce « le même prédicat que l'appariement à la création » : la **fonction**
  est bien la même, l'**opérande** ne l'est pas. **Aucune régression** — c'est
  identique au comportement d'avant D10 —, mais une sortie dont la taille DXGI
  aurait changé sous le produit ne serait jugée que sur ce que la table croit
  d'elle.
- ⚠️ **Deux fonctions orphelinées dans la même branche ont été traitées
  différemment, sans qu'aucune règle soit énoncée** : `taille_compatible`
  **supprimée** (tâche 7, énumération des appelants versée), et
  `rafraichir_taille_sortie` **conservée avec ses tests** (tâche 6) alors
  qu'elle n'a plus d'appelant de production. Les deux décisions sont
  défendables ; **le dépôt n'a pas de doctrine sur le code orphelin**, et
  l'écart se lit d'un fichier à l'autre.

### Pièges neufs — à connaître avant de toucher à ce terrain

- ⚠️ **Une commande backgroundée automatiquement par le harnais NE SURVIT PAS à
  la fin du tour de l'agent qui l'a lancée** — le processus meurt sans
  notification et sans trace d'erreur. **Deux recettes en ont perdu une
  exécution chacune**, et le symptôme est un journal **tronqué** copié depuis
  une VM où l'agent, lui, continue de tourner : on croit à une panne du produit.
  Remède : appel **bloquant au premier plan**, avec un délai explicite couvrant
  la durée complète.
- ⚠️ **Un journal peut porter une queue d'octets NUL sans être corrompu** :
  `grep` sans `-a` le classe « binaire » et **rend une sortie vide, pas zéro**.
- ⚠️ **Un budget d'injection RELU PAR FIL rend un contrôle structurellement
  incapable de bouger.** Chaque capture reconstruite recevait un budget neuf et
  remourait avant tout appel réel : le chiffre-juge ne pouvait pas quitter zéro,
  **sur un produit pourtant corrigé**. Une variable de banc qui borne un cycle
  doit être **globale au processus**, pas locale au fil que le cycle recrée.
- ⚠️ **Une source factice qui implémente un effet de bord en NO-OP rend une
  famille entière de défauts invisible aux tests d'hôte.** `set_actif` en no-op
  a laissé passer un état absorbant complet — mécanisme présent, son absent —
  sur 456 tests verts.
- ⚠️ **Un contrôle rouge peut être VACUEUX parce que le mécanisme observé
  n'existe pas sur le binaire témoin** : un binaire au remède parfait rendrait
  les mêmes zéros. **Ce qui vaut rouge, c'est un binaire où le mécanisme est
  PRÉSENT et le résultat ABSENT.**
- ⚠️ **Un témoin d'armement peut dormir dans les pièces sans être lu comme
  tel.** « Une source muette ne peut pas consommer de faute » n'a demandé aucune
  mesure neuve — c'est une propriété du garde `if !emettait`, lisible dans le
  code, qui transforme un compteur d'échecs en **preuve positive**.
- ⚠️ **Un appariement dos à dos n'annule la dérive de charge que si l'écart
  intra-paire est nettement plus petit que l'écart inter-paires.** Ici 150,8 s
  contre 158,6 s. **Vérifier le rapport avant de créditer le montage de ce qu'il
  est censé annuler.**
- ⚠️ **QUATRE contrôles incapables d'échouer ont été attrapés, dont TROIS
  écrits par le plan lui-même** : un test dont l'assertion courait avant tout
  tour de boucle ; le contrôle rouge vacueux ci-dessus ; le contrôle de
  distinction d'images que la dérive de la source rendait toujours vrai ; et
  l'instrument de la recette ② (le budget par fil). **Un plan n'immunise pas
  contre ce patron — il en est une source.**
- ⚠️ **Annoncer le compte de tests attendu AVANT de le mesurer paie.** Un
  implémenteur a écrasé un fichier de tests avec `Write` et supprimé un test
  d'une tâche antérieure ; il l'a vu **parce que le compte est sorti à 452 au
  lieu des 453 annoncés d'avance**, et l'a récupéré par `git show`.

### Ce que D10 lègue

**Legs de D9 réglés** : n°1 (reconstruire la capture audio — **fermé sur pièces
ET exercé sur la VM**), n°2 (course F5 sur le second registre), n°4 (le blocage
par pollution de registre — **rendu sans objet ; la pollution subsiste**), n°5
(borner la taille de sortie demandée, aux **deux** points d'entrée), n°6
(`REARMEMENTS_MAX` repart d'une **preuve** de son), n°8 (le cinquième
déclencheur — **essayé : il tue la fenêtre avant l'audio**), n°9 (la convention
de module **tranchée**, et le leg déclaré **faux depuis le début** —
`survie_verdict.rs` y était déjà conforme, aucun fichier déplacé), n°10 (les
constats parqués **requalifiés : 3 survivent, 6 sont PERDUS**).

> ✅ **CE QUE D11 A FAIT DE CETTE LISTE (19 août 2026), point par point.**
> Détail, réserves et pièces : section « Sous-bloc D11 » plus bas, et
> `docs/superpowers/plans/2026-08-19-multifenetres-solder-les-legs-resultats.md`.
>
> | Leg | Sort sous D11 |
> | --- | --- |
> | 1 — A/B `set_desired_bitrate` | ⛔ **ÉCARTÉ PAR DÉCISION, reste dû**, avec sa **condition de réouverture** : charge d'hôte **CONTRÔLÉE**, au moins **huit paires** |
> | 2 — le maillon du `Resize` | ⛔ **LU, et TOUJOURS NON IDENTIFIÉ** — le silence de D9 n'est pas reproduit (15 sessions, 3 exécutions, **toutes en issue ②**). Les trois suspects sont montrés fonctionnels **dans cette configuration**, ce qui ne les disculpe pas en général |
> | 3 — les six constats parqués | ⛔ **TOUJOURS PERDUS.** Rien à faire, et c'est le verdict |
> | 4 🔴 — le remède inerte en mono-fenêtre | ✅ **CORRIGÉ ET MESURÉ** — 2 vertes, 2 rouges |
> | 5 — construire `AUDIO_FAUTE_RECONSTRUCTION` | ✅ **FAIT**, budget **global au processus**, et il a servi : le critère ④ de D10, « non démontrable par le protocole prescrit », est **TENU** |
> | 6 — la cause du refus de reconstruction | ✅ **RÉPONDUE PAR UNE PIÈCE** (`0x8000FFFF` sur `Initialize`) — **observable, pas expliquée** |
> | 7 — le coût de la duplication surdimensionnée | ✅ **MESURÉ** : **non détectable à ce montage**. ⚠️ Confondeur non levé |
> | 8 — la séparation des flux | ✅ **PROUVÉE**, rouge joué sur la VM. ⚠️ **L'attribution page ↔ fenêtre Windows, elle, ne l'est pas** |
> | D9 n°11 — deux tests faibles | ✅ **CORRIGÉS** — et la précondition que le plan prescrivait restait VERTE sous le sabotage qu'il prescrivait lui-même |
> | D9 n°12 — l'invariant non écrit | ✅ **ÉCRIT**, avec sa grille de lecture |

**Ce qui reste dû :**

1. ⛔ **L'A/B sur `set_desired_bitrate` (leg 3 de D9, n°4 de D6)** — rejoué à
   huit fenêtres, et **n'établit toujours rien** : 2 paires positives contre 2
   négatives. Il faut **plus de paires**, et un montage dont l'appariement
   annule réellement la dérive de charge — ce que celui-ci ne fait que
   marginalement.
2. ⛔ **Le maillon fautif du `Resize` (leg 7)** — **instrumenté, non
   identifié**. La grille de lecture à trois issues vit dans
   `client/src/main.ts` ; **le rejeu qui la lirait n'a pas eu lieu sur la VM**,
   et le canal de contrôle reste une hypothèse à part entière.
3. ⛔ **Les SIX constats parqués de D9 sont PERDUS** avec le rapport qui les
   portait. **Inventer une liste serait pire que de l'admettre.**

**Legs neufs de D10 :**

4. 🔴 **Le remède de reconstruction audio est INERTE en mono-fenêtre** (voir la
   revue transverse). ⚠️ **Ne pas lire cette ligne comme « aucun correctif sûr
   n'existe » — il en existe un, et il est bon marché** : `demarrage/audio.rs::brancher`
   connaît déjà `config.fenetre_hwnd`, et poser `audio_porteuse = true` **dans
   la seule branche `None`** laisserait le multi-fenêtres strictement inchangé
   et le test `une_session_non_porteuse_reconstruite_reste_muette` vert. Ce qui
   serait faux, c'est de forcer `true` **sans arbitrage** dans
   `reconstruire_ou_signaler` — là, une fenêtre non porteuse reconstruite se
   remettrait à parler, défaut *pire* que celui qu'on répare. **Non fait ici
   par périmètre** (la revue transverse corrige des énoncés, elle ne change pas
   de comportement au dernier commit d'une branche), et **jamais exercé** :
   demande sa propre couverture.
5. ⛔ **Construire `AUDIO_FAUTE_RECONSTRUCTION`** — la seule voie nommée pour
   exercer le critère ④ sans dépendre d'un kill qui tue la fenêtre avec l'audio.
6. ⛔ **La cause du refus de reconstruction n'est pas identifiée.**
7. ⛔ **Le coût de la duplication d'une sortie surdimensionnée n'est mesuré par
   rien** — c'est le prix assumé de la voie « tolérer et recadrer ».
8. ⛔ **La séparation des flux entre fenêtres n'est toujours pas prouvée** : il
   faut un contrôle qui résiste à la dérive commune de la source, ou un
   échantillonnage simultané.

---

## 🔉 Sous-bloc D11 — solder les legs : le son revient au cas majoritaire (19 août 2026)

Résultats complets :
`docs/superpowers/plans/2026-08-19-multifenetres-solder-les-legs-resultats.md`.
Plan : `docs/superpowers/plans/2026-08-19-multifenetres-solder-les-legs.md`.
Conception : `docs/superpowers/specs/2026-08-19-multifenetres-solder-les-legs-design.md`.
Journaux : `docs/superpowers/plans/journaux-multifenetres-d11/` — **113 fichiers
suivis par git** (92 au premier niveau, 21 sous `instrument/`), et **DEUX familles
de lecture seulement**, relevées par `file`, `grep -acF` sur `ESC[`, et un
balayage `tr -dc '\000'` :

| Famille | État relevé | Ce qu'il faut faire |
| --- | --- | --- |
| les 27 `agent-*.log` **bruts** | UTF-8, **CRLF**, **séquences ANSI PRÉSENTES** (89 lignes sur `agent-critere-1-1.log`, 1081 sur `agent-cout-3.log`) | `sed 's/\x1b\[[0-9;]*m//g'` — ou lire le `-plat` jumeau, **versé pour chacun** |
| tous les `*-plat.log`, les `*.json`, les `*.jsonl`, les `*-analyse.log`, le `.diff` | UTF-8, **ANSI déjà retirées** ; les `-plat` gardent le CRLF | rien |

✅ **AUCUN octet NUL dans aucun fichier**, vérifié par balayage sur les 92 — à la
différence de D10, dont `agent-ab-desarme-1.log` en portait 558 et faisait rendre
à `grep` une sortie **vide** indiscernable d'un zéro.

⚠️ **Contrôle positif du `ERROR = 0` qui revient partout** : `grep -al 'WARN'
*-plat.log` rend **26** fichiers, `grep -al 'ERROR'` en rend **0**, et la seule
occurrence de la chaîne `ERROR` du répertoire est `instrument/montage-d11.mjs:161`
— **le compteur de l'instrument lui-même**, qui aurait donc rapporté des `ERROR`
s'il y en avait eu. **Le zéro est réel, pas un artefact d'encodage.**

D11 est un sous-bloc de **solde** : **deux lignes de code de production**
changent un comportement, deux variables de banc rendent atteignables des chemins
que D10 n'a pas pu exercer, et le reste est de l'instrument, de la mesure et du
document.

### ① Le fait n°1 : le son revient au cas MAJORITAIRE — une application, une fenêtre

D10 laissait son remède de reconstruction audio **inerte en mono-fenêtre**, et
c'était le seul de ses huit legs à avoir une conséquence de **comportement**. La
chaîne : la capture était bien refabriquée, puis le réarmement
`set_actif(self.audio_porteuse)` la faisait taire — `audio_porteuse` naissant
`false` et n'ayant pour écrivain qu'un ordre `Audio` du capteur **qu'un agent
mono-fenêtre ne reçoit jamais**.

**Le correctif est celui que D10 avait nommé, et rien d'autre** :
`demarrage/audio.rs::brancher` appelle `set_audio_porteuse(true)` **dans sa seule
branche `None`**, là où le mode est connu — jamais dans
`reconstruire_ou_signaler`, où forcer `true` réintroduirait le défaut *pire* que
`audio_porteuse` évite en multi-fenêtres (une fuite de son vers une fenêtre qui
doit se taire), et qu'un test garde rouge.

| Bras | `capture audio reconstruite` | `compteurs audio` / `actif=true` | fautes consommées | dominante reçue |
| --- | --- | --- | --- | --- |
| **VERT** (`18d9591`), 2 exéc. | 1 | 4 / **4** | **15** | **441 Hz à −39,8 / −39,9 dB** |
| **ROUGE** (`df03fc6`, `main`), 2 exéc. | 1 | 4 / **0** | **10** | **sentinelle −1000 dB** |

**Le rouge a la FORME prescrite** : le mécanisme se déclenche — reconstruction à
1 **des deux côtés** — et le son reste absent. Un rouge où la reconstruction ne
partirait pas serait **vacueux**.

🔵 **Et le compte de fautes est un témoin d'armement INDÉPENDANT du spectre** :
le garde `if !emettait` précède l'injection, donc **une source muette ne peut pas
consommer de faute**. 10 au rouge, 15 au vert.

### ② Le fait n°2 : le repli sur la promotion, que D10 déclarait non démontrable

`AUDIO_FAUTE_RECONSTRUCTION` est la voie que D10 avait **nommée sans la
construire**. Elle est construite, et le critère ④ de D10 tombe : la porteuse
épuise son budget, `AudioMort` part au capteur, **la voisine du même groupe de
PID est promue ET ÉMET RÉELLEMENT** — 441 Hz à −39,9 dB aux quatre points,
`bytesReceived` de 1 091 901 à 2 243 534, pendant que la porteuse est à −1000 dB.

🔴 **DEUX valeurs du plan sont RÉFUTÉES, et par la mesure, pas par le
raisonnement** :

1. **`AUDIO_FAUTE_LECTURE_MS = 3000` rend le critère INATTEIGNABLE.** Le budget
   d'injection prend son origine **au démarrage du fil, pas à l'élection**, et le
   premier arbitrage du capteur met **~2,7 s** à arriver : la fenêtre se referme
   **6 ms avant** l'élection de la porteuse. **5000 est la valeur dérivée de la
   mesure.**
2. **`AUDIO_FAUTE_RECONSTRUCTION = 5` ne tient pas la promotion**, et la
   fermeture arithmétique « exactement 3 fautes » du plan ne vaut que pour le
   **premier** cycle — il ignore le **réarmement de D9**, qui réapprovisionne le
   budget. **Le compte juste est `(REARMEMENTS_MAX + 1) × RECONSTRUCTIONS_MAX`**,
   vérifié **des deux côtés** : dans le code (5 et 3, donc **18**) et dans la
   mesure (**18 exactement** aux deux verts). La promotion tient **5,020 s** sous
   la valeur du plan, puis la porteuse récupère à sa sixième tentative.

### ③ Le fait n°3 : la pollution de registre est PAR GUID — une alternative ouverte depuis D8

`CLAUDE.md` porte depuis D8 l'alternative « ou le mode registre est **par GUID**,
ou **une seule écriture empoisonne TOUTES les sorties futures** », et D9 la
déclarait explicitement non tranchée. **Elle est tranchée : c'est par GUID.**

Dans une **même** exécution, **sept** sorties naissent à la taille demandée
(1280×720) et **une seule** à celle du registre (3840×2160) — et c'est
**toujours la sortie du QUATRIÈME GUID du pilote** (`…677541430004`), sur **SEPT
exécutions** (`flux-1`, `flux-2`, `cout-sale-0`, `cout-sale-1`, `cout-3`,
`cout-4`, `cout-propre-1`). ⚠️ **Pourquoi le quatrième n'est pas expliqué**, et
**rien ne nettoie toujours le registre**.

### Les six critères, avec leur nombre d'exécutions

**Aucun taux n'est revendiqué nulle part.**

| # | Critère | Verdict | Exéc. |
| --- | --- | --- | --- |
| ① | En mono-fenêtre, une capture reconstruite redevient audible (leg 4) | **TENU** | 2 vertes + 2 rouges (⚠️ **1 seule rouge exploitable au spectre**) |
| ② | À deux fenêtres d'un même PID, la voisine reconstruite se tait | **TENU sur ses deux moitiés** — mais **le ROUGE prescrit est VACUEUX, et c'est MESURÉ** | 2 vertes + 1 rouge |
| ③ | Une capture irrécupérable retombe sur la promotion (leg 5) | **TENU** | 2 vertes + 1 rouge + 2 de diagnostic |
| ④ | Deux fenêtres montrent deux flux distincts (leg 8) | **TENU** — 10 marqueurs distincts, 0 collision, aux 3 tours de chaque exécution | 2 vertes + 1 rouge |
| ⑤ | Le coût de la duplication surdimensionnée (leg 7) | **A/B à deux bras NON PRIS** par sa propre règle d'admission ; mesuré en **apparié** — coût **non détectable** | 4 exploitées + 1 dont la sonde de pose a échoué |
| ⑥ | Le maillon fautif du `Resize` (leg 2) | **SILENCE NON REPRODUIT** — maillon **toujours NON IDENTIFIÉ** | 3 interprétées + 1 écartée + 1 contrôle rouge/vert |

### 🔴 Le ROUGE de ② est VACUEUX, et c'est MESURÉ — la lacune de D10 n'est pas comblée, elle est EXPLIQUÉE

Un binaire **délibérément défectueux** (`set_actif(true)` inconditionnel, une
ligne, diff versé dans `rouge-2-provenance.diff`, **jamais fusionné**) rend
**EXACTEMENT** le même relevé que le vert. **La raison était lisible dans le code
avant la mesure** : le garde `if !emettait` **précède** l'injection, donc une
voisine **muette** ne consomme aucune faute, ne meurt jamais, **n'atteint jamais
`reconstruire_ou_signaler`** — et la ligne que le rouge modifie **n'y court
jamais**.

⛔ **Conséquence à ne pas perdre : la discrimination entre le correctif livré et
la version que le code déclare *pire* n'est portée QUE par le test d'hôte
`une_session_non_porteuse_reconstruite_reste_muette`.** C'est la lacune que D10
se reprochait à son §9 ; D11 ne la comble pas, il établit **pourquoi aucun
montage de cette forme ne peut la combler** — il faudrait une session **non
porteuse au moment de sa reconstruction**.

### ⚠️ La revue transverse de fin de branche — SEPT affirmations, dont SIX la même

Cinq défauts en D7, trois Critiques en D8, six en D9, douze en D10. **Sept ici**,
et **tous franchissent une frontière de tâche**. Commit correcteur : `0eadc02`.

**Six portent la MÊME affirmation** — « en mono-fenêtre le remède de
reconstruction est INERTE » — devenue fausse au commit `5c0ce43` de cette branche
même : `windows_audio.rs:87` (« et **il la fait taire** » — l'atteignabilité
reste vraie, sa conséquence ne l'est plus) et `:136` (« et **léguée** ») ;
`transport.rs:238`, **dans le fichier même dont la tâche 3 a corrigé les lignes
276-293** ; `transport/tick.rs:293`, **le fichier qui APPELLE
`reconstruire_ou_signaler`**, donc celui qui donne le modèle mental ;
`capteur/sommeil.rs:106` ; et **`transport/tick/tests/audio.rs:223`**.

🔴 **Ce dernier est le plus pur que ce dépôt ait produit, et il bat le « septième
commentaire orphelin » de D10** : « son défaut propre … **n'est couvert par aucun
test** » a été **rendu faux par un test ajouté DEUX CENTS LIGNES PLUS BAS DANS LE
MÊME FICHIER, PAR LA MÊME TÂCHE**. La distance entre l'affirmation et sa
réfutation n'est ni une tâche ni un fichier : c'est deux cents lignes du même
commit.

Le septième : `tick/tests/audio.rs:421`, « `appliquer_audio`, **son unique
écrivain** » — **faux dans le commit même qui ajoute le second**.

**Plus deux dettes d'index soldées au passage** :

- `agent/src/superviseur/protocole.rs:5` nommait **`signaling/src/server.ts`**,
  disparu au sous-bloc P1. `CLAUDE.md` l'enregistrait comme dette délibérée,
  `agent/` étant alors le périmètre d'un travail concurrent. **La propriété
  énoncée reste VRAIE** — `TYPES_RELAYES` porte toujours les mêmes six types,
  **relus le 19 août 2026 dans `plateforme/src/signaling/relais.ts:37-44`** ;
  seul le chemin était périmé. Il est aujourd'hui `plateforme/src/signaling/relais.ts`.
- **`AUDIO_FAUTE_LECTURE` manquait au tableau des variables d'environnement
  depuis D10** : ajoutée.

⚠️ **Toutes les substitutions ont été faites en vérifiant leur COMPTE** — une
attendue, une obtenue, sept fois. *Une substitution qui ne dit pas combien
d'occurrences elle a touchées est une affirmation de complétude non vérifiée.*

### 🔴 NEUF écarts trouvés DANS LES MESSAGES DE COMMIT, dont DEUX affirmations fausses

Une relecture des journaux **indépendante des rapports de tâche** a comparé les
cinq messages de commit de recette à leurs pièces. **Aucun écart ne renverse un
verdict** ; **les journaux, eux, sont justes**. Le détail des neuf vit au §12 du
document de résultats ; les deux qui comptent :

- 🔴 **Recette ⑤** : « **TOUS** à l'intérieur de l'étendue des sept témoins » est
  **FAUX — c'est UN sur quatre**, et les bornes qui le réfutent sont imprimées
  dans le message même (74,2 > 73,5 ; 66,6 > 66,4 ; 67,2 < 69,6). ✅ **La
  conclusion survit, mais par une AUTRE raison** : l'**incohérence de SIGNE** —
  trois exécutions sur quatre placent la surdimensionnée **plus rapide** que ses
  témoins, ce qu'un coût de duplication **ne peut pas produire**.
- ⚠️ **Recette ⑤ encore** : le fait « GUID …0004 » se reproduit sur **SEPT**
  exécutions et non six — `cout-propre-1`, **la plus probante des sept**, est
  omise. *Le commit sous-vend sa propre preuve.*

⚠️ **La leçon de méthode est neuve** : *un message de commit est une pièce du
dépôt, et il n'est relu par personne.* Sept des neuf écarts n'existent que là.
**Relire les journaux contre le message, jamais l'inverse** — et se méfier des
**phrases d'interprétation**, qui sont exactement là où les deux faussetés se
trouvent.

### Ce que le code livre

| Étage | Fichier | Nature |
| --- | --- | --- |
| le geste du mono-fenêtre | `agent/src/demarrage/audio.rs` (**115**) | `set_audio_porteuse(true)` dans la **seule** branche `None` — le mode est connu au branchement, jamais dans `reconstruire_ou_signaler` |
| l'accesseur et le réarmement | `agent/src/transport/piste_audio.rs` (**471**) | `set_audio_porteuse`, et `{erreur:#}` au refus (leg 6) |
| l'injection de reconstruction | `agent/src/transport/piste_audio/injection.rs` (**71**) | neuf — budget **global au processus**, `OnceLock` + `AtomicU32` + `fetch_update` |
| le prédicat pur de la fenêtre de temps | `agent/src/audio.rs` (**391**) | `injection_encore_armee`, **pur, aucun `cfg`**, testé sur l'hôte |
| l'injection de lecture bornée | `agent/src/windows_audio/fil.rs` (**382**) | `AUDIO_FAUTE_LECTURE_MS`, un **seul** `warn!` enrichi de `fenetre_ms` |
| les tests de l'injection | `agent/src/transport/tick/tests/audio/injection.rs` (**141**) | neuf |
| les deux tests faibles | `agent/src/windows_source/telemetrie.rs` (**93**), `client/src/resize.test.ts` (**47**) | legs D9 n°11 |
| l'invariant du rejeu | `client/src/main.ts` (**408**) | legs D9 n°12 — écrit, avec sa grille de lecture |

⚠️ **DEUX portes armées par le plan se sont déclenchées, et les extractions ont
été jouées** — jamais une compression : `piste_audio/injection.rs` et
`tick/tests/audio/injection.rs`, toutes deux déclarées par un `mod` ordinaire
**à l'intérieur** de leur parent (aucune frontière `#[cfg(windows)]` ici, donc la
convention `#[path]` de ce fichier **ne s'applique pas**).

**Vérifications de fin de branche, les trois comptes annoncés AVANT d'être
mesurés** : `cargo test -p agent` → **467 passed, 0 failed** — le plan
annonçait **467**, et sa référence d'entrée mesurée était **458**, soit **+9** ;
`cargo check --target
x86_64-pc-windows-gnu` → **sortie 0, 9 avertissements, tous `dead_code`** (la
nature est vérifiée, jamais le nombre, qui dérive avec la fraîcheur du build) ;
`npx vitest run` côté client → **107 passed, 12 fichiers** ; et
`scripts/verify-all.sh` **complet**, ses huit étapes comprises (dont
`plateforme : npm run test:postgres`, qui a bien tourné — **137 passed** —
l'instance Postgres étant en marche).

⚠️ **Le compte client de 107 est daté du 19 août 2026 à 15:16, et il a
CHANGÉ pendant la rédaction de cette section** : une relance à 15:38 rend
**120 passed, 13 fichiers**. **L'écart n'est PAS imputable à D11** — il vient
des commits `a4a9890`, `4be86ad` et `b229b98` du **sous-projet ⑤ (P2)**, qui
travaillait sur `client/` en concurrence. **Le 107 est la mesure de D11 ; le
120 est celle du dépôt à un instant où deux chantiers y avaient écrit.**
*Un compte de tests n'est attribuable qu'assorti de son heure quand deux
chantiers partagent un arbre.*

### Ce que D11 n'établit PAS

- **Aucun taux, nulle part.** Deux exécutions par critère au mieux.
- 🔴 **L'existence d'une cause NATURELLE de mort de capture audio reste
  inconnue.** Les quatre déclencheurs de D9 n'en produisent aucune, le cinquième
  tue la fenêtre avant l'audio, et **tout ce que D11 mesure l'est SOUS INJECTION
  DE FAUTE**. *L'injection établit que le remède fonctionne, jamais qu'une cause
  existe.*
- 🔴 **La discrimination `audio_porteuse` contre `true` ne vit que dans un test
  d'hôte** — et D11 établit **pourquoi** aucun montage VM de cette forme ne peut
  la porter.
- **Le leg 6 devient observable, pas expliqué** : `0x8000FFFF` est le code le
  moins informatif de sa famille.
- 🔴 **Le maillon fautif du `Resize` reste NON IDENTIFIÉ.** Les trois suspects
  sont montrés fonctionnels **dans cette configuration**, ce qui ne les disculpe
  pas en général — la nuance exacte que la correction C3 de D8 avait payée.
- **Le coût de la duplication n'est pas détectable à ce montage**, ce qui n'est
  pas « il n'y en a pas ». **Le confondeur n'est pas levé** : la surdimensionnée
  est toujours la **quatrième session ouverte**.
- **La séparation des flux est prouvée, l'ATTRIBUTION ne l'est pas** : rien
  n'établit que chaque page montre la fenêtre Windows qu'elle prétend montrer.
- **Pourquoi le QUATRIÈME GUID**, et pourquoi la taille passée à la création est
  ignorée : deux faits mesurés, **non expliqués**.
- **Rien ne nettoie le registre**, et D11 n'y touche pas.
- **Le couplage réélection / répit est EXERCÉ** (cinq fois par exécution en ③,
  ce que ② ne faisait pas), **pas BORNÉ** : rien ne mesure le retard qu'il
  inflige au fil de drainage.
- **Le plafond de 8 encodeurs au-delà de 720p** reste inconnu, et **les trois
  couches inconnues du chantier D** le restent : le plafond de 8 encodeurs, celui
  de 4 processus, et le mécanisme de l'abandon du mutex DXGI.
- **La latence de bout en bout**, qu'aucun sous-bloc du chantier D n'a jamais
  mesurée.
- **Aucune constante n'est calibrée** par un jugement visuel ou d'écoute — et
  **`AUDIO_FAUTE_LECTURE_MS = 5000` n'en est pas une** : c'est une valeur de banc
  dérivée d'une mesure de ce montage-ci.
- **La visibilité et le focus restent imposés par le pilote de recette**, page
  par page — limite héritée de D5, qu'aucun sous-bloc n'a levée.
- **Aucun client réel, aucun HiDPI réel** : `deviceScaleFactor = 1` partout, donc
  le legs HiDPI reste **inexercé**.
- **Le chemin d'extinction propre du superviseur** n'a toujours jamais été
  exercé, depuis D1.

### Pièges neufs — à connaître avant de toucher à ce terrain

- ⚠️ **Un budget d'injection dont l'origine est prise au DÉMARRAGE DU FIL ne
  mesure pas ce qu'on croit.** `AUDIO_FAUTE_LECTURE_MS=3000` rendait le critère ③
  inatteignable, la fenêtre se refermant **6 ms avant** l'élection de la porteuse.
  **Lire l'origine d'un compteur de banc avant de dimensionner sa fenêtre.**
- ⚠️ **Une fermeture arithmétique peut ne valoir que pour le PREMIER cycle** :
  le plan calculait « exactement 3 fautes » en ignorant le réarmement de D9, qui
  **recharge** le compteur. **Vérifier qu'un mécanisme voisin ne réapprovisionne
  pas ce qu'on croit épuiser.**
- 🔴 **Un ROUGE peut être VACUEUX parce que la ligne qu'il modifie n'est jamais
  ATTEINTE par le montage.** **Avant de bâtir un binaire défectueux, vérifier que
  sa ligne court.**
- ⚠️ **Un jeton de protocole peut n'être journalisé NULLE PART** : `grep
  'AudioMort'` rend **0 sur le vert comme sur le rouge**, et le compte réel se lit
  sur l'effet côté capteur (`capture audio morte`, 6 contre 0). **Un `grep` de
  recette doit viser une trace qui EXISTE, ce qui se vérifie sur le VERT avant de
  conclure du rouge.**
- 🔴 **Un message de commit est une pièce du dépôt, et personne ne le relit.**
  Sept des neuf écarts de D11 n'existent que là. **Relire les journaux contre le
  message, jamais l'inverse.**
- ⚠️ **Un dénominateur « 5/5 » peut exclure silencieusement ce que la recette
  cherchait** : les pages `?session=w-impair` de ⑥ ont la **forme** de l'issue ①.
  L'exclusion est justifiée (aucune session agent derrière, pages fermées avant le
  balisage), **elle n'était écrite nulle part**. *Énoncer la règle de sélection
  avant de compter* — le piège de D6, rejoué sur des pages.
- ⚠️ **Une provenance d'identité peut manquer sur une exécution et pas sur
  l'autre** : `assignations = []` sur deux JSON, et l'identité y retombe **sur un
  rang de nom** — la pratique que le protocole déclarait bannie. **Contrôler que
  la garantie de méthode a produit sa pièce, exécution par exécution.**
- ⚠️ **Un contexte audio suspendu rend `-Infinity` exactement comme un silence
  réel** — le contrôle ne pouvait pas distinguer les deux sans `ctx.resume()` ni
  `ctx.state`. Et **`-Infinity` se sérialise en `null`** : la sentinelle disparaît
  du journal versé.
- ⚠️ **Une sonde de pose lancée sur un vivier DÉJÀ PLEIN s'arrête sans verdict**,
  en dix-neuf lignes (`0x80070044`, `ERROR_TOO_MANY_NAMES`) : dix sorties
  orphelines survivent à un `Stop-Process`. **La purge passe AVANT la pose.**
- ⚠️ **Un découpage sur `:` casse sur un chemin Windows** : `--fenetres=hz:profil`
  donnait `profil = 'C'` sur `C:\dev\…`, **et le seul symptôme était l'absence
  d'annonce**.
- ⚠️ **L'ordre shell → superviseur → fenêtres fait capturer la console PowerShell
  de la tâche planifiée** : ouvrir les fenêtres **avant** le superviseur, qui les
  trouve par `enumerer_existantes`.
- ⚠️ **Une précondition prescrite par un plan peut rester VERTE sous le sabotage
  que le plan prescrit lui-même** (tâche 6). **Un plan n'immunise pas contre le
  contrôle vacueux : il en est une source.**
- ⚠️ **`Runtime.consoleAPICalled` rend la chaîne `"Object"` pour tout argument
  objet** — donc pour les nombres dont une grille a besoin. Les champs se relèvent
  dans `preview.properties`.
- ⚠️ **Une commande `git commit -m` dont le message porte des accents graves
  perd des morceaux de phrase** : le shell les interprète comme des substitutions
  de commande. Trois phrases ont été mutilées ainsi dans ce sous-bloc, rattrapées
  par un `--amend -F fichier`. **Passer les messages longs par un fichier.**
- 🔴 **UN COMPTE DE TESTS N'EST ATTRIBUABLE QU'ASSORTI DE SON HEURE quand deux
  chantiers partagent l'arbre.** Le compte client est passé de **107 (12
  fichiers) à 15:16** à **120 (13 fichiers) à 15:38** sans qu'aucune tâche de
  D11 n'y touche : le sous-projet ⑤ committait sur `client/` en parallèle. Le
  même piège a failli fausser une **marge de fichier** —
  `client/verify-webrtc.mjs` vaut **497 au dépôt commité** et **488 dans
  l'arbre de travail**, une modification non commitée du chantier voisin.
  **Mesurer avec `git show HEAD:` quand l'arbre est partagé**, et dater tout
  compte.
  ⚠️ **CE PIÈGE A ÉTÉ REPAYÉ LE LENDEMAIN, DANS L'AUTRE SENS** (19 août 2026,
  clôture de P2) : le 488 est commité, le fichier vaut **494**, et c'est
  désormais le chantier E (microphone) qui bouge sous le sous-projet ⑤ — trois
  commits pendant la seule tâche de clôture, `HEAD` passant de `f9cc330` à
  `988e2ee` puis `85ed23a`. **La leçon n'a rien perdu ; elle a été confirmée
  par le rôle inverse.**


### Ce que D11 lègue

**Legs de D10 réglés** : 4 (le remède inerte en mono-fenêtre — **corrigé ET
mesuré**), 5 (`AUDIO_FAUTE_RECONSTRUCTION` — **construite, et le critère ④ de D10
tombe**), 6 (la cause du refus — **répondue par une pièce, observable et non
expliquée**), 7 (le coût de la duplication — **mesuré, non détectable**), 8 (la
séparation des flux — **prouvée, rouge joué sur la VM**), **plus les deux legs
froids de D9** (n°11 les deux tests faibles, n°12 l'invariant non écrit).

**Ce qui reste dû :**

1. ⛔ **L'A/B sur `set_desired_bitrate`** (leg 1 de D10, n°3 de D9, n°4 de D6) —
   **écarté par décision**, joué deux fois sans rien trancher. **Condition de
   réouverture** : un montage dont la charge d'hôte est **CONTRÔLÉE** — et non
   seulement appariée —, sur **au moins huit paires**. Chantier de banc à part
   entière, pas une tâche de solde. ⚠️ *Cela n'affirme pas que l'appel est sans
   effet.*
2. ⛔ **Le maillon fautif du `Resize`** (leg 2 de D10) — **lu, et NON IDENTIFIÉ**.
   Piste déclarée comme piste : dans les deux exécutions **de D9**, la seule
   session muette à coup sûr est celle qui n'a **jamais** annoncé `visible=true`,
   et le basculement de sa voisine tombe **sous les 200 ms de lissage du
   `ResizeObserver`**. **Corrélation sur les journaux de D9, mécanisme non
   éprouvé.**
3. ⛔ **Les SIX constats parqués de D9 restent PERDUS.** Inventer une liste serait
   pire que de l'admettre.

**Legs neufs de D11 :**

4. ⛔ **Aucune cause NATURELLE de mort de capture audio n'est connue** : tout le
   remède est éprouvé **sous injection**.
5. ⛔ **La discrimination `audio_porteuse` contre `true` ne vit que dans un test
   d'hôte.** Il faudrait un montage produisant une session **non porteuse au
   moment de sa reconstruction** — état qu'aucun réglage du montage actuel ne
   produit.
6. ⛔ **Pourquoi le QUATRIÈME GUID** naît à la taille du registre quand les sept
   autres naissent à la taille demandée. Fait reproduit **sept fois**, mécanisme
   inconnu. ⚠️ **Et rien ne nettoie le registre.**
7. ⛔ **Le confondeur de ⑤ n'est pas levé** : « sortie surdimensionnée » et
   « quatrième session ouverte » sont confondus. Les départager demande un montage
   où le rang d'ouverture et la taille de naissance varient indépendamment — ce
   que D11 montre **non posable** par le levier de mode de sortie sur cette VM,
   la combinaison gagnante y étant `aucun drapeau (dynamique, non persisté)`, qui
   **par construction n'écrit pas le registre**.
8. ⛔ **L'ATTRIBUTION page ↔ fenêtre Windows n'est pas prouvée** : ④ prouve la
   distinction, pas la correspondance. Il y faudrait un identifiant porté de bout
   en bout par la fenêtre, pas un appariement RGB.
9. ⛔ **Le shell réémet `fenetre-ouverte` pour une fenêtre déjà ouverte** —
   constat survivant de D9, requalifié par D10 en **comportement du PRODUIT**, et
   dont la trace se relit dans ⑥ : une à trois pages d'application par exécution
   **sans session agent derrière**. **Mécanisme toujours non élucidé.**
10. ⛔ **Le couplage réélection / répit est exercé, pas borné.**

---

## 🗄️ Sous-projet ⑤ Plateforme — sous-bloc P1 : le service naît, absorbe le signaling, et persiste (19 août 2026)

Résultats complets :
`docs/superpowers/plans/2026-08-19-plateforme-p1-resultats.md`.
Plan : `docs/superpowers/plans/2026-08-19-plateforme-p1.md`.
Conception : `docs/superpowers/specs/2026-08-19-plateforme-design.md`.
Journaux : `docs/superpowers/plans/journaux-plateforme-p1/` — **15 fichiers,
UTF-8, AUCUNE séquence ANSI** (Vitest ne colore pas quand sa sortie est
redirigée) : ils se `grep`ent à plat, **sans `sed`**, contrairement à tous les
journaux du chantier D. **Une seule famille de lecture**, la plus simple du
dépôt.

⚠️ **Ils portent l'`ExperimentalWarning` de `node:sqlite`, et c'est VOULU** : il
est la trace visible de la décision §3.2 de la spec, et un contrôle du
sous-bloc interdit de l'éteindre. Ne pas le filtrer en les relisant.

⛔ **Aucune tâche de P1 n'a employé la VM Windows**, et c'est une décision de
conception, pas une commodité : ⑤ est un sous-projet serveur, et faire dépendre
sa recette d'une ressource exclusive et lente rendrait chaque itération coûteuse
et chaque échec ambigu.

### ① Le fait n°1 : `signaling/` n'existe plus

Les 284 lignes de production et les 483 lignes de test du paquet `signaling/`
vivent désormais dans **`plateforme/src/signaling/`**, à l'intérieur d'un
service qui porte aussi un serveur HTTP, une couche SQL portable et un dépôt.
**Le paquet `signaling/` est SUPPRIMÉ** — laisser deux copies est la façon dont
un fork dérive.

**Le protocole du fil ne change pas** : même port, même chemin racine, mêmes six
types relayés, même poignée de main `{role, session}`. **L'agent et le client
d'aujourd'hui fonctionnent sans recompilation**, et ce n'est pas une déclaration :
des pairs scriptés reproduisant **les octets exacts** d'`agent/src/signaling.rs`
et de `client/src/shell-page.ts` sont joués sur le chemin racine, l'offre est
relayée, et une ligne apparaît en base
(`journaux-plateforme-p1/compatibilite-pairs-reels.log`).

⚠️ **Ce qui change pour l'opérateur, et qui casse le lancement naïf :
`PLATEFORME_HOTE` est OBLIGATOIRE et n'a AUCUN défaut.** Sans elle :

```
Error: PLATEFORME_HOTE est obligatoire et n'a aucun défaut : nommer l'adresse
d'écoute, sans quoi le service écouterait sur toutes les interfaces.
```

**C'est délibéré.** L'ex-`server.ts` faisait `new WebSocketServer({ port })` sans
`host` : le service écoutait sur toutes les interfaces et délivrait des
identifiants TURN valables 86 400 s à quiconque atteignait le port. Poser un
défaut — même `127.0.0.1` — ferait passer le critère ④ **sans rien garantir**.
Une rupture bruyante vaut mieux qu'une écoute universelle silencieuse.

⚠️ **`scripts/run-agent.sh` n'est PAS modifié par P1** : l'agent ne lit aucune
des quatre variables neuves, elles sont toutes du côté serveur. Le piège maison
« toute variable neuve doit être ajoutée à `run-agent.sh` », payé en D1, D2 et
D7, **ne s'applique pas ici** — et le dire évite qu'un successeur cherche une
ligne manquante.

> ✅ **TOUJOURS VRAI APRÈS P2, et pour la même raison** (relevé le 19 août 2026,
> revue transverse de P2) : les **deux** variables que P2 ajoute
> (`PLATEFORME_SECRET_JETON`, `PLATEFORME_ORIGINE_CLIENT`) sont elles aussi
> entièrement du côté serveur, et `scripts/run-agent.sh` n'est pas davantage
> touché. ⚠️ **Seul le compte « quatre » a vieilli : il y en a SIX.**

### ② Les quatre variables d'environnement neuves

> ⚠️ **CE TITRE ET CE TABLEAU SONT LE RELEVÉ DE P1, ET LE COMPTE A VIEILLI : le
> sous-bloc P2 en ajoute DEUX, soit SIX en tout.** `PLATEFORME_SECRET_JETON`
> (**aucun défaut, le service REFUSE de démarrer sans elle**, et la refuse plus
> courte que `LONGUEUR_SECRET_MIN`) et `PLATEFORME_ORIGINE_CLIENT`
> (**facultative**, et son absence est un refus du navigateur, jamais une
> ouverture). Leur description complète vit dans la section **P2**, en fin de
> fichier — les quatre lignes ci-dessous ne sont pas fausses, elles sont
> **incomplètes**, et c'est le tableau qu'un opérateur lira en premier.


| Variable | Effet |
| --- | --- |
| `PLATEFORME_HOTE` | l'adresse d'écoute. 🔴 **AUCUN défaut** — le service REFUSE de démarrer sans elle. C'est **l'inverse** de la convention `=0 désarme` des variables de banc du chantier D : ici l'absence n'est pas un désarmement, c'est un refus |
| `PLATEFORME_PORT` | défaut **8080**. Un port non entier est refusé, jamais ramené au défaut |
| `PLATEFORME_BASE` | `sqlite` (défaut) ou `postgres`. **Une valeur inconnue LÈVE**, à deux endroits — `lireConfig` et `ouvrirBase` — plutôt que de retomber sur sqlite |
| `PLATEFORME_BASE_URL` | chemin de fichier SQLite (défaut `:memory:`) ou URL `pg` |

Instance Postgres de test, **versionnée et sans secret**, sur le modèle de
`docker-compose.coturn.yml` :

```bash
docker compose -f docker-compose.plateforme.yml up -d
```

### ③ Le fait n°2 : le sous-ensemble SQL portable, et ses TROIS gardes

Ce que cette couche échange : **une bibliothèque contre une discipline**. Aucun
compilateur ne vérifie une chaîne SQL écrite à la main. Ce qui remplace le
compilateur, ce sont des gardes — et **il est MESURÉ qu'aucun ne suffit seul** :

| Garde | Ce qu'il attrape | Ce qu'il ne peut PAS voir |
| --- | --- | --- |
| **lint statique** des `.sql` | `SERIAL`, `AUTOINCREMENT`, `now()`, `UUID`, `JSONB`, `BOOLEAN`, `_a INTEGER`, toute chaîne littérale | une construction licite des deux côtés mais de sémantique divergente |
| **double passe d'exécution** | la sémantique divergente | ce qui est licite ET de même sémantique aux valeurs employées |
| **le CHOIX DES VALEURS** *(garde neuf de P1, voir ⑤)* | une colonne trop étroite pour une valeur réelle | ce qu'aucune valeur du jeu d'essai n'exerce |

**Le relevé fondateur, 19 août 2026, SQLite 3.50.4** :

```
AUTOINCREMENT sqlite : ACCEPTE
SERIAL sqlite : ACCEPTE (type libre)
litteral avec ? : SQL valide
```

**SQLite accepte n'importe quel nom de type par affinité.** `SERIAL` y passe donc
sans bruit — et il passe aussi sur Postgres, **où il signifie autre chose**. Deux
passes vertes, deux schémas différents : **le test d'exécution ne peut pas
attraper `SERIAL`**, seul le lint le peut.

Et `SELECT '?' AS x` est du **SQL parfaitement valide** : une conversion naïve
des marqueurs `?` → `$1..$n` le rendrait `SELECT '$1' AS x` et changerait
silencieusement le sens de la requête. D'où la décision : **`rendreMarqueurs`
LÈVE si le SQL porte une apostrophe ou un guillemet**, ce qui rend la règle
« toute valeur passe en paramètre » **mécanique au lieu de documentaire**. Le
coût est nommé : aucune migration, aucune requête ne peut porter de littérale,
**pas même une valeur par défaut**.

### ④ Les six divergences spec/code, tranchées AVANT d'écrire une ligne

| # | Divergence | Ce qui a été tranché |
| --- | --- | --- |
| D1 | « les 483 lignes de tests restent INCHANGÉES » est **intenable** — `resilience.test.ts` lance le point d'entrée comme processus enfant par des chemins relatifs au paquet | **Aucune ASSERTION ne change**, le harnais change du minimum, et la preuve se fait par `sha256sum` et `git diff`, jamais par une impression |
| D2 | renommer `server.ts` en `relais.ts` touche `server.test.ts:3` | Déménagement **verbatim** d'abord, renommage **isolé** ensuite : c'est ce qui permet de prouver le premier |
| D3 | 🔴 `vm.utilisateur_id REFERENCES utilisateur(id)` **force P1 à créer `utilisateur`** | La table naît en P1 et **reste vide**. **SQLite ne sait pas ajouter une contrainte par `ALTER TABLE`** (mesuré : `near "CONSTRAINT": syntax error`) : une clé étrangère naît avec sa table ou n'existe jamais |
| D4 | la table `session` de la spec n'a **aucune** colonne pour le nom de session du signaling | `nom_session TEXT NOT NULL` ajoutée — sans elle la ligne écrite ne désigne rien. `utilisateur_id` et `vm_id` naissent **`NULL`** et **ne seront PAS resserrés** (voir D3) |
| D5 | `SERIAL` traverse les DEUX moteurs sans erreur | Deux gardes, pas un — et P1 en a découvert un **troisième** (voir ⑤) |
| D6 | « quatre étapes » de `verify-all.sh` en désigne trois | **Trois**, et le compte de trois est écrit plutôt qu'une quatrième étape inventée pour honorer un nombre |

⚠️ **D3 déborde P1 et doit être porté à P2 et P4** : toute contrainte que
`utilisateur` ou `vm` recevra plus tard **doit naître avec sa table**, ou exiger
une reconstruction en douze étapes — laquelle est elle-même un danger de
portabilité, Postgres ne s'y prenant pas de la même façon.

### ⑤ 🔴 Le défaut que la recette a trouvé : `INTEGER` ne tient pas un `Date.now()`

**Le service ne pouvait pas démarrer du tout sur Postgres, et rien ne le
disait.** Il échouait sur ses **PROPRES** migrations :

```
error: value "1787136797072" is out of range for type integer
```

`INTEGER` vaut jusqu'à **8 octets sur SQLite** et **exactement 4 sur Postgres**
(16.15). Le service n'écrit que des `Date.now()` (≈ 1,79 × 10¹²). **SQLite
l'acceptait sans un mot.**

**Pourquoi les deux gardes ne l'ont pas vu — c'est le fait le plus réutilisable
de P1** : le lint est **lexical**, et `INTEGER` est un type parfaitement licite ;
la double passe d'exécution n'écrivait que de **petites valeurs** (`1_000`), qui
tiennent dans quatre octets. **Ce n'est ni le lint ni la double passe qui
manquaient : c'est le CHOIX DES VALEURS.** Une suite qui n'écrit que des `1_000`
déclare portable un schéma qui refuse **toute écriture réelle** sur l'un des deux
moteurs.

**Mesure de l'angle mort**, arbre d'avant le correctif (`4183b7e~1`), harnais
seul porté à une magnitude d'époque :

| Moteur | Relevé |
| --- | --- |
| sqlite | `Test Files 11 passed (11)` / `Tests 57 passed (57)` |
| postgres | `Test Files 3 failed \| 8 passed (11)` / `Tests 13 failed \| 44 passed (57)` |

**Le remède, commit `4183b7e`, en trois pièces** : horodatages en **`BIGINT`**
(dans `0001-socle.sql` **et** dans la définition en dur de `migrations.ts`) ; le
harnais de test applique ses migrations à un instant de la **magnitude d'une
époque** (`INSTANT_MIGRATION = 1_700_000_000_000`), de sorte que **toute la
suite** exerce désormais la vraie magnitude ; et **deux gardes neufs, tous deux
vus rouges** — le lint refuse un `_a INTEGER`, et la définition **dupliquée** de
`schema_migration` est comparée entre `migrations.ts` et `0001-socle.sql`.

⚠️ **Cette seconde affirmation — « les deux définitions sont à l'identique » —
était portée par un commentaire QUE RIEN NE VÉRIFIAIT**, et elle aurait divergé
en silence : une base créée par la ligne en dur n'aurait plus ressemblé au
schéma que le socle décrit.

⚠️ **La portée du remède est bornée par une CONVENTION DE NOMMAGE** : le lint
reconnaît un horodatage à son nom en `_a` (`cree_a`, `vue_a`, `ouverte_a`,
`fermee_a`, `applique_a`). **Une colonne d'horodatage nommée autrement y
échapperait.**

### ⑥ La trace de session : QUAND, exactement

La ligne s'ouvre quand **les DEUX rôles sont présents**, jamais à la
déclaration — un seul pair n'est pas un appariement, et le superviseur se
déclare `agent` sur `bureau` au démarrage de la VM et peut y rester **seul des
heures**. Elle se clôt quand la table des sessions **se vide**.

⚠️ **Conséquence assumée** : un agent qui se déclare et repart sans jamais
rencontrer de client **ne laisse aucune trace**. C'est une décision, pas un
oubli ; elle se rouvrira quand on voudra observer les agents présents — sujet de
P3 (`vu_a`), pas de P1.
✅ **P3 A EU LIEU SANS LA ROUVRIR** (19 août 2026) : observer les agents ne
passe pas par la trace de session mais par `agent_enrole.vu_a`, que le
battement du canal `/agent` avance. **Le pronostic était juste sur le BESOIN et
faux sur le LIEU** ; `signaling/trace.ts` n'a rien eu à changer de ce côté.

🔴 **L'écriture ne doit JAMAIS pouvoir tuer une session.** Elle est lancée **sans
être attendue**, avec un `.catch` qui journalise et n'interrompt rien : une
promesse rejetée dans un gestionnaire d'événement `ws` **abat tout le process
Node**. La trace est une **observation** du signaling, jamais une **dépendance**.
Le coût est nommé — une écriture perdue ne se voit qu'au journal —, et c'est
pourquoi le test attend la ligne avec une **borne** et échoue sur expiration.

⚠️ **Le balayage de démarrage MENT** sur les sessions qui ont réellement survécu
à l'arrêt du service : le flux WebRTC ne dépend plus du signaling une fois
l'offre et la réponse échangées. Limite nommée, non corrigée.

### ⑦ Le verdict des quatre critères, avec leur nombre d'exécutions

**Aucun taux n'est revendiqué nulle part.**

| # | Critère | Verdict | Exéc. |
| --- | --- | --- | --- |
| ① | Le service apparie deux pairs simulés, le média négocie comme avant | **TENU** | 2 (+2 rouges) |
| ② | Une session appariée laisse une trace en base | **TENU** | 2 (+2 rouges) |
| ③ | La même suite passe sur `node:sqlite` **et** sur Postgres | **TENU** | 2 par moteur (+1 rouge) |
| ④ | L'écoute est bornée | **TENU, dans une portée étroite** | 2 (+1 rouge) |

Relevés : `Tests 64 passed (64)` sur **sqlite** comme sur **postgres**,
`typecheck` sortie **0**, `./scripts/verify-all.sh` sortie **0** — cargo
**467** + **32** + 0, client **107**, proto **35**, plateforme **64** / **64**.

**NEUF rouges jouées**, chacune avec son message verbatim et les sources
restaurées à l'identique après chacune (§2 du document de résultats).

⚠️ **Le critère ④ ne porte que sur la MOITIÉ de son nom.** Ce qui est établi :
sans `PLATEFORME_HOTE` le service ne démarre pas, et avec, il écoute sur cette
adresse. Ce qui ne l'est **pas** : qu'il soit injoignable **ailleurs** — sur une
machine de développement, `127.0.0.1` et l'adresse de l'interface sont toutes
deux locales, et la sonde exigerait une machine hors du réseau.

### ⑧ Ce que P1 n'établit PAS

- **Aucun taux.** Deux exécutions par critère, jamais une campagne.
- **Aucune latence, aucune charge.** La cible « < 3 s si VM chaude » n'est
  mesurée par aucun critère ; le nombre de sessions simultanées soutenues est
  **inconnu**.
- **Aucune exécution avec l'agent ou le navigateur RÉELS.** Les pairs sont
  simulés — jusqu'aux octets, mais simulés. La corroboration sur VM est prévue
  en fin de P3, hors critère.
- **L'inaccessibilité du service depuis une autre interface** (voir ⑦).
- **Le comportement de Postgres sous charge, en concurrence, ou après
  redémarrage** : la passe `test:postgres` éprouve un **dialecte**, pas un
  déploiement. C'est le critère ① de P5.
- **Aucune authentification.** Le port, s'il est atteint, délivre toujours des
  identifiants TURN valables 86 400 s à quiconque. **C'est P2**, et le critère ④
  est ce qui rend cette fenêtre tolérable — raison pour laquelle il est en P1.
  > ⚠️ **ANNOTÉ à la revue transverse de P2 (19 août 2026) : cet énoncé est le
  > relevé de P1 et n'est PAS réécrit, mais il n'est plus vrai QUE DE MOITIÉ.**
  > P2 refuse un pair de rôle **`client`** sans jeton — motif `jeton-absent`,
  > socket fermé, **aucun `ice-config` envoyé**. Un pair `{"role":"agent"}` est
  > **toujours** servi sans identité (mesuré, 1 exécution,
  > `journaux-plateforme-p2/e2-role-agent-toujours-anonyme.log`) : c'est **P3**.
  > Le critère ④ de P1 reste donc ce qui borne cette moitié-là.
  >
  > ✅ **RÉ-ANNOTÉ à la revue transverse de P3 (19 août 2026) : cette
  > moitié-là est FERMÉE**, mesurée **2 exécutions**
  > (`journaux-plateforme-p3/e2-ferme-{1,2}.log`). L'énoncé de P1 ci-dessus
  > n'est donc plus vrai **d'aucune moitié**, et il reste un relevé daté que
  > l'on n'écrase pas. ⚠️ **Ce que le critère ④ de P1 borne encore** : le
  > déni de service en une trame, qui court AVANT toute garde
  > (`signaling/resilience.test.ts`) et qu'aucune authentification ne peut
  > fermer. Le frein est P5 ③.

- **La scalabilité horizontale** : la persistance ne la procure pas. Un WebSocket
  vit dans un processus et un seul.
- **Aucune constante calibrée** : ni `DUREE_SECONDES = 86_400`, ni le port par
  défaut, ni les bornes de temps des tests, ni `INSTANT_MIGRATION`. Elles
  rejoignent la liste déjà longue de ce dépôt — `BPP_MIN`, `FACTEUR_FOCUS`,
  `PART_DORMANTE_BPS`, `HYSTERESIS`, `TAILLE_MAX_SORTIE`.
- **Aucune cause NATURELLE de perte d'écriture n'a été observée** : le `.catch`
  de la trace **n'a jamais couru** en recette.
- **La course entre l'appariement et la séparation** est fermée par un
  enchaînement de promesses, **jamais éprouvée sous concurrence réelle**.

### ⑨ Pièges neufs — à connaître avant de toucher à ce terrain

- ⚠️ **`SERIAL` et `AUTOINCREMENT` traversent SQLite sans bruit**, par affinité
  de type. Deux passes vertes peuvent décrire deux schémas différents.
- ⚠️ **`SELECT '?' AS x` est du SQL VALIDE.** Une conversion de marqueurs qui ne
  refuse pas les littérales change silencieusement le sens des requêtes.
- 🔴 **`INTEGER` n'a pas la même largeur des deux côtés**, et une suite qui
  n'écrit que de petites valeurs ne peut pas le voir. **Le choix des valeurs
  d'essai est un garde à part entière** — écrire des magnitudes réelles, pas des
  nombres commodes.
- ⚠️ **Un commentaire qui affirme que deux définitions dupliquées sont
  « à l'identique » doit être ÉPROUVÉ par un test**, sans quoi elles divergent en
  silence. Celui-ci ne l'était pas.
- ⚠️ **L'annonce du port du point d'entrée est COUPLÉE à une expression
  régulière de test** : `resilience.test.ts` lit `/le port (\d+)/` sur la sortie
  d'un processus enfant. Toute autre forme fait expirer le harnais au bout de
  10 s sur `démarrage du process signaling expiré`, **sans que rien ne désigne la
  cause**. Le couplage est écrit dans `plateforme/src/index.ts` plutôt que subi.
- ⚠️ **`pg` prend `:memory:` pour un nom d'hôte.** Un test qui lance un
  processus enfant en lui transmettant `PLATEFORME_BASE=postgres` **sans** l'URL
  le fait mourir sur `ECONNREFUSED` — le service ayant **raison** de refuser de
  démarrer. `resilience.test.ts` fixe donc les quatre variables au lieu d'hériter
  de l'environnement. ⚠️ **Elles sont CINQ depuis P2** (relevé par la commande le
  19 août 2026) : `PLATEFORME_SECRET_JETON` s'y ajoute, sans quoi le processus
  enfant refuserait de démarrer pour une raison de plus.

- ⚠️ **`env.X ?? 'défaut'` ne s'applique PAS à la chaîne vide.** Le plan
  annonçait deux tests rouges en posant un défaut sur `PLATEFORME_HOTE` ; **un
  seul** rougit. Le contrôle tient, mais pas pour la raison écrite.
- ⚠️ **`node:sqlite` est EXPÉRIMENTAL sur Node 24** et crie à chaque import. Le
  cri est **conservé délibérément** dans les journaux ; `engines` épingle la
  majeure, et un changement de majeure impose de rejouer la suite **avant tout
  autre travail**.
- ⚠️ **Un `BEGIN` émis sur un POOL `pg` et un `COMMIT` émis ensuite sur le pool
  prennent deux clients DIFFÉRENTS**, donc deux transactions différentes — et le
  tout **silencieusement**, la première restant ouverte jusqu'à expiration. Le
  client est pris une fois et gardé pour toute la durée du corps.

### ⑩ Le relevé de tailles, PAR LA COMMANDE, après la dernière édition

Le § « Portée » en tête de ce fichier liste désormais **`plateforme/`** au lieu
de `signaling/`.

**Le dépôt entier ne porte que DEUX fichiers de plus de 500 lignes**, et ce sont
les deux lignes de la dette gelée — `agent/src/encode.rs` **1536** et
`agent/src/windows_source.rs` **630** —, **ni l'un ni l'autre touché par P1**.
Les plus gros fichiers de `plateforme/` :

| Fichier | Lignes | Marge |
| --- | --- | --- |
| `plateforme/src/signaling/server.test.ts` | ~~255~~ **272** (19 août 2026, P2) | ~~245~~ 228 |
| `plateforme/src/signaling/relais.ts` | ~~219~~ **310** (19 août 2026, P2) | ~~281~~ 190 |
| `plateforme/src/signaling/resilience.test.ts` | ~~181~~ **208** (19 août 2026, P2) | ~~319~~ 292 |
| `plateforme/src/signaling/trace.test.ts` | ~~151~~ **194** (19 août 2026, P2) | ~~349~~ 306 |
| `plateforme/src/base/pilotes.test.ts` | ~~135~~ **141** (19 août 2026, P2) | ~~365~~ 359 |
| `plateforme/src/base/sous-ensemble.test.ts` | 125 | 375 |
| `plateforme/src/base/migrations.ts` | 118 | 382 |

> ⚠️ **CINQ des sept chiffres de ce tableau ont vieilli sous le sous-bloc P2, et
> ils sont barrés À LEUR PLACE** (relevés par la commande le 19 août 2026, après
> la dernière édition de la ronde de clôture de P2). Les deux derniers étaient
> exacts. **Le plus gros mouvement est `relais.ts`, +91** : c'est le fichier que
> la phrase ci-dessous désigne comme « le seul que P2, P3 et P4 feront
> grossir », et il a fait exactement ce qui était annoncé. **Sa marge reste de
> 190** — aucun plafond n'est approché, et l'extraction de P1 est ce qui le
> permet.


**Aucun fichier de `plateforme/` n'approche le plafond.** `relais.ts` — le seul
que P2, P3 et P4 feront grossir — dispose de **281** lignes de marge, et **sa
table des sessions en est DÉJÀ SORTIE** (`signaling/appariement.ts`, **PUR**,
générique, testable sans ouvrir un socket). **C'est le geste que ce dépôt a
établi en D9** : l'extraction faite AVANT l'addition rend sa marge, celle faite
après se paie d'une compression que ce fichier interdit nommément.

⚠️ **La divergence texte/commande signalée par ce fichier depuis D10 n'est pas
tranchée et P1 n'en crée pas de seconde** : le § « Portée » énumère des
répertoires, la commande, elle, ne filtre que `node_modules`, les verrous,
`dist/`, `testdata/`, `docs/` et `CLAUDE.md` — et attrape donc
`client/verify-webrtc.mjs`, hors de `client/src/`. **Cette décision appartient au
propriétaire du dépôt.**

### ⑪ La revue transverse de fin de branche

Sa cible propre : **les affirmations devenues fausses dans la branche
elle-même**. Un déménagement en produit une classe entière — des commentaires
qui nomment leur ancien emplacement.

| Où | Ce qui était devenu faux | Sort |
| --- | --- | --- |
| `plateforme/src/signaling/relais.ts:2` | « **Aucun état persistant** » — **P1 en pose un** | **CORRIGÉ**. ⚠️ « Aucune authentification » reste **VRAI** et n'est pas touché : les deux clauses de la même phrase n'ont pas le même sort. ❌ **CETTE SECONDE CLAUSE EST TOMBÉE À SON TOUR au sous-bloc P2** (19 août 2026), et c'est P2 qui l'a corrigée dans le fichier — elle y porte désormais la **moitié exacte** qui reste vraie (le rôle `agent`, anonyme jusqu'à P3). **Le pronostic de P1 n'était donc juste que pour un sous-bloc**, ce qui est la durée de vie ordinaire d'un « reste VRAI ». ❌ **ET LA MOITIÉ QUE P2 AVAIT LAISSÉE EST TOMBÉE À SON TOUR AU SOUS-BLOC P3** (19 août 2026, commit `254fdd5`), qui l'a corrigée dans le même fichier. **La même phrase d'en-tête a donc été corrigée TROIS FOIS, une par sous-bloc — P1, P2, P3 —, et chaque correction a laissé derrière elle une « moitié qui reste vraie » que la suivante a dû reprendre.** C'est la mesure la plus nette qu'ait ce dépôt de ce que vaut un « reste VRAI » : un sous-bloc, jamais deux |

| `plateforme/src/signaling/relais.ts` (`isJsonObject`) | « aucun `uncaughtException` n'est installé dans **index.ts** » — le point d'entrée a **changé de fichier** | **CORRIGÉ**, et la propriété a été **REVÉRIFIÉE** sur le nouveau : `plateforme/src/index.ts` n'installe qu'un `SIGINT` |
| `plateforme/src/config.ts:3` | `signaling/src/server.ts:67` — chemin disparu | **CORRIGÉ** |
| `plateforme/src/depot/session.ts:6` | `signaling/src/ice.ts:32-42` — chemin disparu | **CORRIGÉ** (les lignes 32-42, elles, ont été **relues** et sont justes) |
| `plateforme/src/signaling/trace.ts:28` | `signaling/ice.ts` — chemin disparu, écrit **dans la branche même** | **CORRIGÉ** |
| `plateforme/src/signaling/appariement.ts:8` | « extrait de `server.ts` » sans dire ce qu'est ce fichier aujourd'hui | **CORRIGÉ** |
| `CLAUDE.md:1913` | `signaling/src/ice.ts` (chantier C volet 2) | **CORRIGÉ**, le piège « le signaling doit être relancé AVEC l'environnement » étant **conservé entier** et annoté du nom neuf du processus |
| `.gitignore:10` | `signaling/dist/` ne désignait plus rien | **CORRIGÉ** en tâche 2 |
| **`agent/src/superviseur/protocole.rs:5`** | « à ce que **`signaling/src/server.ts`** accepte de relayer » — ce fichier n'existe plus ; il est aujourd'hui `plateforme/src/signaling/relais.ts` | ❌ **NON CORRIGÉ, et c'est délibéré** : `agent/` est le périmètre d'un travail concurrent au moment de P1. **La propriété énoncée reste VRAIE** (les six types relayés sont inchangés) ; **seul le chemin est périmé.** Dette d'une ligne, à reprendre par qui touchera ce fichier. ✅ **SOLDÉE PAR D11, PAS PAR P2** — le fichier nomme aujourd'hui `plateforme/src/signaling/relais.ts` et dit à ses lignes 8-9 que l'ancien chemin n'existe plus. *(Relevé par la revue transverse de P2 le 19 août 2026, qui n'en est pas l'auteur : le tableau D11 en tête de ce fichier l'enregistrait déjà — `superviseur/protocole.rs` 101 → 110, « le chemin `signaling/` disparu ». **Ce ❌ contredisait donc ce ✅ depuis D11, dans le même fichier**, et personne ne l'avait rapproché.)* |


**Trois affirmations FABRIQUÉES par le plan lui-même**, et corrigées par la
mesure plutôt que recopiées :

1. 🔴 sa **tâche 12** attend `server.test.ts` à l'empreinte `2a1304e0…`, ce que
   le renommage prescrit par sa **propre tâche 4** rend impossible. Réel :
   **`5e90b854…`**. Le contrôle décidable est ailleurs, et il a été joué —
   `diff` contre la version d'avant le déménagement rend **une seule ligne, la
   3**, celle de l'import ;
2. sa **tâche 1** annonce « les DEUX premiers tests échouent » ; **un seul**
   échoue (voir ⑨) ;
3. sa **tâche 2/3** promet `resilience.test.ts` à « 1 puis 2 lignes » ; le `diff`
   porte **trois hunks**, tous dans le harnais, **aucune assertion touchée**.

⚠️ **Et un journal de cette recette a porté sa PROPRE affirmation fausse, dans
le tour qui l'écrivait** : `critere-3-double-passe.log` concluait « SQLite est
VERT sur un schéma que Postgres refuse » alors que son relevé, **trois lignes
plus haut**, montrait sqlite à `2 failed | 62 passed` — les deux gardes neufs
étant, eux, indépendants du moteur. **La correction est portée dans le journal
lui-même**, avec l'énoncé daté qui, lui, est exact.

### ⑫ Ce que P1 lègue à P2, P3, P4 et P5

1. ✅ **P2 — l'authentification. FAIT le 19 août 2026, ET DE MOITIÉ SEULEMENT.**
   Un pair `client` sans jeton est refusé ; **un pair `agent` reçoit toujours
   des identifiants TURN de 86 400 s sans aucune identité** — c'est P3.
   `utilisateur` **n'est plus vide** : P2 lui a donné son comportement, et
   **pas sa table**, exactement comme cette ligne le prévoyait.
   ✅ **L'AUTRE MOITIÉ EST FAITE PAR P3 le 19 août 2026** : un pair
   `{"role":"agent"}` sans identité reçoit `authentification requise` et
   **aucun `ice-config`** — mesuré sur le fil, **2 exécutions**
   (`journaux-plateforme-p3/e2-ferme-{1,2}.log`).
2. ✅ **P2 et P4 — toute contrainte doit naître avec sa table** (D3) : SQLite ne
   sait pas l'ajouter par `ALTER TABLE`, et la reconstruction en douze étapes
   n'est pas portable. **P2 l'a APPLIQUÉ** : `0002-identite.sql` fait naître
   `famille` et `remplace_par` **avec** `jeton_rafraichissement`, ainsi que sa
   clé étrangère vers `utilisateur(id)`. ~~**La consigne reste entière pour
   P4.**~~ ✅ **P4 N'A EU AUCUNE MIGRATION À ÉCRIRE** (20 août 2026) : la seule
   colonne dont il est propriétaire, `vm.utilisateur_id`, existe depuis P1 avec
   son index unique partiel. La consigne n'a donc pas été mise à l'épreuve par
   lui, et **elle reste entière pour le premier sous-bloc qui ajoutera une
   table**.

3. ✅ **P3 — `session.vm_id` EST RENSEIGNÉE** (19 août 2026) : le nom de
   session porte la VM (`<préfixe>:bureau`), et `signaling/trace.ts` découpe le
   préfixe pour le chercher dans `agent_enrole`. ⚠️ **`utilisateur_id` reste
   `NULL` pour une session appariée par un agent seul, et les DEUX colonnes
   restent nullables** — pas par dette : deux cas rendent `null` honnêtement
   (session sans préfixe, préfixe inconnu de la base). C'est le coût de D4,
   assumé, et il ne bouge pas.
4. ✅ **P3 — observer les agents présents : FAIT, mais PAS SUR LA COLONNE QUE
   CETTE LIGNE NOMMAIT.** Le pronostic disait `vm.vue_a` ; P3 a créé
   `agent_enrole.vu_a`, avancé par le battement du canal `/agent`
   (`agents/canal.ts`) et jugé par `agents/fraicheur.ts`. **`vm.vue_a` n'est
   écrite par AUCUN code de production à ce jour** (relevé par `grep -rn
   "vue_a" plateforme/src` le 19 août 2026 : seuls des tests l'écrivent).
   ⚠️ Elle reste donc une colonne orpheline du socle, **signalée et non
   retirée** : la retirer demanderait une reconstruction de table que SQLite ne
   fait pas comme Postgres (leg n°2 de P1, en sens inverse).
5. ❌ **P3 — le canal `/agent` N'EST PAS DANS `relais.ts`, et c'est délibéré.**
   Ce pronostic s'est révélé faux **de lieu**, pas de besoin : le canal vit
   dans `http/serveur.ts` (`CHEMIN_AGENT = '/agent'`), sur son **propre**
   `WebSocketServer`. La raison est la divergence E4 du plan de P3 — la garde
   du relais est **pure et synchrone**, l'enrôlement exige une lecture de base
   donc un `await`, et le faire vivre dans le relais aurait rendu la garde
   asynchrone. Le routage `noServer` existait bien, comme annoncé ; c'est la
   branche qui a été posée ailleurs.
6. ⛔ **P5 — l'inaccessibilité effective du service** depuis une autre interface
   n'est pas établie, et ne peut pas l'être sans une machine hors du réseau.
7. ⛔ **P5 — le comportement de Postgres sous charge, en concurrence, après
   redémarrage.** P1 éprouve un **dialecte**, pas un déploiement.
8. ⚠️ **Tous — la convention de nommage `_a`** est ce qui rend le lint des
   horodatages opérant. Une colonne d'horodatage nommée autrement y échapperait
   (voir ⑤).
9. ⚠️ **Tous — `verify-all.sh` exige désormais une instance Postgres.** C'est le
   prix assumé du « un saut est un échec » : sans elle, l'étape **ÉCHOUE** et le
   script s'arrête, il ne se saute pas avec un avertissement.

---

## 🔐 Sous-projet ⑤ Plateforme — sous-bloc P2 : l'identité des humains, et la moitié du trou que la garde ferme (19 août 2026)

Résultats complets :
`docs/superpowers/plans/2026-08-19-plateforme-p2-resultats.md`.
Plan : `docs/superpowers/plans/2026-08-19-plateforme-p2.md` (commit `826a16d`).
Conception : `docs/superpowers/specs/2026-08-19-plateforme-design.md`, §3.5 et §4 « P2 ».
Journaux : `docs/superpowers/plans/journaux-plateforme-p2/` — **29 fichiers
suivis par git** (relevé par `git ls-files`), **tous UTF-8, AUCUNE séquence
ANSI** (vérifié par `grep -lP '\x1b\['` : zéro fichier) : ils se `grep`ent à
plat, **sans `sed`**. **Une seule famille de lecture**, comme P1 et
contrairement au chantier D. Deux singularités sans conséquence :
`cors-et-message-de-refus.log` mêle CRLF et LF, `verify-all-1.log` porte une
ligne de 333 caractères.

⛔ **Aucune tâche de P2 n'a employé la VM Windows**, comme P1, et pour la même
raison : ⑤ est un sous-projet serveur.

### ① Le fait n°1 : la garde mord, et elle ne ferme QUE LA MOITIÉ `client`

Un pair qui se déclare `{"role":"client", session, jeton}` doit désormais
présenter un **jeton d'accès valide**, sans quoi il est refusé, journalisé, son
socket fermé (code 1008, motif `jeton-absent`) — **et il ne reçoit aucun
`ice-config`**. La garde passe **AVANT** `Appariement::declarer`, et l'ordre
n'est pas indifférent : un pair refusé qui serait entré dans la table y
occuperait le rôle et empêcherait le pair légitime d'arriver, soit un déni de
service ouvert à l'anonyme, obtenu **précisément en refusant de
s'authentifier**.

🔴 **UN PAIR `{"role":"agent"}` EST TOUJOURS ACCEPTÉ SANS AUCUNE IDENTITÉ, et
reçoit ses identifiants TURN de 86 400 s.** Ce n'est pas une inférence : c'est
**mesuré** sur le service de la branche (**1 exécution**,
`e2-role-agent-toujours-anonyme.log`), où une sonde qui ne présente rien reçoit
`turn:127.0.0.1:3478` avec son `username` et son `credential`, et n'est jamais
refusée. L'agent Rust n'a pas d'identité avant **P3** (divergence E2), et lui en
exiger une casserait le chantier D. **C'est donc toujours `PLATEFORME_HOTE` —
le critère ④ de P1 — qui borne cette moitié-là de la fenêtre.**

Le libellé du critère ① le dit littéralement : « un pair **`client`** non
authentifié ». **Ne pas lire « le trou TURN est fermé ».**

### ② Les deux variables d'environnement neuves — et trois de recette

| Variable | Effet |
| --- | --- |
| `PLATEFORME_SECRET_JETON` | le secret HMAC des jetons d'accès. 🔴 **AUCUN défaut** — le service REFUSE de démarrer sans elle, **comme `PLATEFORME_HOTE`**, et la refuse aussi **plus courte que `LONGUEUR_SECRET_MIN = 32`**. Un défaut aléatoire invaliderait toutes les sessions à chaque redémarrage ; un secret court n'authentifie personne |
| `PLATEFORME_ORIGINE_CLIENT` | l'origine autorisée pour CORS. **FACULTATIVE**, et **son défaut est le REFUS** : absente, aucun en-tête CORS n'est émis et le navigateur refuse — bruyamment. **Jamais `*`, sous aucune condition** (assertion de test, pas intention). L'asymétrie avec `PLATEFORME_HOTE` est raisonnée : une adresse d'écoute absente produirait une écoute universelle **silencieuse**, une origine absente produit un refus **visible** ; et refuser de démarrer pour elle casserait P5, où le proxy inverse met les deux sur la même origine |

⚠️ **Trois variables de plus, qui ne sont PAS du service mais des OUTILS DE
RECETTE** (`client/recette/jeton-recette.mjs`) : `RECETTE_EMAIL`,
`RECETTE_MOTDEPASSE` et `PLATEFORME_URL` (défaut `http://127.0.0.1:8080`).
**Aucune n'est lue par `config.ts`.** Sans les deux premières, les trois pilotes
CDP **avertissent et continuent** — ils n'échouent pas, ce qui les laisse
utilisables contre un service sans garde.

⚠️ **`scripts/run-agent.sh` n'est PAS modifié par P2**, exactement comme en P1 :
les deux variables du service sont côté serveur, l'agent n'en lit aucune. Le
piège maison « toute variable neuve doit être ajoutée à `run-agent.sh` » ne
s'applique toujours pas.

⚠️ **`.env` ne porte aucune `PLATEFORME_*`** (relevé le 19 août 2026) : un
`source .env` ne suffit donc pas à lancer le service, et il manque désormais
**deux** variables sans défaut au lieu d'une.

### ③ Le format de mot de passe porte son propre algorithme

`scrypt$N$r$p$sel$empreinte`, sel et empreinte en `base64url`,
`PARAMETRES_COURANTS = { N: 16384, r: 8, p: 1 }`, clé de 32 octets. **Le point
n'est pas le choix des paramètres, c'est que le format les porte** : le jour où
ils seront calibrés, `doitEtreRehache` le dit et un re-hachage à la connexion
suivante suffit — **sans migration de données**.

⚠️ **Ces paramètres NE SONT PAS CALIBRÉS.** Les 29 ms mesurés sont une mesure
sur une machine, **pas un objectif atteint** : aucun objectif n'a été posé.

### ④ Le rejeu d'un jeton de rafraîchissement fauche TOUTE la famille

La spec §5 posait `jeton_rafraichissement(id, utilisateur_id, empreinte,
expire_a, revoque_a)`. **Ces cinq colonnes ne permettent PAS de détecter un
rejeu** (divergence E5) : rien n'y relie un jeton tourné à son successeur, si
bien que présenter un jeton déjà tourné ne pourrait révoquer que la ligne **déjà
révoquée** — et le voleur qui a tourné le premier garderait son jeton neuf.

`0002-identite.sql` ajoute donc **`famille TEXT NOT NULL`** et **`remplace_par
TEXT NULL`**, qui **naissent avec la table** (leçon D3 de P1 : SQLite ne sait
pas ajouter une contrainte par `ALTER TABLE`). Règle : présenter un jeton dont
la ligne porte `revoque_a` non nul est un **REJEU**, et **toute la famille est
révoquée d'un coup**, refus typé `rejeu`.

🔴 **LA RECETTE A TROUVÉ LÀ UN DÉFAUT RÉEL, QUE LA RELECTURE N'AVAIT PAS VU** :
`/auth/rafraichir` ouvrait une famille **NEUVE** à chaque appel. La détection de
rejeu révoquait alors une famille à laquelle le jeton volé n'appartenait plus,
et **ne protégeait donc rien** — mécanisme présent, effet absent. Corrigé, et la
preuve est de bout en bout sur le service vivant (**2 exécutions**,
`rotation-et-rejeu-bout-en-bout.log`) : après le rejeu de `r0`, **c'est `r2`, le
jeton le plus RÉCENT, qui tombe en `401 {"refus":"revoque"}`**. **La ligne qui
compte est celle-là**, pas le refus du jeton rejoué.

⚠️ **Aucun test de dépôt ne pouvait l'attraper** : chaque test partait d'une
famille propre.

### ⑤ Le verdict des quatre critères, avec leur nombre d'exécutions

**Aucun taux n'est revendiqué nulle part.**

| # | Critère | Verdict | Exéc. |
| --- | --- | --- | --- |
| ① | un pair `client` non authentifié est refusé **et ne reçoit aucun `ice-config`** | **TENU** | **2** |
| ② | un jeton expiré est refusé, **horloge qui VARIE** | **TENU** | **2** |
| ③ | un utilisateur ne peut pas rejoindre la session d'un autre, appartenance **enregistrée** | **TENU** | **2** (+1 sur le service vivant) |
| ④ | un mot de passe n'est **jamais** journalisé ni renvoyé | **TENU** | **2** |

⚠️ **LES DEUX EXÉCUTIONS D'UN CRITÈRE DIFFÈRENT PAR LE MOTEUR DE BASE** — sqlite
pour la première, postgres pour la seconde. **Ce n'est pas ce que le plan
prescrivait** (il disait « deux exécutions » sans dire lesquelles) : la décision
est prise par la recette et déclarée dans l'en-tête de chaque journal. Elle est
**plus forte** que deux passes identiques pour les critères qui touchent la
base, et **strictement identique** pour ceux qui sont purs.

**HUIT rouges jouées, là où le plan en annonçait SEPT**, chacune avec son
message verbatim et les sources restaurées après chacune.

🔴 **La huitième existe parce qu'une PRÉDICTION DU PLAN ÉTAIT FAUSSE.** Le plan
annonçait que la rouge ①A ferait tomber « les **DEUX** assertions du critère ① ».
**Elle n'en fait tomber qu'UNE** : `expect` interrompt le test à la première, si
bien que la seconde — « aucun `ice-config` », c'est-à-dire la fuite
d'identifiants TURN de 24 h — **n'était éprouvée par rien**. La rouge ①A-bis
l'éprouve **seule** : la garde refuse toujours, mais la configuration ICE part
**avant** la vérification, et l'assertion rougit. **C'est le patron que ce dépôt
a déjà payé cinq fois — un contrôle qu'on n'a jamais vu rouge n'est pas un
contrôle — et il se rejoue ici sur une assertion, pas sur un test.**

🔵 **La rouge ①B est la plus décisive, et elle ne dépend d'AUCUNE modification
de notre part** : `git worktree add /tmp/p1-rouge 19f6409` sort le service **de
P1**, et la même sonde y reçoit de vrais identifiants TURN sans jamais être
refusée. C'est la ROUGE gratuite que la spec §7.2 annonçait, **jouée et non
supposée**.

### ⑥ Ce que P2 n'établit PAS

- **Aucun taux, nulle part** : deux exécutions par critère au mieux, une pour
  plusieurs mesures de bout en bout.
- 🔴 **Le rôle `agent` reste ANONYME** et reçoit toujours des identifiants TURN
  de 86 400 s (voir ①). **Le trou n'est fermé qu'à moitié.**
- **La garde ne s'applique qu'à la POIGNÉE DE MAIN.** Une session déjà ouverte
  n'est **jamais** revérifiée : un jeton qui expire en cours de session ne coupe
  rien tant que le socket vit. La spec §3.5 dit qu'un jeton court n'est pas
  révocable **avant** expiration ; ici il ne l'est **même pas après**.
- **L'appartenance de session ne survit pas à un redémarrage** (E3) : le
  registre de décision est **en mémoire**, et après un redémarrage un nom de
  session libéré peut être revendiqué par un autre utilisateur. La vraie réponse
  est le **préfixe opaque de P3**. ✅ **Il existe depuis le 19 août 2026**
  (`agents/prefixe.ts`, 128 bits, durable dans `agent_enrole.prefixe_session`)
  — ⚠️ **et il ne suffit pas** : étant **par VM** et non par session, il ferme
  la devinabilité entre VMs sans fermer la revendication au sein d'une VM. Le
  registre d'appartenance reste en mémoire. **Réduit, pas soldé.**
- **Aucune protection contre le rejeu du jeton d'ACCÈS** : il est porteur, et
  quiconque l'obtient peut ouvrir une session jusqu'à son expiration.
- **Aucune constante n'est calibrée** : `N`/`r`/`p`, `DUREE_JETON_ACCES_MS`
  (600 000), `DUREE_RAFRAICHISSEMENT_MS` (30 jours), `LONGUEUR_SECRET_MIN`
  (32), la marge de rafraîchissement du client, et `DUREE_SECONDES = 86 400`
  que P2 **ne recalibre pas**. Elles rejoignent la liste déjà longue du dépôt —
  `BPP_MIN`, `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`, `HYSTERESIS`,
  `TAILLE_MAX_SORTIE`.
- **L'égalisation temporelle du chemin de connexion n'est pas mesurée**, et ne
  le sera pas ici : un test de temporisation serait instable. Ce qui **est**
  éprouvé est le **message identique** entre compte inexistant et mot de passe
  faux (`{"refus":"identifiants"}`, **1 exécution**), qui est décidable.
- **AUCUN navigateur réel n'a authentifié quoi que ce soit** hors la tâche 17,
  qui n'est pas un critère. **L'écran de connexion n'a jamais été employé par un
  humain** : aucun clic, aucun formulaire soumis. Le chemin `connexion.html` →
  `poser()` → `shell.html` est **raisonné et compilé**, pas observé. Et
  **`shell-page.ts` n'a jamais redirigé** dans une exécution mesurée.
- **Aucun frein sur les routes d'authentification** — c'est P5 ③. Conséquence à
  assumer d'ici là : `/auth/connexion` est ouverte à la force brute, bornée
  seulement par les ~29 ms de `scrypt` et par l'écoute restreinte de
  `PLATEFORME_HOTE`. **C'est la même raison qui rend la fenêtre d'E2 tolérable,
  et elle a la même fragilité.**
- **Aucune terminaison TLS, aucun cookie, aucun en-tête de sécurité** : P5.
- **Aucun audit par un tiers** : CSRF, fixation de session et attaques
  temporelles sont traités par des choix **raisonnés, non éprouvés** (spec §8).
- **Le jeton vit dans `localStorage`**, donc il est lisible par tout script de
  la page. C'est un arbitrage écrit dans `jeton.ts`, **pas un oubli**, et il se
  rouvrira en P5.

### ⑦ Pièges neufs — à connaître avant de toucher à ce terrain

- 🔴 **`scrypt` REFUSE `N = 32768` avec le `maxmem` par défaut, et le message ne
  parle PAS de `N`** — mesuré le 19 août 2026 sur Node v24.9.0 :
  `Invalid scrypt params: error:030000AC:digital envelope
  routines::memory limit exceeded`. La limite est celle de `maxmem` (32 MiB),
  franchie dès que `128·N·r` la dépasse. **Le paramètre qu'on croirait meilleur
  est celui qui casse**, et le diagnostic n'est pas dans le message.
- 🔴 **`timingSafeEqual` LÈVE sur des longueurs différentes** —
  `Input buffers must have the same byte length`. Une empreinte tronquée en base
  ferait donc **lever** la vérification de mot de passe au lieu de rendre
  `false` : l'appelant HTTP répondrait **500 là où il doit répondre 401**, et
  l'écart de comportement serait à lui seul un oracle. **Comparer les longueurs
  D'ABORD, partout** — mot de passe et signature JWT.
- 🔵 **Un JWT HS256 s'écrit ENTIÈREMENT avec `node:crypto` : aucune dépendance
  n'est nécessaire.** Le témoin en est le contrôle d'allow-list de P1
  (`base/pilote.test.ts`), qui exige `['pg', 'ws']` et **n'a pas bougé** —
  vérifié le 19 août 2026. Un sous-bloc entier d'authentification a donc été
  livré à dépendances constantes.
- ⚠️ **`localStorage` est le SEUL stockage partagé entre une page et les
  fenêtres qu'elle ouvre par `window.open`**, et
  `Page.addScriptToEvaluateOnNewDocument` **ne court PAS** sur ces fenêtres
  (piège mesuré en D5). Un pilote CDP ne peut donc **pas** y injecter un jeton
  après coup : c'est pourquoi le jeton doit vivre dans `localStorage`, et
  pourquoi la page-shell (`client/src/shell-page.ts`, qui ouvre par
  `window.open`) fonctionne sans injection supplémentaire.
- ⚠️ **`client/` n'a pas `@types/node`** : un test écrit avec `Buffer` passe
  sous Vitest et **casse `npm run typecheck`** (`TS2580 Cannot find name
  'Buffer'`). Employer `btoa` / `TextEncoder`. C'est exactement la raison d'être
  de `verify-all.sh` : Vitest et Vite reposent sur esbuild, qui transpile **sans
  vérifier les types**.
- 🔴 **`cd client && npx vitest run` NE COUVRE PAS `proto/ts/`.** La racine
  Vitest est `client/`, donc les tests de `client/src/` seuls. **Un test posé
  hors de `client/src/` ne tournerait pas, et personne ne le verrait** ;
  `proto/` exige `cd proto && npm test`. Les deux comptes se relèvent
  séparément, toujours.
- 🔴 **UN COMPTE DE TESTS N'EST ATTRIBUABLE QU'ASSORTI DE SON COMMIT** quand
  deux chantiers partagent l'arbre — piège hérité de D11, rejoué ici en sens
  inverse : c'est P2 qui faisait bouger `client/` sous D11, et c'est le chantier
  E (microphone) qui faisait bouger `agent/` sous P2. **Le témoin
  `verify-all.sh` de la recette a échoué à sa PREMIÈRE étape sur un test Rust
  du voisin**, non suivi par git au moment de la mesure. Ce n'était pas P2, et
  la preuve n'est pas une affirmation : un `git worktree add` du dernier commit
  de P2 rend `467 passed; 0 failed` (`temoin-cargo-arbre-propre.log`).
- ⚠️ **Un jeton de recette doit être OBTENU, jamais FORGÉ.** Un script qui
  signerait lui-même porterait `PLATEFORME_SECRET_JETON` dans un fichier
  versionné ou dans l'`argv` d'un processus : **le trou serait DÉPLACÉ, pas
  fermé**. `client/recette/jeton-recette.mjs` ne sait donc rien signer — il sait
  appeler `POST /auth/connexion`.
- ⚠️ **Deux constantes écrites dans DEUX LANGAGES sans `import` possible entre
  eux divergent en silence.** Les clés du coffre vivent dans `client/src/jeton.ts`
  (TypeScript) et dans `client/recette/jeton-recette.mjs` (JavaScript de
  recette) : ce dernier **relit le fichier TypeScript** et refuse de semer si
  elles ont divergé. **Le garde a été VU LEVER**, par mutation de la clé.
- ⚠️ **Un mot de passe ne se passe JAMAIS en `argv`** : `ps` expose la ligne de
  commande de tout processus à tout utilisateur de la machine.
  `npm run admin:utilisateur` lit le mot de passe sur **l'entrée standard** et
  **refuse explicitement** un `--mot-de-passe`, avec son motif — assertion de
  test, dont la rouge est de l'accepter.

### ⑧ Les onze divergences E1…E11, tranchées AVANT d'écrire une ligne

Elles sont détaillées avec leur sort dans le §6 du document de résultats. En un
mot chacune : **E1** la garde est un paramètre **REQUIS** de
`createSignalingServer` (jamais optionnelle : un défaut « accepter » rendrait un
service mal câblé indiscernable du bon) ; **E2** la moitié `agent` du trou reste
ouverte, et le critère ① le dit littéralement ; **E3** la **décision**
d'appartenance est prise par un registre pur et synchrone, l'**enregistrement**
par `session.utilisateur_id` ; **E4** CORS naît, avec le refus pour défaut ;
**E5** deux colonnes de plus, sans quoi la détection de rejeu ne protège rien ;
**E6** les trois pilotes CDP **sèment** un jeton obtenu, pas d'interrupteur
permissif ; **E7** `SessionOptions.jeton` est **facultatif**, et `main.ts` n'est
pas touché ; **E8** `scrypt` à `N = 16384` faute de `maxmem` relevé ; **E9** les
longueurs se comparent avant `timingSafeEqual` ; **E10** trois modules absents
de l'arborescence de la spec §5 sont créés et la divergence déclarée ; **E11**
la création de compte se fait par `npm run admin:utilisateur`, mot de passe sur
stdin seul.

### ⑨ La revue transverse de fin de branche

Sa cible propre : **les affirmations devenues fausses dans la branche
elle-même**. Elle a trouvé **5** défauts en D7, **3** en D8, **6** en D9,
**douze** en D10, **huit** en P1, **sept** en D11. Elle trouve ici **DIX affirmations distinctes,
réparties sur VINGT-TROIS PLACES** — plus **une** qui n'est **pas** imputable à
P2 et le dit. Le tableau complet, une ligne par place, avec `fichier:ligne` et
sort, vit au §10 du document de résultats.

⚠️ **Les deux comptes ne mesurent pas la même chose, et il faut les donner
ensemble** : « dix » est ce qu'un relecteur a trouvé, « vingt-trois » est ce
qu'il a fallu ÉDITER — et c'est le second qui coûte, parce que « corrigé à sa
place » est une affirmation de **complétude**. Les barèmes des sous-blocs
précédents comptaient, eux, des **défauts**, pas des places : ils ne se
comparent qu'à la colonne de gauche.

**Ce que la classe a de particulier ici** : une branche qui **ferme** un trou
rend fausses toutes les phrases qui **décrivaient** ce trou — et il y en avait
beaucoup, parce que P1 avait pris soin de le nommer partout. **UNE SEULE
affirmation — « le port délivre des identifiants TURN à quiconque » — occupe
SEPT des vingt-trois places**, et aucune n'était fausse quand elle a été écrite.


⚠️ **Le sort le plus fréquent n'est pas « corrigé » mais « annoté »** : un relevé
de P1 reste **vrai comme histoire**, et le barrer le rendrait faux. Ce sont les
**pronostics** qu'il faut reprendre — « reste VRAI », « c'est P2 », « AVANT que
P2 n'y ajoute » —, jamais les mesures.

🔵 **Une leçon neuve, et elle est petite** : « **une addition de commentaire
peut annuler une extraction** ». La revue a ajouté aux trois pilotes CDP le
`RECETTE_EMAIL` / `RECETTE_MOTDEPASSE` que leur bloc `Usage` avait oublié — et
la première rédaction ramenait `client/verify-webrtc.mjs` **exactement à 497**,
c'est-à-dire au chiffre d'avant l'extraction que P2 venait de payer pour tenir
la porte des 500. Elle a été **resserrée sur place** : 488 → **494**, la somme
reste négative face à 497, et **c'est déclaré plutôt que découvert**.

### ⑩ Le relevé de tailles, PAR LA COMMANDE, après la dernière édition

🔴 **Relevé le 19 août 2026, APRÈS la dernière édition de la ronde — y compris
celles de la revue transverse.** Une table mesurée en début de ronde est fausse
à la fin de la même ronde.

🔴 **ET IL FAUT DIRE À QUEL COMMIT.** L'arbre est partagé avec le chantier E
(microphone), qui a commité **trois fois pendant cette clôture** : `HEAD` est
passé de `f9cc330` à `988e2ee` puis à **`85ed23a`** entre le début et la fin de
la tâche 19. **Le relevé ci-dessous est celui de `85ed23a`**, arbre de travail
portant en outre les éditions non commitées de P2 et un `M agent/src/opus.rs`
du voisin.

**Le dépôt entier ne porte que DEUX fichiers de plus de 500 lignes**, et ce sont
les deux lignes de la dette gelée — `agent/src/encode.rs` **1536** et
`agent/src/windows_source.rs` **630** —, **ni l'un ni l'autre touché par P2**
(aucun commit de P2 ne touche `agent/`). **Aucun fichier de `plateforme/` ni de
`client/src/` ne dépasse 500**, ni ne s'en approche.

**Les plus gros du périmètre de P2** :

| Fichier | Lignes | Marge |
| --- | --- | --- |
| `client/src/main.ts` | **408** | 92 — *et P2 n'y a pas touché (E7)* |
| `client/src/webrtc.test.ts` | **405** | 95 |
| `plateforme/src/signaling/relais.ts` | **310** | 190 |
| `client/src/webrtc.ts` | **300** | 200 |
| `plateforme/src/signaling/server.test.ts` | **272** | 228 |
| `plateforme/src/http/routes-auth.ts` | **241** | 259 |
| `plateforme/src/signaling/garde-fil.test.ts` | **215** | 285 |
| `plateforme/src/http/routes-auth.test.ts` | **215** | 285 |
| `plateforme/src/signaling/resilience.test.ts` | **208** | 292 |

⚠️ **MARGES LES PLUS SERRÉES DU DÉPÔT à cette date, et la deuxième est NEUVE ET
N'EST PAS DE P2** : `agent/src/encode/arret.rs` **500** (marge **0**),
**`agent/src/micro/tests.rs` 497 (marge 3)** — fichier du **chantier E**,
commité pendant cette clôture ❌ **et retombé à 270 quelques heures plus tard,
par une extraction vers `micro/tests_lecteur.rs` (423) jouée dans le commit de
l'addition, sans franchissement ; la marge de 3 n'existe plus** —, `client/verify-webrtc.mjs` **494** (marge
**6**), `agent/src/superviseur/table.rs` **492** (8), `agent/src/capture.rs`
**492** (8).

🔴 **`client/verify-webrtc.mjs` : 497 → 488 → 494, et les trois chiffres
comptent.** La tâche 17 devait y ajouter du code alors qu'il était à **497,
marge 3** ; elle a tenu la porte des 500 **par une EXTRACTION** — `waitForDevtools`
vivait à l'identique dans les **trois** pilotes CDP et vit désormais dans
`client/recette/devtools.mjs` (**27** lignes) —, d'où **488**, somme **−9**.
Puis la **revue transverse** y a ajouté l'invocation manquante et l'a porté à
**494**. **La somme reste négative face à 497**, et ce +6 est **déclaré** : il
est 100 % commentaire, et sa première rédaction ramenait le fichier
**exactement à 497** avant d'être resserrée. **Quatrième fois que ce dépôt écrit
que la marge regagnée par une extraction se reperd si on la traite comme
acquise** — et la première fois qu'elle se reperd le jour même, dans la branche
qui l'a gagnée.

⚠️ **La divergence texte/commande relevée depuis D10 n'est toujours pas
tranchée, et P2 n'en crée pas de seconde** : le § « Portée » énumère des
répertoires (`client/src/` entre autres), la commande, elle, ne filtre que
`node_modules`, les verrous, `dist/`, `testdata/`, `docs/` et `CLAUDE.md` — et
attrape donc `client/verify-webrtc.mjs`, hors de `client/src/`. **Décision de
convention, qui appartient au propriétaire du dépôt.**


### ⑪ Ce que P2 lègue à P3, P4 et P5

**Legs de P1 réglés** : n°1 (l'authentification — **faite, et DE MOITIÉ
SEULEMENT** : voir ①) et n°2 (toute contrainte naît avec sa table — **appliqué**
par `0002-identite.sql`, et **la consigne reste entière pour P4**).

**Ce qui reste dû :**

1. ✅ **P3 — E2 EST FERMÉE, et c'est le fait n°1 du sous-bloc P3** (19 août
   2026). Ce legs disait : « un pair `{"role":"agent"}` obtient toujours des
   identifiants TURN de 86 400 s sans aucune identité, **mesuré** (1
   exécution) ». **Il ne l'obtient plus** : `authentification requise`, et
   **`a reçu ice-config : false`** — mesuré sur le fil, **2 exécutions**
   (`journaux-plateforme-p3/e2-ferme-{1,2}.log`), à opposer au `true` suivi
   d'identifiants TURN de la pièce de P2. C'était bien **P3 seul** : P3 a
   modifié `agent/`, ce que ni P1 ni P2 n'avaient fait.
2. ✅ **P3 — LE PRÉFIXE OPAQUE EXISTE** : 128 bits de `randomBytes` en
   `base64url` (`agents/prefixe.ts`), rendus **durables** par
   `agent_enrole.prefixe_session`. ⚠️ **MAIS IL NE RÈGLE PAS CE QUE CE LEGS LUI
   DEMANDAIT** : il est **par VM**, pas par session, et le registre
   d'appartenance de `signaling/propriete.ts` est toujours **en mémoire**.
   Après un redémarrage, deux clients humains de la MÊME VM retrouvent le même
   préfixe, et `<préfixe>:w-1` redevient revendicable. **Le préfixe ferme la
   devinabilité ENTRE VMs ; il ne ferme pas la revendication AU SEIN d'une
   VM.** Le legs est donc **réduit, pas soldé** — reformulé ici plutôt que
   coché.
3. ✅ **P3 — `session.vm_id` EST RENSEIGNÉE** (`signaling/trace.ts`, divergence
   E10). ⚠️ **La colonne reste nullable, et la raison n'a pas changé** : une
   session sans préfixe ou à préfixe inconnu n'a rien d'honnête à inscrire, pas
   plus qu'une session appariée par un agent seul n'a d'utilisateur.
   **`NOT NULL` serait FAUX, pas seulement coûteux.**
4. ✅ **FERMÉ PAR P4 (20 août 2026) — `session.utilisateur_id` a son lecteur.**
   ~~⛔ **P4 — l'appartenance en base est là, et elle attend son lecteur.**~~
   `session.utilisateur_id` porte l'`id` de l'utilisateur, vérifié sur le
   chemin réel (1 exécution, `critere-3-appartenance-en-base.log`). ~~**C'est ce
   dont P4 a besoin ; personne ne le lit encore.**~~ **`depot/session.ts::
   compterOuvertesDe` la lit, et `GET /vm` en rend le champ `sessions_ouvertes`
   par VM.** ⚠️ **La réserve est dans le NOM du champ, et il faut la garder** :
   il compte des LIGNES non closes, jamais des sessions média vivantes — le
   média survit à un redémarrage du service alors que la ligne est close par le
   balayage, et une ligne ouverte peut correspondre à un pair parti sans que la
   déconnexion ait été vue. C'est `sessions_ouvertes`, pas `sessions_actives`.
5. ⛔ **P5 — le frein sur les routes d'authentification** (son critère ③).
   `/auth/connexion` est ouverte à la force brute, bornée seulement par les
   ~29 ms de `scrypt` et par l'écoute restreinte. **Même raison, même
   fragilité que la fenêtre d'E2.**
6. ⛔ **P5 — TLS, cookies, en-têtes de sécurité**, et la réouverture de
   l'arbitrage `localStorage`.
7. ⛔ **P5 — l'origine unique.** `PLATEFORME_ORIGINE_CLIENT` est facultative
   **précisément parce que** P5 mettra le client et la plateforme derrière un
   proxy inverse, sur la même origine, où aucune valeur n'a de sens. **Ne pas
   la rendre obligatoire sans rouvrir cet arbitrage.**
8. ⛔ **Tous — aucune constante de P2 n'est calibrée** (voir ⑥), et **aucun
   jugement d'usage n'a été porté** : c'est la lacune que ce dépôt traîne
   depuis `BPP_MIN`.
9. ⛔ **Tous — la garde ne couvre que la POIGNÉE DE MAIN.** Revérifier une
   session en cours est un changement de conception, pas un correctif : il
   faudrait décider ce qu'on fait d'un média déjà établi, que le signaling ne
   porte plus.
---

## 🤖 Sous-projet ⑤ Plateforme — sous-bloc P3 : l'identité des agents, et le canal plateforme ↔ agent (19 août 2026)

Résultats complets :
`docs/superpowers/plans/2026-08-19-plateforme-p3-resultats.md`.
Plan : `docs/superpowers/plans/2026-08-19-plateforme-p3.md` (commit `d280745`).
Conception : `docs/superpowers/specs/2026-08-19-plateforme-design.md`
(commit `217a765`), §3.3, §3.4 et §4 « P3 ».
Journaux : `docs/superpowers/plans/journaux-plateforme-p3/` — **47 fichiers
suivis par git** (relevé par `git ls-files`), **tous UTF-8** (vérifié fichier
par fichier par `iconv -f UTF-8 -t UTF-8` : aucun non-UTF-8). **DEUX familles
de lecture**, contrairement à P1 et P2 qui n'en avaient qu'une :

| Famille | Fichiers | Ce qu'il faut faire |
| --- | --- | --- |
| tout le reste — sondes, rouges, témoins, `instrument/`, les `-plat` | **43** | rien : LF, aucune séquence ANSI, `grep`-ables à plat |
| les **quatre** journaux d'agent bruts, `vm-{1,2}-agent-{avec,sans}-identite.log` | **4** | **CRLF et séquences ANSI de `tracing` PRÉSENTES** : `sed 's/\x1b\[[0-9;]*m//g'`, **ou** lire le `-plat` jumeau, versé pour chacun |

⚠️ **Neuf fichiers portent des CRLF** (ces quatre, leurs quatre jumeaux `-plat`,
et `vm-2-pilote.log`) : ils viennent de la VM. Cela ne gêne aucun `grep`.

⚠️ **CONTRAIREMENT À P1 ET P2, CE SOUS-BLOC A EMPLOYÉ LA VM WINDOWS** — la
tâche 23, et elle seule, **hors critère** (spec §4).

**Vingt-cinq tâches, numérotées 1 à 26 sans le 5** : le plan déclare l'absence
plutôt que de renuméroter, « une renumérotation tardive étant exactement le
geste par lequel une référence survit à ce qu'elle désigne ».

### ① Le fait n°1 : E2 est fermée, et c'est mesuré sur le fil

Un pair qui se déclare `{"role":"agent"}` **sans rien présenter** ne reçoit plus
d'identifiants TURN. **La même sonde qu'en P2**, à lire ligne à ligne contre
elle, **2 exécutions** (exéc. 1 = `sqlite`, exéc. 2 = `postgres`,
`e2-ferme-{1,2}.log`) :

```
messages reçus   : ["{\"type\":\"error\",\"reason\":\"authentification requise\",\"motif\":\"jeton-absent\"}"]
a reçu ice-config : false
a été refusé      : true
```

En P2, le même relevé donnait `a reçu ice-config = true`, `a été refusé = false`,
et les identifiants TURN de 86 400 s présents
(`journaux-plateforme-p2/e2-role-agent-toujours-anonyme.log`).

🔴 **Le rôle `agent` exige désormais TROIS choses, pas une** : un jeton, **de
type `agent`** (*claim* `sty`, relevé verbatim sur le fil :
`{"sub":"…","exp":…,"sty":"agent"}`), et dont le **sujet PRÉFIXE** le nom de
session demandé. Sans le *claim* de type, un jeton humain volé ouvrirait un rôle
`agent` et réciproquement — **deux rouges distinctes, une par sens de
confusion**, et pas une seule à deux `expect`, précisément à cause de la leçon
①A-bis de P2.

🔴 **LA COMPATIBILITÉ EST CASSÉE, franchement et sans interrupteur permissif**
(divergence E1) : **un binaire d'agent antérieur à P3 n'établit plus AUCUNE
session**. Le remède est un rebâtissage — `scripts/build-agent.sh`, puis
`scripts/run-agent.sh` avec les deux variables neuves.

### ② Les deux variables d'environnement neuves — les PREMIÈRES du sous-projet ⑤ que `scripts/run-agent.sh` transmet

P1 et P2 n'avaient touché ni `agent/` ni ce script. P3 y ajoute **deux lignes**,
et **par une tâche DÉDIÉE qui ne fait que cela** (tâche 20).

| Variable | Effet |
| --- | --- |
| `AGENT_VM` | le nom de la VM enrôlée |
| `AGENT_SECRET` | le secret d'enrôlement, échangé sur le canal `/agent` contre un **préfixe** et un **jeton d'agent** |

⚠️ **LES DEUX OU AUCUNE**, et l'absence est BRUYANTE — c'est la rouge de la
tâche 20, mesurée sur la VM, **2 exécutions** :

```
WARN agent: AGENT_VM ou AGENT_SECRET absent : aucun enrôlement, donc aucun
jeton d'agent. La plateforme REFUSERA la poignée de main et aucune session
ne s'établira (sous-bloc P3, sans interrupteur permissif).
```

côté service `poignée de main refusée : poignée de main sans jeton sur la
session bureau`, puis **5,1 ms** (exéc. 1) et **8,2 ms** (exéc. 2) plus tard
`connexion de contrôle au signaling perdue`. **Aucune session, aucun enfant
lancé.**

🔴 **La tâche dédiée est ce qui a évité le piège que ce dépôt a payé TROIS
fois** — `SUPERVISEUR` en D1, `MULTIFENETRE_REPRISE` en D2, `AUDIO` en D7 : un
agent qui démarre sans la variable et sans rien signaler. En D7 l'implémenteur
**et** le relecteur avaient vérifié la propriété en traçant le code ; le tracé
était juste, la valeur ne pouvait simplement pas atteindre le processus.

⚠️ **Le coût est nommé** : le secret apparaît en clair dans
`C:\dev\run-agent.ps1`, sur un partage CIFS lisible depuis l'hôte, comme les 57
autres variables. Il n'est **pas** dans `argv` — la leçon de P2 (`ps` expose la
ligne de commande) est respectée. **À rouvrir en P5.**

### ③ Le préfixe : 128 bits, et il ne coûte RIEN à la table d'appariement

**16 octets de `randomBytes` en `base64url` — 22 caractères**, séparateur `:`,
émis par la plateforme à l'enrôlement et durable dans
`agent_enrole.prefixe_session`. `<préfixe>:bureau`, `<préfixe>:w-1`.

**Le fait de conception qui survivra au code** : `signaling/appariement.ts`
**n'a pas gagné une ligne**. Il apparie des NOMS, et un nom préfixé reste un
nom — deux VMs qui ouvraient toutes deux `bureau` cessent de se rencontrer dans
la même entrée **sans qu'une ligne de ce fichier ait bougé**. La propriété
acquise du compteur — il ne recule jamais — n'est pas touchée non plus, et un
test neuf la tient sous préfixe
(`superviseur/table/tests.rs::le_compteur_ne_recule_jamais_meme_sous_un_prefixe`).

⚠️ **La collision avec le format d'identifiant TURN est levée par l'ALPHABET,
pas par chance.** `deriverIdentifiants` compose `` `${expiration}:${session}` ``,
donc `4600:<préfixe>:bureau` — **trois** segments. Le préfixe étant en
`base64url` (`A-Za-z0-9_-`) il **ne peut pas contenir de `:`**, et la première
borne reste non ambiguë. **Figé par un test** :
`expect(username).toBe('4600:AAAAAAAAAAAAAAAAAAAAAA:bureau')`. 🔴 **Aucun coturn
vivant n'a été sollicité** : « coturn coupe sur le premier `:` » reste une
lecture de la convention `use-auth-secret`, **non éprouvée**.

⚠️ **Un préfixe absent vaut le préfixe vide**, ce qui restitue exactement
`bureau` et `w-1` — et c'est éprouvé des deux côtés (`agents/prefixe.ts`,
`client/src/prefixe.ts`, `superviseur/table.rs::nouvelle`).

### ④ Le verdict des quatre critères, avec leur nombre d'exécutions

**Deux exécutions par critère** — exéc. 1 = `sqlite`, exéc. 2 = `postgres`,
déclaré dans l'en-tête de chaque journal. **Aucun taux n'est revendiqué.**

| # | Critère | Verdict | Exéc. |
| --- | --- | --- | --- |
| ① | Le préfixe cloisonne, et un agent refusé n'obtient **aucune** `ice-config` | **TENU** | **2** |
| ② | Les deux refus d'enrôlement sont **indistinguables** | **TENU** | **2** |
| ③ | Une version divergente est refusée **des deux côtés du canal** | **TENU** | **2** |
| ④ | Un agent muet est vu comme tel, et la **transition** est VUE | **TENU** | **2** |

**① en détail** : l'agent P demandant `<Q>:w-9`, une session **vierge**, reçoit
`accès refusé à la session demandée` et **aucune trame `ice-config`** ; sur
`<Q>:bureau`, **occupée**, il reçoit **le même** message — et non « un agent est
déjà connecté ». **La garde tranche AVANT l'appariement**, si bien que la
réponse ne révèle pas si la session existe. ⚠️ Le **journal du service**, lui,
nomme tout, et c'est délibéré.

**② en détail** : les deux refus sont identiques **caractère pour caractère**
(43 caractères, `premier caractère divergent : aucun`), **et leurs fermetures
aussi** — `{"code":1008,"raison":"enrolement"}` des deux côtés. Comparer les
seuls refus aurait laissé passer le même oracle d'énumération sous une autre
forme. ⚠️ **Ce critère compare des MESSAGES, jamais des DURÉES** : l'attaque
temporelle sur l'écart entre « lire une ligne absente » et « vérifier un
`scrypt` » n'est mesurée par rien.

**④ en détail** : `t = vu_a + seuil` rend encore `prete`, `t = vu_a + seuil + 1
ms` rend `injoignable`. **La transition est VUE, pas déduite** — ce que rend
possible l'horloge en paramètre, et la raison pour laquelle
`agents/fraicheur.ts` est **pur**.

### ⑤ Sept rouges, là où le plan en nommait six

La rouge ③ a été jouée **en trois** — une par bout du canal : Rust, puis les
**deux** sens du parseur TypeScript —, pour qu'aucun sens ne reste non éprouvé.
Cinq des sept **mutent du code de production** ; chacune porte son `sha256`
avant et après restauration, **et les deux empreintes sont égales**. Un **second
témoin `verify-all.sh`** a été joué après elles, parce qu'une empreinte ne dit
rien des six autres fichiers.

🔵 **La plus instructive est ③-rust** : la mutation omet `verifie_version` sur
**UNE SEULE** variante (`Refus`), et **un seul test tombe sur DIX-HUIT**
(`test result: FAILED. 17 passed; 1 failed`, relevé verbatim). C'est ce
qui établit que la vérification est branchée **variante par variante** et non
une fois pour toutes.

🔵 **La ①A ne mute RIEN, et c'est ce qui fait sa valeur** : elle est jouée sur le
binaire du sous-bloc **précédent** (`f0b2fca`), où le défaut est réel et non
simulé — `un agent est déjà connecté à la session bureau`, verbatim.

### ⑥ Deux hypothèses non mesurées, tranchées — favorablement

- **Les enrôlements concurrents pour la MÊME VM.** Le superviseur **et chacun
  de ses enfants** ouvrent leur propre canal `/agent` avec le même `AGENT_VM`.
  Le sous-bloc l'avait **déduit d'une lecture de `canal.ts`**. Mesuré :
  **4 canaux de front au banc** (2 exécutions), **3 sur le produit réel**
  (2 exécutions), **même préfixe pour tous, aucun refus, aucun socket fermé**.
  ⚠️ **AUCUN PLAFOND N'A ÉTÉ CHERCHÉ.**
- **L'agent sans secret échoue bruyamment** — voir ②.

### ⑦ Les DEUX défauts que la recette a trouvés, et légués à la clôture

La recette les a relevés **sans les corriger**. Ils le sont désormais.

🔴 **`pg` REND LES `BIGINT` EN CHAÎNE — ET C'EST UN DÉFAUT DE CLASSE**
(commit `373e331`). `LigneAgent.vu_a` est déclaré `number | null` ; `typeof`
rend `number` sous `node:sqlite` et **`string` sous `pg`**, qui rend tout `int8`
en texte. `interroger<T>` faisant un `as T[]`, **aucun typage ne pouvait
l'attraper**.

> ⚠️ **CE N'ÉTAIT PAS UNE COQUILLE DE TYPE, et c'est ce qui en fait le pire
> cas.** `agents/fraicheur.ts::etatDe` survivait **PAR ACCIDENT** — sa
> soustraction convertit l'opérande —, et `depot/jeton.ts` s'en tirait par un
> `Number(...)` local avec un type `number | string`. **Rien ne rougissait**, et
> pourtant tout `+`, tout `===` et tout `>` aurait divergé selon le moteur :
> `'1787136773742' + 90000` vaut une concaténation.
>
> **SEPT colonnes `BIGINT` sont relues par le service** — `vm.vue_a`,
> `session.ouverte_a`, `session.fermee_a`, `utilisateur.cree_a`,
> `agent_enrole.vu_a`, `jeton_rafraichissement.expire_a`,
> `schema_migration.applique_a`. Corrigé **AU PILOTE**, une fois, par
> `pg.types.setTypeParser(INT8)` — pas colonne par colonne.
>
> **ROUGE VUE**, une exécution par moteur, journal versé :
> `expected [ 'vm.vue_a', 'string' ] to deeply equal [ 'vm.vue_a', 'number' ]`.
> ⚠️ **Elle rougit sous `test:postgres` et reste VERTE sous `test:sqlite`** :
> c'est exactement la divergence que la double passe existe pour trouver.
>
> 🔵 **ET LE TEST D'ÉPOQUE DE P1 NE POUVAIT PAS LA VOIR** — il enveloppe chaque
> lecture dans `Number(...)`, ce qui **CONVERTIT la divergence au lieu de la
> mesurer**. `agent.test.ts` faisait de même. **Une conversion défensive dans un
> test est un masque, pas une ceinture** : les assertions sont désormais nues.
>
> La conversion **LÈVE** au-delà de `Number.MAX_SAFE_INTEGER` plutôt que
> d'arrondir en silence. ⚠️ Chemin **non atteignable par le service**
> (`Date.now()` ≈ 1,8e12, quatre ordres de grandeur sous la borne) ; gardé
> quand même, et éprouvé.

🔴 **DEUX TRACES ANNONÇAIENT UNE SESSION QUE LA PLATEFORME REFUSAIT**
(commit `c053fa4`). L'agent écrivait `superviseur enregistré sur la session de
contrôle session="bureau"` à l'**ÉMISSION** de la poignée de main, donc avant
d'apprendre qu'elle est refusée. Aux **deux** exécutions sans secret, la ligne
sort alors qu'**aucune session ne s'établit**, et la connexion tombe 5 ms plus
tard. **Qui la cherche au `grep` pour savoir si une session tient conclut
l'inverse de la vérité.**

> ⚠️ **`agent/src/signaling.rs:70` portait le MÊME mensonge** (« agent
> enregistré auprès du signaling »), que le legs ne nommait pas : **le défaut
> était de forme, pas d'instance.** Les deux disent désormais ce qu'elles
> savent — la déclaration est partie, l'acceptation n'est pas encore connue.
>
> **Et le refus devient observable** : `superviseur/signalisation.rs` classait
> le `{"type":"error",…}` du relais dans son bras `Err(_) => debug!`, avec
> `ice-config` et `peer-gone` — donc **invisible sous `RUST_LOG=info`**. Le seul
> signe restant était « connexion de contrôle au signaling perdue », qui se lit
> comme une panne réseau et non comme un refus.
>
> ⚠️ **CE BRAS N'A PAS TOURNÉ SUR LA VM** : `#[cfg(windows)]`, hors de portée de
> tout test d'hôte, et les recettes étaient jouées. Vérifié par
> `cargo check --target x86_64-pc-windows-gnu` et `cargo test --workspace`
> (550 + 52, inchangé). **Déclaré plutôt que dissimulé.**

### ⑧ La corroboration sur VM réelle — HORS CRITÈRE, 2 exécutions

Binaire rebâti **9 620 480** octets, contre **9 401 856** pour celui que P3 rend
périmé. `cargo clean --release -p proto -p agent` a dû précéder la compilation :
P3 modifie `proto`, et l'horloge de la VM avance sur celle de l'hôte, ce qui
fait sauter le rlib à cargo.

| | exéc. 1 | exéc. 2 |
| --- | --- | --- |
| `agent enrôlé auprès de la plateforme` | **3** | **3** |
| préfixes distincts délivrés | **1** | **1** |
| sessions d'agent acceptées | **3** | **3** |
| `ERROR` ou `WARN`, phase « avec identité » | **0** | **0** |

Les **trois** lignes de `session` portent leur `vm_id`, résolu depuis le
**préfixe** du nom de session.

⚠️ **CE QUE CETTE CORROBORATION N'EST PAS** : la page-shell est **scriptée**
(elle signe elle-même son jeton), **aucune session WebRTC n'est négociée**,
aucune image décodée, aucune latence mesurée, aucun `coturn` ne tournait. Ce qui
est établi est l'**enrôlement**, le **préfixe** et l'acceptation des **poignées
de main**.

⚠️ **Une troisième tentative a AVORTÉ et n'est pas versée** : la VM s'était
éteinte d'elle-même (piège documenté depuis D1), et le port 8080 était tenu par
un service de plateforme d'une recette antérieure. **Dit plutôt que tu.**

### ⑨ Ce que P3 n'établit PAS

- **Aucun taux, nulle part.** Deux exécutions par critère ; jamais une campagne.
- 🔴 **La reprise du canal `/agent` (E9) n'a JAMAIS été exercée** : aucune
  coupure n'a été provoquée, ni au banc ni sur la VM. Le calcul du délai est pur
  et testé ; le comportement du socket, non. **C'est du comportement NEUF que ni
  `signaling.rs` ni `signalisation.rs` n'avaient**, et il n'a jamais couru.
- **Le bras `error` neuf de `superviseur/signalisation.rs` n'a pas tourné sur la
  VM.**
- **Aucun plafond d'enrôlements concurrents cherché** : 4 au banc, 3 sur le
  produit.
- **Aucun `coturn` vivant** (E6).
- **Aucune session WebRTC négociée sur la VM**, aucune image, aucune latence.
- **Aucune constante calibrée** : `SEUIL_INJOIGNABLE_MS`, `REPLI_MIN_MS`,
  `REPLI_MAX_MS`, `OCTETS_PREFIXE`, plus celles de P1/P2. **Aucun jugement
  d'usage** — la lacune que ce dépôt traîne depuis `BPP_MIN`.
- **Le déni de service en une trame reste OUVERT** (E15) : le contrôle de forme
  court **avant** toute garde, et **aucune authentification ne peut fermer ce
  chemin-là**. Le frein est P5 ③.
- **Aucune attaque temporelle mesurée.**
- **Aucune revérification d'une session en cours** — legs n°9 de P2, reconduit.
- **Rien du comportement d'un agent qui perd son canal pendant une session
  établie.**
- **`vm.vue_a` reste une colonne ORPHELINE** : P3 a créé `agent_enrole.vu_a` et
  n'écrit jamais dans `vm.vue_a` (relevé par `grep -rn "vue_a" plateforme/src` :
  seuls des tests l'écrivent).
- ~~**`application` reste VIDE** : son chemin d'écriture est le sous-projet ④.~~
  ✅ **CLOS par G1 (20 août 2026) — c'est-à-dire par ④ lui-même.** `agents/canal.ts`
  l'écrit à chaque message `catalogue`, via `apps/catalogue.ts::fusionner` et
  `depot/application.ts::appliquer`. **154 lignes** pour la VM de développement.
- **Aucune protection du secret d'enrôlement sur la VM** (voir ②).

### ⑩ Les seize divergences E1…E16, tranchées AVANT d'écrire une ligne

P1 en avait six, P2 onze, P3 **seize** — chacune avec le relevé qui la fonde. Le
détail et le sort de chacune vivent au §7 du document de résultats. **Les cinq
qui gouvernent** :

- **E1 — P3 casse la compatibilité, et la spec §10.3 ne le disait pas.** Ses
  deux phrases sont vraies du **PRÉFIXE** et fausses de l'**AUTHENTIFICATION**.
  ⚠️ **La spec ne se contredit pas pour autant** : sa conclusion (« sans
  enrôlement valide, aucune session ne s'établit du tout ») reste juste ; **c'est
  sa phrase du milieu qui a vieilli.**
- **E4 — la garde de P2 est PURE et SYNCHRONE : l'enrôlement ne peut pas y
  vivre.** Vérifier un secret haché exige une lecture de base, donc un `await`,
  **sur le chemin de la poignée de main**. D'où le canal `/agent` **hors du
  relais**, dans `http/serveur.ts`, sur son **propre** `WebSocketServer`. **La
  garde ne gagne pas une ligne d'accès à la base.**
- **E5 — jeton d'agent et jeton humain sont signés par le MÊME secret** : sans
  *claim* de type, ils sont interchangeables **dans les deux sens**. L'absence du
  *claim* vaut `'utilisateur'`, de sorte qu'aucun jeton émis par P2 et encore en
  vol ne soit invalidé.
- **E8 — `proto/vectors.json` est structuré pour `input` SEUL**, et sa clé
  `version` n'est vérifiée **que du côté TypeScript**. D'où un fichier **neuf**,
  `proto/plateforme-vectors.json`, dont la version est vérifiée **des deux
  côtés** — la lacune est corrigée **pour le fichier neuf**, sans réécrire
  rétroactivement un test qui n'appartient pas à P3.
- **E9 — aucun des deux clients WebSocket de l'agent n'a de reprise**, et le
  canal `/agent` en exige une : il porte le battement, donc `vu_a`. Sans reprise,
  **la première coupure réseau rendrait la VM `injoignable` définitivement**, et
  le critère ④ punirait une coupure de réseau comme une panne d'agent. ⚠️ **Un
  refus `version` ne se réessaie PAS** — une incompatibilité de version qui se
  déguiserait en boucle de reconnexion infinie est le mode de panne le plus
  coûteux à diagnostiquer.

### ⑪ La revue transverse de fin de branche — DOUZE défauts, QUINZE places

Barème : **5** en D7, **3** en D8, **6** en D9, **douze** en D10, **sept** en
D11, **huit** en P1, **dix** en P2. **Douze ici, sur quinze places**, toutes
énumérées par `grep -n` **avant** l'édition et relues place par place **après**.
Le tableau complet vit au §8 du document de résultats. **Les trois qui
enseignent :**

1. 🔵 **UNE CITATION `fichier:ligne` A ÉTÉ RENDUE FAUSSE PAR LA BRANCHE
   ELLE-MÊME.** `signaling/appariement.ts:50` cite le pair qui lit le motif de
   refus : `agent/src/signaling.rs:130`. **C'était juste**, et la divergence E11
   du plan le déclarait en toutes lettres « relu et juste » — puis le commit
   `5fbc89b` de cette branche, celui qui met le jeton dans les poignées de main,
   l'a poussé à **139**.
   **Leçon neuve, que ce dépôt n'avait pas formulée : relire une citation avant
   d'écrire ne suffit pas quand le plan prescrit par ailleurs de DÉPLACER la
   ligne citée. Il faut la relire APRÈS avoir exécuté ce qui la déplace.**
   *(La seconde citation de la même ligne, `client/src/webrtc.ts:109`, était
   fausse depuis P2 — la l. 109 est **vide**.)*
2. 🔴 **LE NAUFRAGE DU « 487 », À L'INTÉRIEUR DU COMMIT QUI LE DÉNONÇAIT.**
   `0001-socle.sql` disait encore « `vm_id`, lui, reste entièrement vide : c'est
   P3 » — alors que P3 la remplit. Le commit `254fdd5` de cette même branche a
   réécrit les **six** lignes qui précèdent **sans balayer les deux suivantes**,
   dans un message de commit qui revendiquait d'avoir « énuméré les places par
   `grep` AVANT d'écrire ».
3. 🔵 **« DIX » ET « DIX-SEPT » SONT VRAIS DE DEUX CHOSES DIFFÉRENTES.** Le
   tableau S1 de ce fichier annonçait « neuf → dix étapes », le plan de P3
   « ses neuf étapes ». **Mesuré** sur une exécution complète, sortie **0** :
   **DIX** appels `etape` dans le script, et **DIX-SEPT** en-têtes `==>` à
   l'écran — les sept derniers venant de l'intérieur de l'étape
   `client : npm run design:verifier` (un `npm run build`, plus les **six**
   contrôles du socle ; le septième, §7.5, est un test unitaire et tourne dans
   `client : npm test`). **Le plan disait « neuf », ce qui est faux des deux
   façons de compter.**

🔵 **Et une mesure de doctrine que ce dépôt n'avait pas** : l'en-tête de
`plateforme/src/signaling/relais.ts` a été corrigé **TROIS FOIS, une par
sous-bloc** — P1, P2, P3 —, et **chaque correction a laissé derrière elle une
« moitié qui reste vraie » que la suivante a dû reprendre**. La durée de vie
d'un « reste VRAI » dans ce dépôt est d'**UN sous-bloc**.

⚠️ **Relevé, NON corrigé, hors périmètre** :
`docs/superpowers/specs/2026-08-19-design-system-design.md:145` affirme « aucun
jeton dans la poignée de main » sur `client/src/shell-page.ts:45`. **Faux depuis
P2** (le fichier envoie bien `jeton`), et le numéro vaut **70**. C'est une
affirmation du sous-projet ⑥ ; **laissée à son propriétaire.**

### ⑫ Le relevé de tailles, PAR LA COMMANDE, APRÈS la dernière édition

Relevé au commit de clôture, **après** les deux correctifs des défauts légués
**et** la revue transverse — une table mesurée en début de ronde serait fausse à
la fin de la même ronde (erreur de D8).

**Le tableau de dette est INCHANGÉ, et il a toujours DEUX lignes** :
`agent/src/encode.rs` **1536**, `agent/src/windows_source.rs` **630**. **Aucun
autre fichier de code source ne dépasse 500 lignes.**

**59 fichiers touchés par les commits `(p3)`.** Les plus gros, et ceux dont la
marge bouge :

| Fichier | Avant P3 (`d280745`) | Après | Remarque |
| --- | --- | --- | --- |
| ⚠️ `agent/src/demarrage.rs` | 481 | **491** (marge **9**) | ❌ **CE +10 N'EST PAS DE P3, et je l'ai d'abord écrit tel quel avant de le mesurer.** `git log --numstat` : le chantier **E1** y met **+10/−0** (`f2f8e05`, le puits de micro), P3 **+1/−1**, soit **net zéro**. Le plan de P3 disait « rien si le câblage passe par `superviseur.rs` et `main.rs` — **à re-mesurer avant la tâche 19** » : **c'était juste**. La marge de 9 est **déjà signalée par la section du chantier E** ci-dessus. **Toute addition future appelle une extraction** |
| ✅ `agent/src/superviseur/table.rs` | **492** (marge 8) | **433** (marge 67) | **L'EXTRACTION A ÉTÉ FAITE AVANT L'ADDITION** (tâche 17, `aa220ba`), et elle a tenu : `table/orphelines.rs` (**121**) est né, puis la tâche 19 y a écrit son préfixe sans franchir quoi que ce soit. C'est le geste que **D9 (tâche 6) a inventé** et que **D10 a joué trois fois** (ses tâches 1 à 3) ; P3 le reconduit. ❌ *Une première rédaction écrivait ici « deuxième fois seulement dans ce dépôt » : c'est FAUX, et la place qui le réfute est la section D10 de ce fichier même, qui compte trois extractions jouées avant leurs additions.* |
| `agent/src/main.rs` | 329 | **435** | dont **+77 net par P3** (`AGENT_VM`, `AGENT_SECRET`, le câblage de l'enrôlement) et **+29 par les chantiers A-bis et E1** — attribution relevée par `git log --numstat`, pas déduite de l'écart |
| `proto/src/plateforme.rs` | — | **410** | neuf — `verifie_version` **variante par variante** |
| `plateforme/src/agents/canal.test.ts` | — | **361** | neuf |
| `plateforme/src/signaling/relais.ts` | 310 | **327** | +17, presque entièrement du commentaire |
| `agent/src/plateforme.rs` | — | **277** | neuf — le client du canal, avec sa reprise |
| `proto/ts/plateforme.test.ts` | — | **267** | neuf |
| `plateforme/src/base/pilotes.test.ts` | 141 | **251** | +110 : les deux tests du défaut `BIGINT` |
| `proto/ts/plateforme.ts` | — | **207** | neuf |
| `plateforme/src/agents/canal.ts` | — | **198** | neuf |
| `plateforme/src/admin/enroler-agent.ts` | — | **162** | neuf |
| `agent/src/superviseur/table/orphelines.rs` | — | **121** | neuf — l'extraction de la tâche 17 |
| `plateforme/src/signaling/appariement.ts` | 97 | **120** | +23, entièrement du commentaire de revue transverse |
| `plateforme/src/base/pilote-postgres.ts` | 76 | **118** | +42 : le `setTypeParser` et sa justification |
| `agent/src/plateforme/repli.rs` | — | **89** | neuf — **pur** |
| `plateforme/src/agents/fraicheur.ts` | — | **76** | neuf — **pur** |
| `proto/plateforme-vectors.json` | — | **73** | neuf |
| `client/src/prefixe.ts` | — | **72** | neuf — **pur, sans DOM** |
| `plateforme/src/agents/prefixe.ts` | — | **68** | neuf — **pur** |
| `plateforme/src/agents/enrolement.ts` | — | **65** | neuf |
| `plateforme/src/base/migrations/0003-agents.sql` | — | **58** | neuf — `agent_enrole` **et** `application` |

⚠️ **`agent/src/transport.rs` est à 495 lignes, marge 5** — **la deuxième plus
serrée du dépôt** après `encode/arret.rs` (500, marge 0), devant
`client/verify-webrtc.mjs` (494, marge 6). Le plan de P3 le relevait à **491**.
❌ **Une première rédaction ajoutait ici « et qu'aucun document ne signalait » :
c'est FAUX, et la place qui le réfute est dans CE fichier** — la section du
chantier **E1** le déclare en toutes lettres, avec son attribution
(469 → 491 par E1, → 495 par sa propre revue transverse) et l'injonction
d'extraction. **P3 n'y a pas touché, et n'a rien à y corriger** ; la ligne reste
ici parce qu'un lecteur de la section P3 doit connaître la marge dont il
dispose, pas parce qu'elle serait neuve.

**Témoin de clôture** : `./scripts/verify-all.sh` relancé **après la dernière
édition**, **sortie 0** — `cargo test --workspace` **550** + **52**,
`client : npm test` **187**, `design:verifier` **6/6**, `proto : npm test`
**70**, `plateforme` **196** sur `sqlite` **et** sur `postgres`, trois
`typecheck` à 0. Journal versé :
`journaux-plateforme-p3/temoin-verify-all-cloture-finale.log`.
⚠️ **`plateforme` passe de 194 à 196, et les +2 étaient ANNONCÉS avant d'être
lus** : ce sont les deux tests neufs du défaut `BIGINT`.

### ⑬ Pièges neufs — à connaître avant de toucher à ce terrain

- 🔴 **Une conversion défensive dans un TEST masque un défaut au lieu de le
  mesurer.** Le test d'époque de P1 enveloppe chaque lecture de `BIGINT` dans
  `Number(...)` ; il ne pouvait **structurellement pas** voir que `pg` rend une
  chaîne. Un test qui normalise ce qu'il éprouve n'éprouve plus rien.
- 🔴 **Un type déclaré sur un retour de base n'est pas vérifié par le
  compilateur** : `interroger<T>` fait un `as T[]`. **`T` est une affirmation,
  pas une contrainte** — elle se tient par un test, ou pas du tout.
- 🔴 **Une trace émise à l'envoi ne dit rien du verdict.** Deux traces `info`
  annonçaient une session enregistrée avant que la plateforme n'ait répondu, et
  sortaient telles quelles quand elle refusait. **Une trace doit dire ce qu'elle
  SAIT à l'instant où elle sort.**
- 🔴 **Un `match` catch-all qui range un refus avec des messages inintéressants
  le rend invisible.** Le `{"type":"error"}` du relais tombait dans le même
  `debug!` qu'`ice-config` et `peer-gone` : sous `RUST_LOG=info`, un refus de
  poignée de main se lisait comme une panne réseau.
- ⚠️ **Un `grep -n` avant édition ne suffit pas si le chantier DÉPLACE la ligne
  citée** — voir ⑪ n°1. La relecture doit être **postérieure à l'exécution**.
- ⚠️ **Un compte d'étapes peut être vrai de deux façons** : dix `etape`,
  dix-sept en-têtes. **Dire lequel on compte.**
- ⚠️ **Un service de recette laissé vivant tient un port pendant des heures** et
  fait échouer la mesure suivante sans rien dire : une exécution de la
  corroboration VM a été perdue ainsi (port 8080, cinq heures d'ancienneté),
  cumulée avec la VM éteinte d'elle-même. **Vérifier le port ET
  `Get-Process agent` avant chaque tentative.**
- ⚠️ **`cargo clean --release -p proto -p agent` est obligatoire quand P3
  modifie `proto`** : l'horloge de la VM avance sur celle de l'hôte, et cargo
  saute la reconstruction du rlib. **Vérifier la TAILLE du binaire** — 9 620 480
  contre 9 401 856.

### ⑭ Ce que P3 lègue à P4 et à P5

**Legs de P1 réglés** : n°1 (l'authentification, **entièrement** cette fois),
n°3 (`session.vm_id`), n°4 (observer les agents — ⚠️ **mais pas sur la colonne
que le pronostic nommait** : `agent_enrole.vu_a`, pas `vm.vue_a`), n°5 (le canal
`/agent` — ⚠️ **mais pas au lieu que le pronostic nommait** : `http/serveur.ts`,
pas `relais.ts`, et c'est E4 qui l'impose).
**Legs de P2 réglés** : n°1 (E2), n°3 (`session.vm_id`). ⚠️ **Le n°2 est
RÉDUIT, pas soldé** — voir le point 4 ci-dessous.

**Ce qui reste dû :**

1. ✅ **FERMÉ PAR P4 (20 août 2026).** ~~⛔ **P4 — la route qui rend un préfixe
   à un navigateur.**~~ `POST /session` la rend, `client/src/connexion.ts`
   l'appelle une fois le jeton posé, et `poserPrefixe` l'écrit au coffre.
   Corroboré sur un VRAI navigateur, en origine croisée
   (`journaux-plateforme-p4/corroboration-navigateur.log`), et sur le service
   réel de la VM (`vm-corroboration-releve.log`). ⚠️ **LE COÛT DE LA SPEC §10
   EST RÉDUIT, PAS SOLDÉ, et il faut dire par quoi** : deux gardes, aux deux
   bouts — `poserPrefixe` **LÈVE** sur la chaîne vide plutôt que de la coucher
   au coffre, et la route rend **409 `aucune-vm`** au lieu d'un 200 à préfixe
   vide. Ce qui reste ouvert est le point 4 ci-dessous, que P4 ne touche pas.
2. ✅ **FERMÉ PAR P4 (20 août 2026).** ~~⛔ **P4 — `agents/fraicheur.ts` n'a
   AUCUN appelant de production.**~~ `orchestration/inventaire-statique.ts::etat`
   l'appelle, et **les deux routes de P4 le lisent** — `GET /vm` pour l'état de
   chaque VM, `POST /session` pour décider entre 200 et 503. L'orchestrateur lui
   donne son horloge, qui reste un **paramètre** : c'est ce qui rend la
   transition du critère ④ observable à la milliseconde près, borne assiégée des
   deux côtés (`critere-4-{1,2}.log`, 2 exécutions).
3. ⚠️ **À MOITIÉ FERMÉ PAR P4 (20 août 2026).** ~~⛔ **P4 —
   `session.utilisateur_id` attend toujours son lecteur.**~~ Elle a le sien
   (`compterOuvertesDe`, voir le legs n°4 de P2 et sa réserve de nom). ~~**Mais
   `application` attend toujours son écrivain** — sous-projet ④, qui empruntera
   le canal `/agent`, et P4 ne l'approche pas : sa table reste vide.~~
   ✅ **ELLE L'A, depuis G1 (20 août 2026).** ④ a emprunté le canal comme
   annoncé : `agents/canal.ts` applique `apps/catalogue.ts::fusionner` à chaque
   message `catalogue` de l'agent. **Ce legs est CLOS.**
4. 🔴 **P4 ou P5 — le préfixe ne ferme PAS la revendication au sein d'une VM.**
   Il est **par VM**, et le registre d'appartenance de `signaling/propriete.ts`
   est **en mémoire** : après un redémarrage du service, deux clients humains de
   la même VM retrouvent le même préfixe et `<préfixe>:w-1` redevient
   revendicable. **Le préfixe ferme la devinabilité ENTRE VMs ; il ne ferme pas
   la revendication AU SEIN d'une VM.**
5. ⛔ **P5 — le frein sur `/auth/connexion` ET sur `/agent`.** Les tentatives de
   secret d'enrôlement ne sont bridées par **rien** : même fragilité que
   `/auth/connexion`, sur un chemin neuf.
6. ⛔ **P5 — le secret d'enrôlement en clair sur la VM**, nommé d'avance par la
   décision D1 du plan et explicitement renvoyé à P5.
7. ⛔ **P5 — TLS, cookies, en-têtes de sécurité, `/sante`, `coturn` restreint**,
   et le déni de service en une trame (E15).
8. ⛔ **Tous — la reprise du canal `/agent` n'a JAMAIS été exercée** (E9). Sans
   elle, une coupure réseau rendrait une VM `injoignable` définitivement — la
   raison même pour laquelle elle existe, et rien ne l'a éprouvée.
9. ⛔ **Tous — la garde ne couvre que la POIGNÉE DE MAIN.** Legs n°9 de P2,
   reconduit sans changement.
10. ⚠️ **Hors P3 — `agent/src/transport.rs` est à 495 lignes, marge 5**, portée
    là par le chantier E1, **qui la signale déjà lui-même**. Reprise ici pour
    mémoire, non corrigée : hors périmètre.
11. ⚠️ **Hors P3 — `docs/…/2026-08-19-design-system-design.md:145`** affirme
    « aucun jeton dans la poignée de main » sur `shell-page.ts:45`. Faux depuis
    P2, et le numéro vaut 70. Laissé à son propriétaire.

---

## 🖥️ Sous-projet ⑤ Plateforme — sous-bloc P4 : l'orchestration, et la première VM qui appartient à quelqu'un (20 août 2026)

Résultats complets :
`docs/superpowers/plans/2026-08-19-plateforme-p4-resultats.md`.
Plan : `docs/superpowers/plans/2026-08-19-plateforme-p4.md` (commit `7523ea5`).
Conception : `docs/superpowers/specs/2026-08-19-plateforme-design.md`
(commit `217a765`), §3.6 et §4 « P4 ».
Journaux : `docs/superpowers/plans/journaux-plateforme-p4/` — **43 fichiers
suivis par git** (relevé par `git ls-files … | wc -l` à la clôture), dont **9**
d'instrument. **DEUX familles de lecture**, et c'est la répartition la plus
simple qu'ait connue ce dépôt :

| Famille | Fichiers | Ce qu'il faut faire |
| --- | --- | --- |
| tout le répertoire | **41** | rien : aucune séquence ANSI, `grep`-ables à plat |
| `vm-1-agent.log`, `vm-2-agent.log` | **2** | **séquences ANSI de `tracing` PRÉSENTES** : `sed 's/\x1b\[[0-9;]*m//g'`, **ou** lire le `-plat` jumeau, versé pour chacun |

⚠️ **Les CRLF et les séquences ANSI ne coïncident PAS, et le dire évite une
règle fausse** : `grep -rlP '\x1b\['` rend **deux** fichiers (les deux
ci-dessus), `grep -rlU $'\r'` en rend **six** — les deux bruts, leurs deux
jumeaux `-plat`, et les deux `vm-{1,2}-corroboration.log`. Tous les six
viennent de la VM. **Les CRLF ne gênent aucun `grep`** ; seules les séquences
ANSI le font.

⚠️ **P4 n'a employé la VM Windows que pour sa tâche 16, hors critère.** Les
quatre critères sont tenus sans elle.

**Convention des exécutions, reconduite de P2 et P3** : **exécution 1 =
`sqlite`, exécution 2 = `postgres`**, déclaré dans l'en-tête de chaque journal.
**Aucun taux n'est revendiqué nulle part.**

### ① Le fait n°1 : `vm.utilisateur_id` a enfin son écrivain ET son lecteur filtrant

C'était le point le plus lourd de toute la plateforme, et le plan le nommait
comme tel avant dispatch : « **aucun code de production ne lit ni n'écrit
`vm.utilisateur_id`** ». La colonne existait depuis P1 avec son index unique
partiel, et personne ne s'en servait — c'est-à-dire qu'**il n'y avait aucune
isolation entre utilisateurs** : n'importe quel compte authentifié aurait vu
n'importe quelle VM, s'il avait existé une route pour les lister.

Les deux bouts existent désormais, éprouvés de bout en bout :

- **l'écrivain** — `npm run admin:attribuer -- --email <courriel> --vm <nom|id>`,
  plus `--detacher`. Il passe par `orchestration/inventaire-statique.ts::
  attribuer`, **jamais par un `UPDATE` écrit à part** : dupliquer l'ordre
  « lire, écrire sous clause, traduire l'exception » ferait diverger les deux
  chemins le jour où l'un changerait, et la commande d'administration est
  précisément celle qu'on relit le moins souvent ;
- **le lecteur filtrant** — `orchestration/selection.ts::vmsDe`, **pur**, sur
  lequel `GET /vm` et `POST /session` s'appuient tous deux.

🔴 **`attribuer` N'EST PAS EXPOSÉE SUR HTTP, et c'est une décision, pas un
oubli.** Relevé : `identite/jeton.ts` ne connaît que `'utilisateur' | 'agent'`,
et `config.ts` n'a aucune variable d'administrateur — **il n'existe aucun rôle
d'administration dans ce service**. Une route qui attribuerait une VM aurait
donc été, au mieux, ouverte à tout utilisateur authentifié : une escalade de
privilège offerte. **Un test assère nommément qu'`attribuer` ne figure pas dans
la liste blanche des opérations HTTP** — sans lui, l'y ajouter un jour de
fatigue ouvrirait l'attribution à tout le monde sans qu'aucun test ne bouge.

### ② Le verdict des quatre critères, avec leur nombre d'exécutions

**Les quatre sont TENUS, DEUX exécutions chacun.** Aucune assertion n'est
tombée, sur aucun des deux moteurs.

| # | Critère | Verdict | Exéc. |
| --- | --- | --- | --- |
| ① | `instantane` refuse explicitement, en **501**, et le **journalise** | **TENU** | **2** |
| ② | Deux utilisateurs ne partagent pas une VM, un utilisateur n'en a pas deux, **et jamais un 500** | **TENU** | **2** |
| ③ | Un utilisateur sans VM reçoit un refus **immédiat** | **TENU** | **2** |
| ④ | Une VM dont l'agent n'a pas été vu est **annoncée injoignable**, et l'API **avoue** ne pas savoir la redémarrer | **TENU** | **2** |

**① en détail** : `501 {"motif":"non-supporte","operation":"instantane",
"backend":"inventaire-statique"}`, et **exactement une** ligne de journal, qui
nomme l'opération **et** le backend. Le `501` n'est pas écrit à la main dans la
route : il est lu dans `CODE_HTTP`, un `Record<Motif, number>` dont la clé est
l'union **dérivée** du tableau `as const` `MOTIFS`. **Ajouter un motif sans lui
donner son code HTTP est une erreur de compilation.** Et le sens de la
dérivation compte : le tableau produit le type, si bien que la liste
d'exécution et la liste de types sont **le même objet**, non deux objets qu'on
espère égaux — le remède structurel au catch-all silencieux que ce dépôt a payé
quatre fois (`pont_media.rs`, D5 à D8).

**③ en détail — la borne est MESURÉE avant d'être fixée** : 100 refus
consécutifs par moteur (`mesure-borne-3.log`). Premier appel **à froid** 78,7 ms
(sqlite) et 39,9 ms (postgres) ; **maximum des 99 suivants** 7,5 et 12,9 ms.
**Borne retenue : 250 ms**, et **ce n'est PAS le p99 du relevé** — calée sur
lui, elle rougirait au premier ralentissement de la machine et transformerait
le critère en détecteur de charge d'hôte.

**④ en détail — la transition est ASSIÉGÉE des deux côtés, à la milliseconde** :
`t = vu_a + SEUIL` (90 000 ms) rend encore `prete`, `t + 1 ms` rend
`injoignable`. Une VM **jamais vue** est `injoignable`, jamais « peut-être ».
🔴 **`redemarrage` est l'aveu, pas la fonction** : le cadrage promet « VM
injoignable → le hub l'indique, **propose redémarrage** » ; avec le backend v1
le hub **indique** et **dit qu'il ne sait pas redémarrer**. Le champ est rendu
plutôt que laissé au navigateur à deviner, **parce qu'une absence de champ se
lit comme un oubli**. Et **le préfixe est rendu QUAND MÊME sur le 503** : il est
connu et juste, et le navigateur en a besoin pour ne pas rejoindre l'espace de
noms partagé en attendant que la VM revienne.

### ③ 🔴 La ROUGE que la spec prescrivait pour le critère ② ne rougissait PAS ce que le critère énonce

**C'est le fait de conception le plus réutilisable du sous-bloc, il a été
établi AVANT dispatch, par une sonde, et il réfutait trois documents à la
fois.**

La spec §3.2, sa §4 « P4 » et `0001-socle.sql:53-56` disaient tous trois que
l'index unique partiel `vm_un_utilisateur` établissait « deux utilisateurs ne
peuvent pas recevoir la même VM », et prescrivaient comme ROUGE « retirer
l'index partiel ». **C'est faux.** `CREATE UNIQUE INDEX vm_un_utilisateur ON
vm(utilisateur_id) WHERE utilisateur_id IS NOT NULL` rend `utilisateur_id`
unique **à travers les lignes** : il interdit qu'**un utilisateur ait deux
VMs**. Il n'interdit **rien** à `UPDATE vm SET utilisateur_id='bob' WHERE
id='v1'` quand `v1` est à alice — **une VM n'a qu'un `utilisateur_id`, et
l'écraser ne viole aucune unicité.**

**MESURÉ, index INTACT** (`rouge-2a-vol-sans-clause-conditionnelle.log`) : il a
suffi de retirer la clause `AND utilisateur_id IS NULL` de
`depot/vm.ts::attribuerSiLibre` pour que le vol réussisse — `lignes touchées
par l'UPDATE de vol : 1`, propriétaire changé —, sur **SQLite 3.50.4** comme
sur **PostgreSQL 16.15**, une exécution par moteur.

Le critère se scinde donc en **trois** propriétés, à **trois** gardes :

| | Propriété | Garde | Rouge |
| --- | --- | --- | --- |
| ②a | une VM n'est attribuée qu'une fois | la clause `AND utilisateur_id IS NULL` | retirer la clause |
| ②b | un utilisateur ne reçoit qu'une VM | l'index partiel, qui **lève** | retirer l'index de `0001-socle.sql` |
| ②c | la violation est traduite en refus **typé**, jamais un 500 | la **relecture** après exception | laisser l'exception remonter |

🔴 **LES DEUX MOTEURS NE LÈVENT PAS LE MÊME TEXTE** — `UNIQUE constraint
failed: vm.utilisateur_id` contre `duplicate key value violates unique
constraint "vm_un_utilisateur"`, les deux relevés. **Le code ne compare donc
JAMAIS le message de l'exception** : il **relit** l'état et ne traduit que ce
que la relecture explique ; **sinon il RELANCE**. Un `catch` qui traduirait
*toute* exception avalerait une base injoignable et la présenterait comme un
refus métier — la panne muette exacte que la spec §6 interdit. Un `Pilote`
factice dont `executer` lève une erreur étrangère éprouve ce point à part.

⚠️ **Un contrôle atteste que la relecture a EU LIEU, pas seulement que le motif
est juste** : `lectures faites par la course : 2`. Un code qui devinerait le
motif rendrait **1**, et la rouge ②a le montre.

**Les trois places de l'attribution fausse sont annotées à leur place**
(`0001-socle.sql`, spec §3.2, spec §4 « P4 »), pas seulement là où on nous
l'avait montrée.

### ④ 🔴 Trois contrôles vacueux attrapés en chemin — dont un d'une espèce NEUVE

Ce dépôt tient une doctrine : *un contrôle qu'on n'a jamais vu rouge n'est pas
un contrôle*. P4 en a attrapé **trois** qui la violaient, chacun d'une espèce
différente, et **les trois par l'exécution, jamais par la relecture**.

- **(a) Un `toThrow()` NU, vert alors que la fonction n'existait pas.** Le test
  appelait une fonction absente ; `expect(() => …).toThrow()` attrapait le
  `ReferenceError` et se déclarait satisfait. **Un `toThrow()` doit nommer ce
  qu'il attend.**
- 🔵 **(b) UNE ROUGE RESTÉE VERTE PARCE QUE LA CHAÎNE À MUTER APPARAISSAIT
  D'ABORD DANS LE COMMENTAIRE QUI LA JUSTIFIE.** La mutation devait retirer
  `AND utilisateur_id IS NULL` du SQL ; la substitution a frappé la **première**
  occurrence, qui était dans la phrase française expliquant pourquoi la clause
  est là. **Le code est resté intact, et le contrôle est resté vert.** Ce qui
  l'a attrapé n'est pas une relecture mais un **garde** ajouté à l'instrument :
  *une rouge doit produire une sortie non vide*, et un diff vide est un échec
  de la rouge, jamais un succès du produit.
  > ⚠️ **LEÇON NEUVE, ET ELLE VISE CE DÉPÔT EN PARTICULIER** : dans un dépôt
  > qui commente abondamment ses invariants, **une mutation par substitution de
  > chaîne frappe le commentaire AVANT le code**. Plus un invariant est bien
  > documenté, plus sa rouge est fragile. **Muter par numéro de ligne, ou par
  > un motif ancré sur la syntaxe — jamais par la seule sous-chaîne.**
- **(c) Une mutation restée verte a révélé qu'une clause du critère n'était
  éprouvée par RIEN.** « Violation d'index traduite en refus typé, jamais un
  500 » passait par un chemin où la **lecture préalable** refuse avant toute
  écriture : l'`UPDATE` n'était jamais atteint, donc l'exception jamais levée,
  donc la traduction jamais exercée. **Le contrôle mesurait un chemin, la
  clause en décrivait un autre.**

### ⑤ 🔴 Deux défauts CORS rendaient les DEUX routes inatteignables depuis un navigateur

Et **aucun test de Node ne pouvait les voir** — c'est ce qui en fait une
classe, pas deux accidents.

1. **`Access-Control-Allow-Headers` ne permettait pas `Authorization`.** Un
   `fetch` de Node envoie l'en-tête sans rien demander à personne ; un
   navigateur ne l'envoie que si la réponse préalable le permet.
2. **La requête préalable `OPTIONS` n'était pas traitée.** `Authorization` rend
   la requête **non simple** : le navigateur envoie d'abord un `OPTIONS`, et
   **abandonne sans jamais envoyer la vraie requête** si la réponse ne lui
   convient pas.

Trouvés par une corroboration navigateur montée exprès
(`corroboration-navigateur.log`) : service réel sur un port, client servi par
`vite` sur un autre, donc **origine croisée** — c'est ce qui met la politique du
navigateur dans le chemin. ⚠️ **La classe reste OUVERTE** : « ce qu'un
navigateur exige et qu'un test serveur ne voit pas » n'a **aucun garde
automatique** dans ce dépôt, et la seule parade employée est manuelle.

⚠️ **Le montage porte un détail à réemployer** : le coffre garde délibérément
le préfixe de l'utilisateur précédent avant d'éprouver le cas « aucune VM ».
**Sans ce résidu, l'assertion d'effacement ne pourrait pas échouer** — on
constaterait qu'un coffre déjà vide le reste.

### ⑥ La corroboration sur VM réelle — PARTIELLE, et la collision de D6 est ARRIVÉE

**Deux exécutions** (`vm-1-*`, `vm-2-*`, synthèse dans
`vm-corroboration-releve.log`) : **dix assertions tenues aux deux, quatre non
tenues aux deux**, toutes de la même cause, relevée verbatim :

```
WARN agent::plateforme: message de la plateforme illisible (version divergente ?)
erreur=version de plateforme non supportée : 2
texte="{\"type\":\"refus\",\"v\":2,\"motif\":\"version\"}"
```

L'agent présent sur la VM parle `PLATEFORME_VERSION = 1` ; le service, bâti
depuis l'arbre partagé, parle la **2**. **C'est EXACTEMENT la collision que la
décision D6 du plan avait nommée avant tout dispatch** — « `PLATEFORME_VERSION`
a un seul propriétaire à la fois, et c'est G1 » —, et elle est survenue
**pendant cette tâche même**, le sous-bloc G1 ayant fusionné sa montée de
version dans le même arbre entre la clôture de P4 et sa corroboration.

🔵 **ET LA COLLISION EST BRUYANTE, PAS MUETTE — c'est le fait le plus utile de
cette tâche.** Le service refuse, l'agent journalise chacun de ses essais,
`vu_a` reste `null`, et la plateforme annonce donc correctement `injoignable` :
**elle n'a rien fait de faux**, elle a fait de cette VM exactement ce que le
critère ④ lui demande de faire d'une VM dont l'agent ne bat pas. Une plateforme
qui aurait accepté un message de version inconnue aurait produit un agent à
moitié enrôlé, et c'est cette panne-là que P3 a payé pour rendre impossible.

**L'agent n'a PAS été rebâti, et c'est une décision** : P4 ne touche ni
`agent/` ni `proto/` ; le rebâtissage appartient à G1, actif dans le même
arbre ; et à la première tentative `proto/` y était **modifié et non commité**,
si bien qu'une compilation depuis l'hôte aurait poussé du travail à demi fait
sur la VM.

**Ce qui est établi malgré tout, contre le VRAI service HTTP** (par `curl`,
sans une ligne de code à nous entre la surface et le relevé), **avec un VRAI
jeton obtenu par `POST /auth/connexion`, sur une base neuve** : `POST /session`
sans attribution rend **409** sans délivrer de préfixe ; attribuée et agent
muet, **503** avec l'aveu **et** le préfixe ; le préfixe rendu est celui de la
VM enrôlée ; la page-shell compose `<préfixe>:bureau` **à partir du préfixe
rendu par la route** et sa poignée de main est acceptée ; `GET /vm` porte
`sessions_ouvertes`. **Les trois commandes d'administration ont tourné à la
suite sur une base neuve** — c'est la première fois que le chemin d'attribution
complet tourne hors des tests.

✅ **Un legs de P3 exercé PAR ACCIDENT, et seulement à moitié.** E9 — « la
reprise du canal `/agent` n'a jamais été exercée » — était encore due. Les deux
journaux portent **sept** reprises, avec leur échelle doublante relevée :
`delai_ms=500, 1000, 2000, 4000, 8000, 16000, 30000`. ⚠️ **Ce que cela
n'établit PAS, et c'est l'essentiel** : l'échec est ici **permanent** (une
divergence de version ne se répare pas d'elle-même). Ce qui est exercé est
**l'échelle de réessai et son plafonnement à 30 s**, jamais une reprise
**RÉUSSIE**. Le legs reste dû dans sa moitié utile.

### ⑦ Ce que le code livre

| Étage | Fichier | Nature |
| --- | --- | --- |
| le vocabulaire du refus | `plateforme/src/orchestration/refus.ts` (**98**) | **PUR** — `MOTIFS` `as const`, `Motif` dérivé, `CODE_HTTP: Record<Motif, number>` |
| l'interface | `plateforme/src/orchestration/interface.ts` (**127**) | **PUR** — `EtatVm` réexporté de `fraicheur.ts`, DEUX variantes et non quatre |
| la sélection | `plateforme/src/orchestration/selection.ts` (**48**) | **PUR** — `vmsDe`, `laVmDe` ; `laVmDe` **LÈVE** sur deux VMs pour un même utilisateur |
| l'orchestrateur | `plateforme/src/orchestration/inventaire-statique.ts` (**189**) | lit, attribue sous transaction, refuse par un type |
| le dépôt | `plateforme/src/depot/vm.ts` (**143**) | ⚠️ son `SELECT` énumère ses colonnes **pour EXCLURE `vue_a`**, avec son commentaire |
| le jeton porteur | `plateforme/src/http/porteur.ts` (**90**) | **PUR** — exige `type === 'utilisateur'`, **deux rouges, une par sens de confusion** |
| les routes | `http/routes-vm.ts` (**212**), `http/routes-session.ts` (**157**) | liste blanche **dérivée de l'union** ; `attribuer` n'y figure pas, et un test le dit |
| l'administration | `plateforme/src/admin/attribuer-vm.ts` (**224**) | passe par l'orchestrateur, **jamais par un `UPDATE` à part** |
| le coffre | `client/src/prefixe.ts` (**123**) | `poserPrefixe` **LÈVE** sur la chaîne vide ; `effacerPrefixe` |
| le câblage | `client/src/connexion.ts` (**171**) | non testé, **déclaré**, et la clause qui le rend tenable est **resserrée** (voir ⑧) |

**Tailles relevées PAR LA COMMANDE, APRÈS la dernière édition de la ronde,
revue transverse comprise.** 🔴 **AUCUNE EXTRACTION N'A ÉTÉ REQUISE PAR P4, et
ce n'est pas une omission : c'est un relevé**, annoncé par le plan avant
dispatch et confirmé à la clôture. **Aucun fichier de `plateforme/` ni de
`client/src/` n'atteint 450 lignes**, et le tableau de dette reste à **deux**
entrées inchangées — `agent/src/encode.rs` **1536**,
`agent/src/windows_source.rs` **630**.

### ⑧ La revue transverse de fin de branche — huit places, et deux constats neufs

Les places ont été **énumérées par `grep -n` avant d'écrire** et **relues place
par place après**.

| # | Place | Sort |
| --- | --- | --- |
| 1 | `plateforme/src/base/migrations/0001-socle.sql:53-56` | 🔴 attribution **FAUSSE** (voir ③). Annotée, avec le chemin du journal qui la réfute |
| 2 | spec §3.2 | la même, mot pour mot. Annotée |
| 3 | spec §4 « P4 », colonne ROUGE du critère ② | la même. **Barrée** |
| 4 | `plateforme/src/agents/fraicheur.ts` — **`:21-30` depuis l'annotation, `:11-20` avant elle** | « IL N'A AUCUN APPELANT DE PRODUCTION » — P4 lui en donne un. Annotée aux lignes **11-20**, qui poussent la phrase réfutée de dix lignes vers le bas : **c'est le déplacement de citation que le plan prévenait, relevé en relisant après l'édition** |
| 5 | `plateforme/src/signaling/appariement.ts:28` | « P4 reste à venir » — faux dès la fusion. **REMPLACÉE, pas annotée sous elle** : la ligne 28 porte désormais la correction, qui cite la phrase disparue pour que le registre en garde trace. **Et la propriété du fichier tient une TROISIÈME fois** : il n'a toujours pas gagné une ligne |
| 6 | `plateforme/src/signaling/propriete.ts:36-39` | « c'est ce dont P4 **aura** besoin » — il la lit. Annotée **avec la réserve du nom** |
| 7 | `plateforme/src/depot/session.ts:56` | la même formule au futur. Annotée |
| 8 | `client/src/prefixe.ts:11-17` | déjà corrigée par la tâche 13 ; **vérifiée** à la revue plutôt que supposée |

**Trois affirmations du plan VÉRIFIÉES plutôt que supposées** : `TYPES_RELAYES`
inchangé (**vrai** — deux places, toutes deux dans `signaling/relais.ts`) ;
`pilote.test.ts:49` vaut toujours `expect(deps).toEqual(['pg', 'ws'])`
(**vrai** — **P4 n'ajoute aucune dépendance de production**) ; et
`PLATEFORME_VERSION` vaut 1 des deux côtés — ❌ **FAUX au moment du relevé, et
pas du fait de P4** : elle vaut **2**, montée par G1.

**Deux constats NEUFS, documentés et NON corrigés :**

- 🔴 **(a) Le littéral `aucune-vm` du client est une COPIE qu'aucun type ne
  confronte à sa source.** `MOTIFS` est un tableau `as const` dont le type
  dérive, précisément pour qu'ajouter un motif sans son code HTTP soit une
  erreur de compilation — **et cette propriété s'arrête à la frontière du
  paquet** : `client/` ne peut pas importer de `plateforme/`, et le seul paquet
  partagé est `proto/`, que P4 s'interdit de toucher. **Renommer `aucune-vm`
  côté service laisserait le test du client toujours faux, donc le préfixe
  périmé au coffre — une panne MUETTE que ni `typecheck` ni aucun test de ce
  dépôt ne verrait.**
- ⚠️ **(b) La tension du plan sur `connexion.ts` est ARBITRÉE, pas
  contournée.** Sa tâche 14 interdit toute condition dans ce fichier **puis en
  prescrit les branches** ; l'implémenteur l'a signalée sans la trancher.
  L'arbitrage est écrit **dans le fichier** plutôt que dans un rapport : *ce que
  la clause interdit est qu'une RÈGLE vive dans un fichier non testé, pas qu'un
  `if` y apparaisse*, et le critère qui départage est reproductible — **une
  condition est une règle si la changer change ce que le produit décide ; elle
  est du câblage si elle ne fait que router une décision déjà prise ailleurs,
  et testée là-bas.** La clause est **resserrée, pas assouplie**.

### ⑨ ⛔ Une divergence de SÉCURITÉ avec G1, déclarée et NON TRANCHÉE

Sur une VM qui appartient à **quelqu'un d'autre** :

- **P4** rend **404 `vm-inconnue`**, *indistinguable* du cas où la VM n'existe
  pas. Distinguer les deux ferait un **oracle d'énumération** — un utilisateur
  apprendrait quelles VMs existent en lisant le code de retour. C'est la règle
  du critère ② de P3 et celle de `routes-auth.ts`, appliquées pour la troisième
  fois ;
- **G1** retient, pour ses propres routes, **403 `vm-etrangere`** —
  c'est-à-dire **un oracle**, distinct du 404 d'une VM inconnue.

**Les deux chantiers ne peuvent pas avoir raison en même temps.** P4 ne
l'aligne pas : **unifier est une décision de sécurité qui appartient au
PROPRIÉTAIRE DU DÉPÔT**, pas à la seconde branche arrivée. L'écart est inscrit
dans `plateforme/src/http/routes-vm.ts`, dans le document de résultats, et ici.

### ⑩ Ce que P4 n'établit PAS

- **Aucun taux, nulle part.** Deux exécutions par critère, deux pour la
  corroboration VM, **une** pour la corroboration navigateur, **une** pour la
  mesure de la borne du critère ③.
- **Aucun backend d'hyperviseur** : `demarrer`, `arreter` et `instantane` ne
  sont **jamais exécutés**, seule leur voie de refus l'est. Le hub **indique**
  et **avoue** ; il ne redémarre rien.
- **La corroboration sur VM est PARTIELLE** (⑥) : l'état `prete` n'a **jamais**
  été obtenu d'un battement RÉEL. Il l'est en test, sur les deux moteurs, avec
  sa transition assiégée — jamais sur la VM.
- **La concurrence n'est mesurée qu'à DEUX transactions**, une exécution, sur
  **Postgres seul** ; et le test de la suite la reproduit **séquentiellement** :
  il éprouve la traduction du refus, **pas** la sérialisation par le moteur.
  Rien n'est établi à N concurrents, ni sous une autre isolation.
- **`compterOuvertesDe` compte des LIGNES ouvertes, pas des sessions média
  vivantes** — le nom du champ porte la réserve.
- **`vm.vue_a` reste ORPHELINE**, vérifié à la clôture : aucun code de
  production ne l'écrit ni ne la lit. P4 lit `agent_enrole.vu_a`.
- **`depot/session.ts::lireParNom` reste sans appelant de production.**
  ⚠️ Ne pas le confondre avec `depot/vm.ts::lireParNom`, homonyme, qui en a un.
- **La reprise RÉUSSIE du canal `/agent` n'est toujours pas exercée.**
- **Aucune revérification d'une session en cours** : legs n°9 de P2, reconduit.
- **Aucun durcissement de production** : ni TLS, ni cookies, ni en-têtes de
  sécurité, ni frein, ni `coturn` restreint, ni `/sante`. **Les deux routes
  neuves sont ouvertes à la force brute exactement comme `/auth/connexion`
  l'est.** C'est P5.
- **AUCUNE CONSTANTE N'EST CALIBRÉE**, et aucune ne l'a été depuis `BPP_MIN` :
  `SEUIL_INJOIGNABLE_MS` et `PERIODE_BATTEMENT` — qui se recalibrent
  **ENSEMBLE** et vivent dans **DEUX DÉPÔTS DISTINCTS** —,
  `DUREE_JETON_ACCES_MS`, `OCTETS_PREFIXE`, `DUREE_SECONDES`, et la borne de
  250 ms du critère ③. ⚠️ **Celle-là est MESURÉE avant d'être fixée, ce qui
  n'est pas la même chose qu'être calibrée.**
- ~~**La table `application` reste vide** : son écrivain est le sous-projet ④.~~
  ✅ **CLOS par G1 (20 août 2026).** Voir la section G1.

### ⑪ Le témoin de clôture — DEUX témoins, et pourquoi le second sort en 1

⚠️ **Il faut dire ce que chacun mesure, sans quoi le second se lit comme un
échec de P4.**

- ✅ **`temoin-verify-all-cloture.log`, joué au commit `b4b9adb` — sortie 0.**
  **C'est le témoin de P4, et c'est lui qui fait foi.**
- 🔴 **`temoin-verify-all-cloture-finale.log`, relancé après la dernière édition
  de la ronde — sortie 1.** L'échec **n'est pas celui de P4**, et c'est établi
  par **trois relevés**, jamais par une conviction : ① l'étape qui échoue est la
  dernière, `plateforme : npm run typecheck`, sur
  `agents/canal.ts(147): Property 'vm' does not exist on type 'EnrolerMessage |
  CatalogueMessage | LanceeMessage'` ; ② `git log -S 'CatalogueMessage' -- proto/ts/plateforme.ts`
  rend **`3bb7487 apps(g1)`**, un commit du sous-bloc **G1** qui a élargi
  l'union sans que `canal.ts` ne suive ; ③ `git diff --stat b4b9adb..HEAD --
  plateforme/src/agents/canal.ts` rend **VIDE** — le fichier qui ne compile pas
  n'a pas bougé, et tout ce que P4 a touché depuis est du **commentaire** (sept
  fichiers, 165 insertions, 4 suppressions, **aucune ligne exécutable**).

⚠️ **Lequel des deux comptes est rapporté** — les deux sont vrais de choses
différentes, et P3 a payé une correction pour ne pas l'avoir dit : **dix**
appels de la fonction `etape` dans le script (`grep -cE '^etape '`), et
**dix-sept** en-têtes `==>` à l'écran de cette exécution, les sept de plus
venant de l'intérieur de l'étape `client : npm run design:verifier`. **Les deux
sont relevés ce jour.**

Comptes de tests de ce témoin : `plateforme` **284 / 37 fichiers** sur les deux
moteurs, `client` **223 / 24**, `proto` **130 / 5**. ⚠️ **La montée de `proto`
(111 → 130) est ENTIÈREMENT celle de G1** ; et **`cargo test --workspace` est
rapporté, jamais revendiqué** — deux chantiers voisins travaillent dans
`agent/`.

### ⑫ Pièges neufs — à connaître avant de toucher à ce terrain

- 🔵 **Une mutation par substitution de chaîne frappe le COMMENTAIRE avant le
  CODE**, dans un dépôt qui commente ses invariants. Voir ④(b) : la rouge est
  restée verte, et seul un garde « une rouge doit produire une sortie non
  vide » l'a attrapée. **Muter par numéro de ligne, ou par un motif ancré sur
  la syntaxe.**
- ⚠️ **Une clause d'un critère peut décrire un chemin que le contrôle
  n'emprunte pas** (④c) : ici, une lecture préalable refusait avant toute
  écriture, si bien que l'exception qu'on croyait éprouver n'était jamais
  levée. **Compter les gestes réellement faits**, pas seulement lire l'issue.
- ⚠️ **Un `toThrow()` NU est vert sur une fonction qui n'existe pas** — il
  attrape le `ReferenceError`. **Nommer ce qu'on attend.**
- 🔴 **Ce qu'un navigateur exige, aucun test de Node ne le voit** (⑤) : deux
  défauts CORS rendaient les deux routes neuves inatteignables, avec une suite
  entièrement verte. **Une route qui exige `Authorization` doit être éprouvée
  en ORIGINE CROISÉE, dans un vrai navigateur.**
- ⚠️ **Une assertion d'effacement a besoin d'un RÉSIDU pour pouvoir échouer.**
  Constater qu'un coffre déjà vide reste vide n'éprouve rien.
- 🔴 **Deux chantiers ne peuvent pas monter la même version de protocole** — et
  quand cela arrive, **c'est le binaire déjà déployé qui devient muet** (⑥). Le
  symptôme est propre et bruyant grâce à la vérification de version ; **sans
  elle, ce serait un agent à moitié enrôlé**.
- ⚠️ **Ne jamais lancer `scripts/build-agent.sh` quand `proto/` porte des
  modifications non commitées d'un chantier voisin** : il rsynchronise les
  sources, et pousserait sur la VM du travail à demi fait.
- ⚠️ **Un port « libre par convention » ne l'est pas.** 8082 était tenu par
  `otbr-agent`, et 8081 par `crowdsec` : une exécution a été perdue. **Relever
  `ss -ltn` avant de choisir**, plutôt que de reprendre le port du sous-bloc
  précédent.

### ⑬ Ce que P4 lègue à P5

**Legs de P3 réglés** : n°1 (la route du préfixe — ⚠️ **le coût de la spec §10
est RÉDUIT par deux gardes, pas soldé**), n°2 (`fraicheur.ts` a son appelant),
n°3 **à moitié** (`session.utilisateur_id` a son lecteur ; `application` attend
toujours son écrivain). **Legs de P2 réglé** : n°4.
⚠️ **Le n°4 de P3 n'est NI réglé NI réduit** : P4 branche la **source** du
préfixe, pas sa **portée**.

**Ce qui reste dû :**

1. 🔴 **P5 ou ④ — le préfixe reste PAR VM, et `signaling/propriete.ts` reste EN
   MÉMOIRE.** Après un redémarrage du service, deux clients humains de la même
   VM retrouvent le même préfixe et `<préfixe>:w-1` redevient revendicable.
   **Legs n°4 de P3, reconduit sans réduction.**
2. 🔴 **Tous — faire descendre `MOTIFS` dans `proto/ts`.** Sans quoi le littéral
   `aucune-vm` du client reste une copie qu'aucun type ne confronte à sa source,
   et son renommage côté service serait une **panne muette** (⑧a).
3. ⛔ **Le PROPRIÉTAIRE DU DÉPÔT — trancher la divergence de refus avec G1**
   (⑨) : `404 vm-inconnue` contre `403 vm-etrangere`, c'est-à-dire l'absence
   d'oracle d'énumération contre un oracle. **Ce n'est pas une décision de
   chantier.**
4. ⛔ **Tous — rejouer la corroboration VM sur un arbre où `PLATEFORME_VERSION`
   est stable.** L'instrument est versé et prêt
   (`journaux-plateforme-p4/instrument/vm-corroboration.sh`), les quatre
   assertions qui tombent sont écrites, et **le script n'a pas une ligne à
   changer**.
5. ⛔ **Tous — la reprise RÉUSSIE du canal `/agent`** (E9). Son échelle de
   réessai est désormais vue (⑥) ; une reprise qui **aboutit**, non.
6. ⚠️ **Tous — la classe « ce qu'un navigateur exige et qu'un test serveur ne
   voit pas » n'a AUCUN garde automatique** (⑤). Deux défauts y sont passés ; la
   seule parade employée est une corroboration **manuelle**.
7. ⛔ **P5 — le frein sur `/auth/connexion`, `/agent`, ET les DEUX routes
   neuves.** `GET /vm` et `POST /session` sont ouvertes à la force brute
   exactement comme `/auth/connexion` l'est.
8. ⛔ **P5 — le secret d'enrôlement en clair dans `C:\dev\run-agent.ps1`** :
   intouché, P4 n'écrit pas dans `scripts/`.
9. ⛔ **P5 — TLS, cookies, en-têtes de sécurité, `/sante`, `coturn` restreint.**
10. ⛔ **Tous — `vm.vue_a` reste ORPHELINE** et `depot/session.ts::lireParNom`
    **sans appelant de production**. Le `SELECT` de `depot/vm.ts` exclut
    explicitement `vue_a`, avec son commentaire : c'est la seule garde bon
    marché contre un successeur qui la croirait renseignée.
11. ⛔ **Tous — la garde ne couvre que la POIGNÉE DE MAIN.** Legs n°9 de P2,
    reconduit par P3, reconduit ici.
12. ⛔ **Tous — aucune constante calibrée** (⑩), et P4 en ajoute une, **mesurée
    avant d'être fixée mais pas calibrée**.
13. ⚠️ **Tous — un jeton reste valide jusqu'à son expiration même si le compte
    disparaissait.** Aucun chemin de suppression d'utilisateur n'existe, donc le
    cas n'est pas atteignable — **déclaré, non corrigé**.

---
## 🎨 Sous-projet ⑥ Design system — sous-bloc S1 : le socle, et les sept contrôles (19 août 2026)

Résultats complets :
`docs/superpowers/plans/2026-08-19-design-system-s1-resultats.md`.
Plan : `docs/superpowers/plans/2026-08-19-design-system-s1.md` (commit `f4cb8c0`).
Spec : `docs/superpowers/specs/2026-08-19-design-system-design.md` (commit `5b6b830`).
Journaux : `docs/superpowers/plans/journaux-design-s1/`.

⚠️ **UNE SEULE FAMILLE DE LECTURE — la plus simple de tous les sous-blocs de ce
dépôt, et c'est MESURÉ, pas supposé** :
`grep -lP '\x1b\[' docs/superpowers/plans/journaux-design-s1/*` rend **la liste
vide** — **aucune séquence ANSI, dans aucun fichier**. `file` rend « UTF-8 » ou
« ASCII » partout, et **aucun fichier ne porte de `\r`**. Ce sont des sorties
`npm`/`node` sur l'**hôte**, jamais du PowerShell distant : ni `sed`, ni
`grep -a`, ni `iconv`. **Ils se `grep`ent à plat.**

⛔ **AUCUNE TÂCHE DE S1 N'A EMPLOYÉ LA VM WINDOWS**, et ce n'est pas une gêne :
⑥ est un sous-projet **navigateur**, et la spec §9 déclare qu'il n'a **aucune
recette sur VM**. La VM était par ailleurs occupée par la recette du chantier
microphone. La seule mesure hors terminal est la corroboration **hors critère**
à deux fenêtres, faite dans un Chromium **de l'hôte**.

### Les sept contrôles — verdict, et nombre d'exécutions

**Deux exécutions de chacun, au commit `604f91c`. Aucun taux n'est revendiqué** :
les sept contrôles sont **déterministes**, et deux exécutions y établissent la
**reproductibilité**, jamais une fréquence.

| # | Contrôle | Verdict | Le chiffre, **relevé** |
| --- | --- | --- | --- |
| §7.1 | contrastes WCAG | **VERT** | **50 paires, 0 échec, minimum 3,16** |
| §7.2 | aucune couleur littérale hors `tokens.css` | **VERT** | **0** sur 46 fichiers — contre **onze** sur l'arbre intact |
| §7.3 | toute surface bâtie porte les tokens | **VERT, DEUX assertions** | A : 0/4 pages ; B : 0, **évaluée sur 4 pages** contre **1** avant |
| §7.4 | les trois blocs déclarent le même ensemble | **VERT** | **0 écart** |
| §7.5 | la bascule atteint les N fenêtres | **VERT** | **10 tests** |
| §7.6 | orphelins et `var()` non déclarés | **VERT, sous liste d'attente nommée** | ① **0 écart** ; ② **28 orphelins, 28 en attente déclarée** |
| §7.7 | le poids CSS ne dérive pas | **VERT** | **3 503** octets / plafond **12 288** |

`npm run design:verifier` → **6/6**. Le septième, §7.5, est un **test
unitaire** et tourne dans `npm test` — le dire évite qu'un lecteur compte six et
conclue qu'il en manque un.

✅ **`scripts/verify-all.sh` sort 0 sur ses DIX étapes**, et **aucune étape
étrangère n'a échoué** — ni `cargo test --workspace`, ni `clippy`, ni `proto`,
ni les trois `plateforme` (Postgres compris). C'est à signaler parce que le plan
prévoyait le contraire : P2 avait vu son témoin tomber « sur un test Rust du
voisin », et le chantier E avait du travail non commité dans `agent/`. **Le
risque était réel et ne s'est pas réalisé.**

Comptes : `client` **179** tests, `proto` **37**, `typecheck` **exit 0**.

### 🔵 Le fait le plus réutilisable : Node v24.9.0 importe un `.ts` depuis un `.mjs`

**Mesuré, sans `tsx`, sans `ts-node`, sans `@types/node`, sans AUCUNE dépendance
neuve.** C'est ce qui permet à **trois** contrôles (§7.1, §7.4, §7.6) de partager
un seul parseur — `client/src/design/tokens.ts`, typechecké et testé — au lieu
d'en recopier la logique. Le point de conception que cela sert est celui du
§7.1 : « **un contrôle qui a sa propre copie des valeurs valide sa copie** ».

⚠️ **Le coût, mesuré lui aussi : le retrait de types NE TYPECHECKE PAS, et il
refuse le TypeScript NON EFFAÇABLE.** Un `export enum T { A, B }` importé de
cette façon fait planter Node :

```
$ node runenum.mjs
.../enum.ts:1
export enum T { A, B }
```

**Tout module de `client/src/design/` importé par un outil doit donc rester
« effaçable »** : pas d'`enum`, pas de `namespace`, pas de propriétés de
constructeur, pas de décorateurs. Un commentaire de tête de chaque module
concerné la porte.

🔵 **Et S1 l'a exploité une SECONDE fois, au-delà de ce que le plan prévoyait** :
`client/outils/tokens-orphelins.mjs` importe **`client/vite.config.ts`** pour
lire la liste des entrées Vite, plutôt que de la recopier. C'est ce qui empêche
son périmètre d'attraper `probe-coalesced.html` et `client/recette/*.html`, qui
sont des **instruments de banc hors produit**.

### 🔴 La décision des 28 tokens orphelins — refusée SUR MESURE, pas sur un goût

À la fin de S1, `tokens.css` déclare **47** tokens et le produit en appelle
**19** : **28 sont orphelins**, par construction — S1 pose la palette entière,
S2 à S4 l'emploieront.

**Élaguer la palette à ce qui sert** était la voie évidente. Elle est refusée
sur un relevé :

```
paires totales : 50
paires citant au moins un token sans appelant : 46
```

**Sur les 50 paires de contraste que §7.1 vérifie, 46 citent au moins un des 28
tokens sans appelant.** Élaguer ferait tomber §7.1 **de 50 paires à 4** : on
satisferait un contrôle en **vidant** l'autre — le geste que ce dépôt combat.

> ⚠️ **RELEVÉ DE S1, ET IL EST FAUX AU PRÉSENT DEPUIS S2 — il est laissé DATÉ
> plutôt qu'effacé, parce qu'un relevé daté reste vrai comme histoire.** La même
> commande, relancée le **20 août 2026** sur les dix entrées restantes, rend
> `paires totales : 52 | citant un token en attente : 0`. **ZÉRO — et ce zéro dit
> l'inverse de ce qu'on croirait y lire** : il montre que cette décision de S1 a
> TENU jusqu'au bout, aucune des quatorze couleurs par thème n'étant plus
> orpheline. ⚠️ **Corollaire qui mord : cet argument NE PROTÈGE PLUS RIEN** — un
> élagage n'atteindrait aujourd'hui plus aucune couleur, et ce qui protège les
> dix entrées restantes n'est plus que leur annotation, c'est-à-dire exactement
> ce que rien ne contrôle. Voir la section S2, §②.

**La forme retenue est une LISTE D'ATTENTE EXACTE, jamais un seuil.** Le
contrôle exige l'**ÉGALITÉ** entre l'ensemble des orphelins et la liste, donc il
échoue **dans les deux sens** : un orphelin **absent** de la liste
(`NOUVEL ORPHELIN`), et un token de la liste qui **a gagné** un appelant
(`À RETIRER DE LA LISTE`). **La seconde moitié est celle qui compte : elle rend
la liste AUTO-NETTOYANTE.** Un seuil (« au plus 28 ») aurait pourri sur place ;
une liste dont chaque retrait est **forcé** rétrécit toute seule, et **le jour
où elle est vide, elle disparaît**. Les deux sens ont été vus rouges.

⚠️ **Ce que ce contrôle mesure, dit sans le maquiller** : **pas** « la palette
est-elle entièrement employée ? » — la réponse est non jusqu'à S4 —, mais que
**l'écart entre la palette et son emploi soit CONNU, ÉNUMÉRÉ ET DÉCROISSANT**.
C'est moins que ce que le §7.6 laissait espérer, c'est écrit dans le script à
l'endroit où on le lit, et **cela peut échouer dès aujourd'hui**.

### Les huit relevés PÉRIMÉS de la spec — valeur juste

Corrigés par le plan à `8ad03a2`, et **repris après S1** là où S1 les a fait
bouger à nouveau.

| # | Ce que la spec écrit | Juste (`8ad03a2`) | Après S1 (`604f91c`) |
| --- | --- | --- | --- |
| 1 | « tout le style tient en **85 lignes** » | **139** | **181** + **207** + **73** + **8** (quatre feuilles) |
| 2 | « **cinq** valeurs littérales comme couleurs » | **neuf** | **zéro** |
| 3 | « `vite.config.ts:10-12` déclare **deux** entrées » | **trois**, en `:15-19` | **quatre** — `design.html` |
| 4 | « **aucun écran de connexion**, P2 est à venir » | **il est arrivé** | et **il a sa feuille** |
| 5 | « aucun jeton dans la poignée de main » | `shell-page.ts:62` en envoie un | inchangé par S1 |
| 6 | « **25 737** octets, dont **1 055** de CSS » | **31 701** / **1 429** | CSS **3 503**, en **deux** actifs |
| 7 | « la fenêtre de session a **quatre** éléments » | **cinq** | inchangé par S1 |
| 8 | « `verify-webrtc.mjs` à **497** (marge 3) » | **494** (marge **6**) | **494** — **intouché par S1** |

### Les onze divergences D1…D11, en une phrase chacune

- **D1** — la spec annonce **trois** contrôles rouges, il n'y en a que **deux** :
  §7.2 et §7.3 joués rouges sur l'arbre intact ; les quatre autres **n'avaient
  rien à lire**, ce qui est un plantage, pas une mesure.
- **D2** — **trois longueurs n'ont aucun cran** dans les échelles
  (`padding: 6px`, `font-size: 18px`, `letter-spacing: 0.02em`) : elles restent
  littérales, déclarées, et **la clause « aucune longueur hors échelle » du §8
  est FAUSSE à la fin de S1**, de trois valeurs exactement.
- **D3** — **deux des neuf couleurs ne sont pas des noirs** : six voiles hors
  thème, valeurs reprises **verbatim**, aucun raccord à `--danger`/`--alerte`.
- **D4** — **`--e-3` ne vaut 12 px que si `html` perd son `font-size`** ; et
  « à l'identique » vaut **à racine 16 px, et là seulement**.
- **D5** — la clé est **`guac.theme`**, préfixée comme les `guac.jeton.*` de P2,
  et le test de robustesse emploie **la vraie clé voisine**, pas une inventée.
- **D6** — un `.mjs` **importe** un `.ts` nativement (voir ci-dessus).
- **D7** — **`client/vite.config.ts` n'est PAS typechecké** : une erreur y est
  un **échec de build**, jamais une erreur `tsc`.
- **D8** — le mot-clé **`red` lève un faux positif sur du français**
  (« re**d**éclenche ») : §7.2 retire les commentaires d'abord.
- **D9** — **`color-scheme` suit le thème**, sans être un token `--*` : c'est
  lui qui décide de l'apparence **native** des `<input>` de l'écran de connexion.
- **D10** — l'arbre est **partagé**, et l'événement s'est produit (voir plus bas).
- **D11** — la galerie est la **4ᵉ entrée Vite**, **incluse** dans §7.2 et §7.3,
  **exclue** de la seule moitié « employé » de §7.6.

### Variables d'environnement : **AUCUNE**

**S1 n'en introduit pas une seule**, ni pour l'agent, ni pour le service, ni
pour le client — et c'est dit parce que le tableau du dépôt en compte beaucoup
et qu'**une absence se déclare**. Corollaire : le piège maison « toute variable
neuve doit être ajoutée à `scripts/run-agent.sh` » **ne s'applique pas**, et ce
script **n'est pas modifié**. Même situation qu'en P1 et P2.

### Pièges neufs — à connaître avant de toucher à ce terrain

- 🔴 **`injectTo` par défaut place le script AVANT `<meta charset>`.**
  `HtmlTagDescriptor.injectTo` vaut `'head-prepend'` par défaut : le script
  d'amorce sortirait avant la déclaration d'encodage, alors qu'il porte des
  commentaires accentués. **`injectTo: 'head'`** le place après `<meta charset>`
  et `<title>`, avant le module et la feuille — **et cela suffit à l'anti-FOUC**,
  un script en ligne synchrone du `<head>` s'exécutant avant que `<body>` ne
  soit analysé.
- 🔴 **UN SCRIPT INJECTÉ PAR `transformIndexHtml` PART VERBATIM, COMMENTAIRES
  COMPRIS — mesuré à 1 921 octets DANS CHACUNE des pages.** Il ne traverse aucun
  transform, à l'inverse des commentaires d'un `.ts` que le bundler retire.
  **Aucun des sept contrôles ne l'aurait dit** : §7.7 ne pèse que le CSS. Le
  raisonnement long doit donc vivre là où il est **gratuit** — dans le greffon
  (`vite.config.ts`, temps de build) et dans `theme.ts`.
- 🔴 **UN GARDE PEUT ÊTRE SATISFAIT PAR SON PROPRE COMMENTAIRE.** Le garde qui
  compare `CLE_THEME` au texte de `amorce-theme.js` a été **vu vacueux** : la
  clé étant nommée entre quotes dans le commentaire d'en-tête de l'amorce, une
  amorce réduite à `var t = null;` **passait les deux assertions**. Remède : la
  clé ne s'écrit plus dans ce commentaire, et l'assertion porte sur l'**appel**
  (`getItem('guac.theme')`), pas sur la présence de la chaîne quelque part.
- 🔴 **UNE PERTURBATION QUI NE PERTURBE RIEN SE LIT COMME UN CONTRÔLE QUI NE
  MORD PAS.** La rouge de §7.4 a d'abord rendu `exit=0` : elle ancrait sur
  `@media (prefers-color-scheme: light)` et `:root[data-theme="clair"]`, qui
  apparaissent **d'abord dans le commentaire d'en-tête** de `tokens.css`. Rien
  n'était retiré. **Ancrer sur le sélecteur suivi d'une accolade**, et poser une
  `assert` qui refuse de jouer si l'ancre est introuvable.
- ⚠️ **`*/` DANS UN COMMENTAIRE DE BLOC : la sous-chaîne `src/**` suivie de
  `/*.ts` ferme le commentaire.** Écrire un glob TypeScript dans un commentaire
  de `vite.config.ts` a fait échouer le build sur `ERROR: Unexpected "*"` — et
  **`tsc` n'en aurait rien dit** (D7).
- ⚠️ **Le nom de l'actif CSS partagé n'est PAS prévisible** : Vite le dérive
  d'un morceau JavaScript voisin. Balayer `dist/assets/*.css` sans présumer
  d'aucun nom ; un script cherchant `socle-*.css` ne trouverait rien.
- ⚠️ **`grep -c` compte des LIGNES, pas des occurrences** : le contrôle
  « l'amorce est-elle injectée ? » rendait `2` par page tant que le commentaire
  nommait la clé sur une autre ligne que le code.
- ⚠️ **`pkill -f <motif>` depuis un shell dont la ligne de commande contient le
  motif tue le shell** (exit 144). Piège déjà documenté par ce fichier, **payé
  une fois de plus ici** en arrêtant un `vite preview`. Tuer par PID relevé.
- ⚠️ **Vitest court-circuite le CSS par défaut**, `?raw` compris : un
  `import css from './x.css?raw'` rend la chaîne **vide** et un test qui la
  parserait **passerait au vert en ne mesurant rien**. `test: { css: true }` est
  posé dans `client/vite.config.ts` — **et surtout, il n'y a délibérément PAS de
  `client/vitest.config.ts`**, qui prendrait le pas sur lui **sans rien dire**.

### ⚠️ La revue transverse de fin de branche — CINQ énoncés devenus faux

Elle en a trouvé **cinq** en D7, **trois** en D8, **six** en D9, **douze** en
D10, **sept** en D11, **huit** en P1 et **dix** en P2. **Cinq ici**, et ils ont
la forme habituelle : **corrects des deux côtés pris séparément**.

| # | Énoncé | Sort |
| --- | --- | --- |
| **1** | `client/src/design/tokens.css` — « **sans appelant, le token sort** », écrit par la tâche 7 à propos de `--police-mono`, **RÉFUTÉ par la tâche 12 de la même branche** : le token n'est pas sorti, il est sur une liste d'attente nommée dont **S4** décide | **CORRIGÉ** |
| **2** | `client/src/design/theme.ts` — « la confirmation à deux fenêtres réelles **est prévue** » : la tâche 14 l'a **faite** | **CORRIGÉ**, avec son commit et ses réserves |
| **3** | `client/connexion.html` — « cette page porte la structure **et rien d'autre** » : la tâche 11 lui a lié le socle, et **son rendu change** | **CORRIGÉ** sans retirer le bloc, qui appartient à **S3** |
| **4** | `client/outils/surfaces-baties.mjs` — « B **est** rouge sur `dist/index.html` », un **présent** daté sans son commit, alors que B est verte sur quatre pages depuis la tâche 11 | **CORRIGÉ** en relevé daté (`71f3c36`) |
| **5** | `client/outils/couleurs-litterales.mjs` — « le vert **arrive quand** la tâche 9 les fait migrer » : il est **arrivé** (`ab9e9b9`) | **CORRIGÉ** |

🔵 **Et un énoncé du PLAN, réfuté par la mesure et non par une opinion — c'est
le plus instructif du sous-bloc.** Le plan annonçait que §7.2 rendrait **neuf**
sur l'arbre intact et prescrivait : « **s'il rend onze, le traitement des
commentaires n'a pas été fait** ». **Il rend ONZE, et le diagnostic est faux des
deux côtés** :

- les deux de l'écart sont `style.css:3-4` — `--surface: #0b0d10` et
  `--text: #e6e8eb` —, **des déclarations de token vivant hors de `tokens.css`**.
  L'exclusion du §7.2 est par **FICHIER**, jamais par rôle, et une déclaration
  hors de la source unique est **précisément la dérive que ce contrôle existe
  pour voir** ;
- **un défaut de traitement des commentaires rendrait TREIZE**, pas onze :
  `resize.ts:7` et `resize.test.ts:30` portent le mot `red` en français.

**Les deux moitiés sont vérifiées par la commande, sur l'arbre reconstruit à
`71f3c36`** — `git archive` puis le contrôle avec `--racine` —, pas déduites.

⚠️ **Le plan porte aussi QUATRE EXIGENCES QUI NE TIENNENT PAS ENSEMBLE**, et la
quatrième cède : il demande que les `*.test.ts` soient balayés, **et** que
`contraste.test.ts` porte les vecteurs `#000000`/`#ffffff`/`#808080` fixés par
WCAG, **et** que `reprise.test.ts` compare `--fond-0` à `#0b0d10` exactement,
**et** que §7.2 rende zéro. Les littéraux de ces deux tests sont
**obligatoires** — les interdire rendrait les tâches impossibles. **L'exclusion
est donc posée au plus étroit : `client/src/design/*.test.ts` seulement.** Sans
elle, le contrôle rendrait **46**.

⚠️ **Trois énoncés du plan et de la spec sont laissés FAUX, délibérément, faute
d'être dans le périmètre de S1** — un index durable ne se commite pas avec une
spec, et c'est la même décision que le legs n°11 de D8 :

1. la spec §4.1 déclare qu'« **aucun appelant de `getComputedStyle` n'existe
   aujourd'hui** » : `client/src/design/galerie.ts` en est le **premier**, et il
   respecte la règle que le §4.1 écrivait pour lui (lecture **au changement de
   thème**, jamais par image) ;
2. la spec §8 porte « aucune longueur hors échelle », **faux de trois valeurs**
   (D2) jusqu'à S4 ;
3. le plan, tâche 12, prescrit de retirer `font-family: var(--police-mono)` de
   `style.css` pour jouer une rouge — **cette ligne n'a jamais existé**, S1
   n'ayant pas câblé le token.

### ⚠️ L'arbre est PARTAGÉ, et il a bougé EN PLEINE RECETTE

Le chantier E (microphone) a commité **`604f91c`** entre la première et la
seconde salve de mesures, prises initialement à `c9bb8a7`. **Toutes les mesures
ont été REPRISES à `604f91c`**, jamais recopiées — c'était le geste le moins
cher et le seul honnête. Ce que ce commit touche dans `client/`, relevé par
`git diff --stat` : **un seul fichier, `client/recette/micro-e1.mjs`**, un
instrument de banc **hors du périmètre de chacun des sept contrôles** — vérifié
par la sortie des contrôles eux-mêmes, pas supposé.

**Aucun `git add -A` de tout le sous-bloc** ; chaque commit à pathspec explicite.

### Le relevé de tailles, PAR LA COMMANDE, à `1bf93cf`

**Le tableau de dette est INCHANGÉ, à deux lignes** — `agent/src/encode.rs`
**1536**, `agent/src/windows_source.rs` **630**. **Aucun fichier de `client/` ne
dépasse 500 lignes.**

| Fichier | Lignes | Remarque |
| --- | --- | --- |
| `client/verify-webrtc.mjs` | **494** (marge **6**) | **INTOUCHÉ par S1** — la leçon de P2 (« une addition de commentaire peut annuler une extraction ») est respectée à la lettre |
| `client/src/main.ts` | **451** | ⚠️ **DU VOISIN** : **392** au relevé D10, porté là par le chantier E. Relevé, **attribué au voisin**, pas repris à notre compte |
| `client/outils/tokens-orphelins.mjs` | **233** | neuf |
| `client/design.html` | **231** | neuf — la galerie |
| `client/outils/couleurs-litterales.mjs` | **228** | neuf |
| `client/src/design/tokens.css` | **207** | neuf — **la source unique** |
| `client/src/design/galerie.ts` | **193** | neuf |
| `client/src/style.css` | **181** | 139 → 181 |
| `client/src/design/tokens.ts` | **176** | neuf — **pur**, partagé par trois contrôles |
| `client/src/design/theme.test.ts` | **156** | neuf |
| `client/src/design/contraste.ts` | **150** | neuf — **pur**, WCAG 2.1 et les 50 paires |
| `client/src/design/tokens.test.ts` | **133** | neuf |
| `client/src/design/contraste.test.ts` | **114** | neuf |
| `client/outils/surfaces-baties.mjs` | **112** | neuf |
| `client/src/design/theme.ts` | **107** | neuf — **pur, dépendances injectées** |
| `client/vite.config.ts` | **106** | +le greffon d'amorce, +la 4ᵉ entrée |
| `client/src/design/reprise.test.ts` | **95** | neuf |
| `scripts/verify-all.sh` | **92** | **neuf → dix étapes** ⚠️ **PRÉCISÉ à la revue transverse de P3 (19 août 2026) : « dix » est le compte des appels `etape` du script, et l'EXÉCUTION en affiche DIX-SEPT.** Relevé par la commande sur une exécution complète (`grep -c '^==>'` sur le journal de `./scripts/verify-all.sh`, sortie 0) : **10** en-têtes viennent de `verify-all.sh` lui-même, et **7** de l'intérieur de son étape `client : npm run design:verifier` — un `npm run build` plus les **six** contrôles du socle. Le septième contrôle du socle, §7.5, est un test unitaire et tourne dans `client : npm test`. **Les deux comptes sont vrais de choses différentes ; ni « dix » ni « dix-sept » ne se suffit sans dire lequel on compte** |
| `client/outils/poids-css.mjs` | **89** | neuf |
| `client/src/design/base.css` | **73** | neuf |
| `client/outils/verifier-design.mjs` | **60** | neuf — l'agrégateur |
| `client/connexion.html` | **47** | +le lien du socle, +l'annotation de revue |
| `client/outils/contraste.mjs` | **44** | neuf |
| `client/outils/blocs-de-theme.mjs` | **44** | neuf |
| `client/src/design/amorce-theme.js` | **35** | neuf — **ni typechecké ni testé, et déclaré** |
| `client/index.html` | **26** | +le lien du socle |
| `client/shell.html` | **15** | **sa première feuille de style depuis D1** |
| `client/src/design/socle.css` | **8** | neuf — le point d'entrée unique |

⚠️ **La porte de S1 est à 300 lignes, pas à 500** (spec §10), et **aucun fichier
ne l'approche** : le plus gros est à **233**. Ce dépôt a franchi le plafond
**trois fois en D10** et **deux fois en D9**, et l'a rattrapé **après**, dont
deux fois par une **compression** que ce fichier interdit nommément.

**Poids CSS : 3 503 octets**, ligne de base **1 429** (avant S1), plafond
**12 288**, marge **8 785**. ⚠️ **Le `<style>` en ligne de `client/design.html`
n'est PAS compté** — §7.7 ne pèse que `dist/assets/*.css`. La galerie n'étant
pas du produit ce n'est pas une lacune, mais c'est une **portée**.

### Ce que S1 n'établit PAS

- **Aucun taux.** Deux exécutions par contrôle, sur des contrôles
  **déterministes** : reproductibilité, rien de plus.
- 🔴 **Aucun jugement visuel n'a été porté sur aucune valeur, et les HUIT
  jugements humains du §8 restent entiers** : que la direction soit « sobre » et
  « pro », que `#7aa2f7` soit le bon bleu, que le ratio **1,2** soit le bon, que
  **14 px** soit assez dense, que le pas de **4 px** soit le bon, que `--bord`
  ait été employé là où il fallait, que le plafond de **12 Kio** soit au bon
  endroit, que la galerie montre ce qu'il faut regarder. **Aucun ne deviendra
  une mesure**, et ils rejoignent `BPP_MIN`, `FACTEUR_FOCUS`,
  `PART_DORMANTE_BPS`, `HYSTERESIS`, `TAILLE_MAX_SORTIE` et les paramètres
  `scrypt` de P2.
- **Rien de l'anti-FOUC OBSERVÉ.** L'amorce est prouvée **injectée** (`1` par
  page bâtie), **placée** (après `<meta charset>`) et **exécutée** — par
  isolation sur `connexion.html`, qui n'importe **aucun** module de thème et
  porte pourtant `data-theme`. **Qu'aucun éclair de mauvais thème ne soit
  visible n'est mesuré par rien.**
- **Que le thème atteigne les N fenêtres du PRODUIT** : la corroboration porte
  sur **deux fenêtres de la galerie**, pas sur une page-shell qui ouvre N
  sessions par `window.open`. Et elle est **HORS CRITÈRE**, une exécution.
- **Trois longueurs restent hors échelle**, et **aucun des sept contrôles ne
  mesure une longueur**.
- **La reprise à l'identique vaut à racine 16 px, et là seulement.**
- **Aucune primitive, aucune surface habillée** : c'est S2 et S3. `shell.html`
  et `connexion.html` reçoivent les **tokens**, et **leur rendu change** — la
  neutralité n'est promise que sur `index.html`.
- **Aucune vérification hors d'un Chromium de bureau** : rien de Firefox, de
  Safari, du mobile. **Rien du HiDPI** (`deviceScaleFactor` = 1).
- **L'accessibilité au-delà du contraste** : navigation clavier complète,
  lecteurs d'écran, `prefers-reduced-motion`, cibles tactiles. **Le contraste
  est mesuré ; le reste ne l'est pas.**
- **Aucune internationalisation** : rien ne dit que la mise en page survit à une
  langue plus longue.
- **Le legacy n'est pas touché**, et **aucun contrôle ne le balaie** : deux
  directions visuelles coexistent dans le dépôt.
- ⚠️ **La galerie n'a AUCUN test**, ni elle ni `galerie.ts` : **une galerie qui
  cesserait de rendre une famille entière ne serait attrapée par aucun
  contrôle**, seulement par l'œil — ce qui est précisément son statut
  d'instrument de jugement humain.

### Les legs

1. ⛔ **`--police-mono` n'a qu'un appelant prévu et n'est pas câblé.** **S4
   tranche : ou il le câble sur `#stats`, ou il le retire.** C'est le **seul**
   des 28 tokens en attente dont le sort soit ouvert ; les 27 autres ont un
   sous-bloc nommé. ⚠️ S'il est retiré, il faut aussi le retirer de la galerie,
   qui l'emploie — l'inclusion ① de §7.6 le dirait.
   ✅ **TRANCHÉ PAR S4 (20 août 2026) : IL EST CÂBLÉ**, sur `#stats`, l'appelant
   unique que la spec §4.3 désigne. **La liste d'attente est VIDE** — 52 tokens
   déclarés pour 52 employés — et **elle ne disparaît pas pour autant** : c'est
   l'ÉGALITÉ de §7.6 qui vaut, pas la liste.
2. ⛔ **Les trois longueurs hors échelle de D2** — `padding: 6px`,
   `font-size: 18px`, `letter-spacing: 0.02em` — sont à reprendre par **S4**,
   seul sous-bloc autorisé à toucher ces éléments. **La clause « aucune longueur
   hors échelle » du §8 reste fausse jusque-là.**
   ✅ **REPRISES PAR S4, ET LA CLAUSE EST DEVENUE UNE COMMANDE** : elles étaient
   **SIX** à la fin de S3 (les trois de `shell.css` et `connexion.css` s'étant
   ajoutées), et **§7.10** — le premier contrôle de ⑥ qui mesure une longueur —
   les fait toutes tomber : **0 occurrence, 0 valeur**.
3. ⛔ **Le plafond de 12 Kio n'est calibré par rien** : « un garde-fou contre une
   addition massive, pas une cible de budget » (spec §7.7).
4. ⛔ **`prefers-reduced-motion` est nommé et non pris** — le moins cher des
   quatre manques d'accessibilité, et le premier à prendre.
5. ⛔ **Les trois énoncés faux laissés dans la spec et le plan** (voir la revue
   transverse) : l'appelant de `getComputedStyle`, la clause du §8, et la rouge
   d'une ligne qui n'a jamais existé.
6. ⛔ **La liste d'attente des 28 tokens doit RÉTRÉCIR à chaque sous-bloc.** Le
   contrôle le force — il refuse un token de la liste qui a gagné un appelant —
   mais **rien ne force S2 à en consommer** : c'est une règle de revue.
   ✅ **TENU PAR S2 (20 août 2026) : 28 → 10 en QUATRE commits**, chaque retrait
   **lu dans la sortie du contrôle** et jamais deviné. La règle de revue a été
   suivie, elle n'a pas été outillée : **rien ne force S3 à en consommer non
   plus**, et le re-étiquetage de trois entrées par S2 montre par quelle porte on
   assouplirait cette liste sans qu'aucune commande ne le dise. Voir la section
   S2, §②.

---

## 🎨 Sous-projet ⑥ Design system — sous-bloc S2 : les primitives (20 août 2026)

Recette : `docs/superpowers/plans/2026-08-19-design-system-s2-resultats.md`.
Plan : `docs/superpowers/plans/2026-08-19-design-system-s2.md` (`0e27ee6`).
Spécification : `docs/superpowers/specs/2026-08-19-design-system-design.md`
(`5b6b830`), **non modifiée par S2**.
Journaux : `docs/superpowers/plans/journaux-design-s2/` — **UNE SEULE FAMILLE DE
LECTURE**, contrairement aux trois de D9 et aux quatre de D8, et c'est **mesuré,
pas supposé** :

| Famille | État | Ce qu'il faut faire |
| --- | --- | --- |
| **tous** les fichiers | **aucune séquence ANSI** (`grep -lP '\x1b\['` → aucun, `exit=1`), « ASCII text » ou « Unicode text, UTF-8 text » (`file`), **aucun `\r`** (`grep -lc $'\r'` → aucun) | **rien** — ils se `grep`ent à plat |

**La raison de cette simplicité est structurelle** : ce sont des sorties
`npm`/`node` sur l'**hôte**, jamais du PowerShell distant. Le défaut à deux
réglages ne peut pas les atteindre.

⛔ **AUCUNE TÂCHE DE S2 N'A EMPLOYÉ LA VM WINDOWS**, et ce n'est pas une
omission : ⑥ est un sous-projet **navigateur**, et la spec §9 déclare qu'il n'a
**aucune recette sur VM**.

**Variables d'environnement introduites par S2 : AUCUNE**, et aucune n'est lue.
*L'absence est déclarée parce qu'une absence se déclare.*

S2 pose les quatre familles de primitives dont un écran de connexion a besoin —
**bouton, champ, surface, message** —, **entièrement en tokens**, et fait tomber
la liste d'attente des tokens orphelins **de 28 à 10 dans les commits mêmes qui
mettent chaque token en service**.

### ① Les quatre critères, avec leur nombre d'exécutions

🔴 **DEUX EXÉCUTIONS ÉTABLISSENT LA REPRODUCTIBILITÉ, JAMAIS UN TAUX.** Les
contrôles de ⑥ sont **déterministes** (spec §9) ; la question « combien de fois
sur combien » ne se pose pas ici et **n'est pas empruntée** à une campagne qui,
elle, l'aurait posée. Les deux exécutions rendent des sorties **identiques,
chiffre pour chiffre**.

| # | Critère | Verdict | Exéc. | Le chiffre, **relevé** |
| --- | --- | --- | --- | --- |
| ① | les sept contrôles restent verts | **TENU** | **2** | `6/6 contrôle(s) vert(s)` ; §7.1 **52 paires**, 0 échec, minimum **3,16** ; §7.5 **10** tests |
| ② | la liste d'attente a **RÉTRÉCI**, 28 → 10 | **TENU** | **2** | **48** déclarés / **38** employés / **10** orphelins = **10** en attente, `0 écart` |
| ③ | aucun sélecteur de `primitives.css` ne peut s'appliquer à `index.html` | **TENU** | **2** | `primitives.test.ts` **9 passed** — G1 à G7 |
| ④ | le poids CSS reste sous le plafond | **TENU** | **2** | **6 374** octets / plafond **12 288**, marge **5 914** |
| — | le **jugement visuel** de `primitives.html` | ⛔ **NON PORTÉ** | **0** | voir le §⑥ |

Suites : `client` **196** tests (22 fichiers), `proto` **79** (4 fichiers),
`typecheck` `exit 0` des deux côtés. ⚠️ **Le mouvement de `proto` (70 → 79)
N'EST PAS DE S2** — aucune tâche du sous-bloc ne touche `proto/`.

🔴 **LE CRITÈRE ③ NE DIT PAS « S2 NE CHANGE RIEN À LA FENÊTRE DE SESSION ».**
`tokens.css` a gagné un token, donc `socle-*.css` change d'octets (2 010 →
2 179, relevé) et `index.html` le charge. ③ établit qu'**aucune règle de
`primitives.css` ne peut sélectionner un élément d'`index.html`** — la seule
chose qui compte pour le rendu, et la seule qu'une commande sache dire.

### ② 🔴 Le fait le plus réutilisable : une liste d'attente rétrécit parce qu'un contrôle la force

Le contrôle §7.6 exige l'**ÉGALITÉ** entre l'ensemble des orphelins et une liste
nommée — **pas une inclusion, pas un seuil**. Il échoue donc **dans les deux
sens** : un orphelin **absent** de la liste (`NOUVEL ORPHELIN`), et une entrée de
la liste qui **a gagné** un appelant (`À RETIRER DE LA LISTE`). **C'est la
seconde moitié qui compte : elle rend la liste AUTO-NETTOYANTE.**

**28 → 10 en QUATRE commits, et chaque retrait a été LU dans la sortie du contrôle,
jamais deviné** (relevé par `git show <commit> -- client/outils/tokens-orphelins.mjs`) :

| Tâche | Commit | Retirés | Compte |
| --- | --- | --- | --- |
| bouton | `78eb679` | `--fond-1` `--fond-2` `--bord` `--bord-fort` `--texte` `--texte-faible` `--sur-accent` `--r-1` `--duree-1` `--trait` | **10** |
| champ | `b00099a` | `--danger` `--t-l` | **2** |
| surface | `21393e0` | `--t-xl` `--lh-serre` `--e-4` `--r-3` | **4** |
| message | `4c5d9e2` | `--succes` `--alerte` | **2** |
| galerie | `b9c706d` | **aucun** — elle ajoute une **exclusion**, pas un appelant | **0** |

**28 − 18 = 10.** ⚠️ **Le plan prédisait une AUTRE répartition** (8/2/5/3), et
l'écart n'est pas une dérive d'exécution : **le plan se contredisait lui-même** —
son tableau de prédiction assignait `--bord` à la tâche 4 et `--texte` à la
tâche 5, quand le Step 4 de sa tâche 2 les prescrivait tous deux au bouton.
L'implémenteur a suivi **le contrôle**, comme le plan l'ordonne lui-même. **La
prédiction se trompait sur la répartition, jamais sur la somme.**

⚠️ **L'EXCLUSION DES GALERIES EST PORTEUSE, ET C'EST MESURÉ** :
`tokens-orphelins.mjs --sans-exclusion` rend **9 écarts**, `exit=1` — neuf tokens
que **seules** `design.html` et `primitives.html` emploient, dans leur propre
mise en page. Sans elle, la liste rétrécirait **sans que le produit ait gagné un
seul appelant**.

⚠️ **LE PÉRIMÈTRE « EMPLOYÉ » EST LE FICHIER, JAMAIS LA SURFACE.** À la fin de
S2, les dix-huit tokens sortis ont **un appelant ÉCRIT, pas un pixel RENDU** :
`primitives.css` n'est liée que par sa galerie. **C'est S3 qui referme cet
écart.**

### ③ 🔴 Deux mesures qui ont tranché une conception, et une troisième qui a réfuté le plan

**a) `color-mix(…, black)` est refusé par le contrôle §7.2** — la voie
« composer le survol au rendu » est fermée par une commande, pas par un avis.
⚠️ **Relevé le 19 août 2026 par la CONCEPTION du plan (D2), pas rejoué par la
recette** ; arbre jetable, aucun fichier du dépôt touché :

```
$ node client/outils/couleurs-litterales.mjs --racine /tmp/s2probe
/tmp/s2probe/src/x.css:1: .b { background: color-mix(in oklab, var(--accent) 88%, black); }
couleurs littérales : 1
exit=1
```

D'où **un token neuf, `--accent-survol`**, quatorzième couleur par thème là où la
spec en fixait treize. Contrastes **relancés par `rapportDeContraste` le 20 août
2026** : sombre repos **7,73** → survol **9,39** ; clair repos **5,72** → survol
**7,27**. Les deux survols **augmentent** le contraste, et le minimum global
reste **3,16**. ⚠️ **Le CHOIX des teintes reste un jugement humain** ; seul leur
contraste est mesuré.

**b) `prefers-reduced-motion` NE PEUT PAS vivre dans `tokens.css`** — mesuré le
19 août 2026 sur une **copie jetable** de `tokens.css`, relevé recopié dans
l'en-tête de la règle (`client/src/design/base.css`), **pas rejoué par la
recette** :

```
$ node client/outils/blocs-de-theme.mjs --fichier /tmp/s2-rm.css
  bloc racine : 48 token(s)
  bloc media-clair : 14 / attribut-clair : 14 / media-clair : 1   ← le QUATRIÈME bloc
écarts : 15
exit=1
```

`lireBlocsDeTheme` **ne sait nommer que trois blocs** : tout `:root` sous un
`@media` sans `[data-theme="clair"]` s'appelle `media-clair`, et §7.4 compare
alors ce quatrième bloc à la racine. **La règle vit donc dans `base.css`.**

**c) 🔵 UN TROU DE §7.4, MESURÉ, ET IL SURVIVRA À CE SOUS-BLOC.**
`ecartsEntreBlocs` compare **clair ⇄ clair** et **clair ⊆ racine** ; il ne
compare **JAMAIS racine ⊆ clair**. `--accent-survol` retiré des **deux** blocs
clairs et laissé à la racine seule :

```
  bloc racine : 48 token(s)
  bloc media-clair : 13 token(s)
  bloc attribut-clair : 13 token(s)
écarts : 0
exit=0
```

**Quarante-huit contre treize, et zéro écart.** ⚠️ **Conséquence directe : la
rouge de §7.4 que le plan prescrivait était IMPOSSIBLE selon le bloc choisi** —
elle n'est rouge que si le token est posé dans **un seul des deux blocs clairs**,
et le libellé du plan ne le disait pas. **Un token de couleur oublié dans les
deux blocs clairs est invisible à ce contrôle.**

### ④ Les onze divergences du plan, en une phrase chacune

| # | Sort |
| --- | --- |
| D1 | la liste rangeait `--succes`/`--alerte`/`--danger` en S3, la spec les confie à S2 : **la spec l'emporte**, et le contrôle l'aurait forcé |
| D2 | l'état survol n'a aucun token, `color-mix` refusé **sur mesure** → **`--accent-survol`**, 50 → **52** paires, minimum inchangé |
| D3 | `prefers-reduced-motion` **hors de `tokens.css`** — quinze écarts, mesurés |
| D4 | `design.html` à 231 pour une porte à 300 → **cinquième entrée Vite**, `primitives.html`, **décidée AVANT l'addition** |
| D5 | 🔴 `verify-all.sh` compte **DIX** étapes ; une **exécution complète** affiche **DIX-SEPT** en-têtes `==>` (10 + les 7 de `design:verifier`) ; une passe de `design:verifier` en affiche **SEPT**. **Ni « dix » ni « dix-sept » ne se suffit sans dire lequel on compte** — et cette fois les trois nombres sont **relevés**, plus sommés |
| D6 | trois tokens annotés S2 décrivent une famille étiquette/pastille que la spec ne confie pas à S2 → **re-étiquetés avec leur raison** |
| D7 | le périmètre « employé » de §7.6 est le **fichier**, jamais la surface → **tenu, et déclaré sans le maquiller** |
| D8 | `--police-mono` reste à **S4** — **non rouvert**, et la raison est inscrite auprès de l'entrée |
| D9 | les comptes de tests bougent sous les voisins → **arrivé** : `proto` 70 → 79, **pas de notre fait** |
| D10 | aucun contrôle ne mesure une longueur → **G4**, rouge versée, et sa portée déclarée étroite |
| D11 | l'anneau de focus existe déjà ; le risque est de l'**effacer** → **G2**, rouge versée |

### ⑤ Chaque garde a été vu ROUGE, et une rouge ne vaut que pour son assertion

Neuf gardes, et **douze mutations versées** (`rouge-{1..4}-*.log`,
`rouges-rejouees.log`, `blanchiment-neutralise.log`), chacune jouée **seule**,
arbre restauré et restauration **vérifiée** par `git status --porcelain client/`
vide. Le compte `n failed | m passed` est la preuve que la mutation n'a
fait tomber **que** l'assertion visée.

🔴 **G5 EST LE SEUL GARDE D'ATTEIGNABILITÉ, ET SA ROUGE LE MONTRE** : sur des
feuilles vidées de leurs règles, **G1 à G4 restent VERTS** — `4 failed |
5 passed`, les quatre tombés étant G5 et les trois G6. **Sans G5, quatre gardes
sur cinq ne prouveraient rien.**

### ⑥ ⛔ Le jugement visuel : NON PORTÉ, et ce n'est de toute façon pas un critère

Personne n'a ouvert `dist/primitives.html`. **C'est déclaré, pas remplacé par un
« probablement ».** Un agent qui prendrait une capture d'écran ne porterait pas
un jugement — il produirait une image que personne n'a regardée.

Ce qui **a** été fait est une **mesure**, étiquetée comme telle : compter les
classes dans le `dist/primitives.html` bâti (11 839 octets) — les dix classes
attendues y sont. **Cela dit que la page mentionne les quatre familles. Cela ne
dit RIEN de son apparence.**

**Les huit jugements humains du §8 restent entiers, et S2 en ajoute TROIS** :
que `#93b4f9`/`#2650b4` soient les **bonnes** valeurs de survol ; que l'état
**actif** dit « par le retour au repos » soit lisible ; que **quatre tons de
message distingués par la seule encre** suffisent. **Aucun des onze ne deviendra
une mesure.**

### ⑦ Pièges neufs — à connaître avant de toucher à ce terrain

- 🔴 **UN GARDE DE FORME EST UN TEST D'ABSENCE, DONC VERT SUR UN FICHIER VIDE.**
  D'où **G5**. Mesuré, pas raisonné : feuilles vidées → G1 à G4 tous verts.
- 🔴 **UN GARDE QUI CHERCHE UNE SOUS-CHAÎNE EST SATISFAIT PAR SON PROPRE
  COMMENTAIRE**, puisque l'en-tête écrit ce qu'il interdit. D'où le blanchiment.
  ⚠️ **Mais sa portée réelle est plus étroite qu'on ne l'écrit spontanément, et
  elle est MESURÉE** : sans lui, **G1** remonte **889** compounds parasites dont
  `/*` (et **G5** avec, il lit les mêmes préludes) ; **G2, G3 et G4 restent
  verts** — ils lisent une **position de propriété** dans une déclaration, pas
  une sous-chaîne.
- 🔴 **UN GARDE PEUT ÊTRE FAUX EN SENS INVERSE DE CE QU'ON CRAINT.** G7 affirmait
  que sans blanchiment il « resterait VERT ». **Mesuré : il ne reste pas vert.**
  Sa première assertion est bien satisfaite par le commentaire, la seconde tombe
  — et **elle tombe AUSSI sur un `base.css` intact**, le commentaire portant le
  bloc `{ :root { --duree-1: … } }` du relevé. **Sans blanchiment, ce garde rend
  le MÊME rouge que la règle soit présente ou absente : il cesse de
  discriminer.** Le blanchiment n'est pas ce qui l'empêche d'être vert à tort,
  c'est ce qui le rend capable de dire quoi que ce soit.
- 🔴 **UNE ADDITION DE COMMENTAIRE ANNULE UNE EXTRACTION, et S2 l'a payé DEUX
  fois dans le même fichier.** `tokens-orphelins.mjs` devait maigrir : 18 entrées
  retirées (**−18**) contre **+49** de commentaire, **233 → 270**. Puis le
  constat écrit pour le dire, posé en tête du fichier, l'a porté à **296 — marge
  4** : *un encadré qui dénonçait la dérive la produisait.* **Remède du dépôt
  appliqué — extraire, jamais compresser** : la liste part avec **toute sa
  doctrine** vers `client/outils/tokens-orphelins/attente.mjs`, et le fichier
  retombe à **202**.
- ⚠️ **UN RE-ÉTIQUETAGE N'EST VU PAR AUCUN CONTRÔLE** — il compare des ensembles
  de **noms**. C'est **le point le plus faible de S2**. ❌ **Et le plan avait tort
  d'écrire qu'aucune mitigation n'est possible** : *aucune entrée ne doit nommer
  un sous-bloc déjà clos* **pourrait échouer utilement**, et serait passée au
  rouge à la fin de S2 sur les trois entrées annotées « S2 ».
- ⚠️ **UNE PAGE ABSENTE DE `vite.config.ts` NE SORT PAS DU BUILD, ET RIEN NE LE
  DIT.** Les contrôles §7.2, §7.3 et §7.6 **lisent** la liste des entrées Vite au
  lieu de la recopier — c'est ce qui leur a fait attraper `primitives.html` sans
  qu'aucune ligne de script ne change.
- ⚠️ **UN HOOK `chpwd` DU SHELL DE L'HÔTE INJECTE UN `ls` DANS CHAQUE JOURNAL.**
  La première passe de journaux de S2 portait une liste de fichiers en tête de
  **chaque** log, à cause d'un `cd` dans un sous-shell. `unset -f chpwd` avant
  toute collecte. **Un journal pollué par l'instrument est une pièce fausse.**
- ⚠️ **`tokens.ts` ne sait nommer que TROIS blocs** : `tokens.css` ne peut donc
  accueillir aucune autre requête média. Le jour où un sous-bloc en voudra une,
  c'est `lireBlocsDeTheme` qui doit apprendre à nommer ses blocs, **pas la règle
  qui doit se contorsionner**.

### ⑧ La revue transverse — douze affirmations devenues fausses DANS la branche

Elle a trouvé **cinq** défauts en D7, **trois** en D8, **six** en D9, **douze**
en D10, **sept** en D11, **huit** en P1, **dix** en P2, **cinq** en S1, **neuf**
sur le chantier E et **douze** en P3. **Douze ici**, et toutes ont la même forme :
**correctes des deux côtés prises séparément.**

- **Le compte des paires, 50 → 52**, corrigé aux **deux** endroits que le plan
  nommait et laissé à **quatre** autres — dont **deux dans `contraste.ts` et
  `contraste.test.ts`, les fichiers mêmes que la tâche 2 éditait**. C'est le
  naufrage du « 487 » sous sa forme la plus banale.
- **`672` compounds parasites** annoncé par l'en-tête de `primitives.test.ts` :
  **remesuré 889**. Le 672 datait de la tâche 2, quand `bouton.css` était seul.
- **G7 « resterait VERT »** : réfuté par mesure (§⑦).
- **« Aucune mitigation du re-étiquetage n'est possible »** : réfuté (§⑦).
- **Trois énoncés au FUTUR que S2 a rendus faux en ayant lieu** — `design.html`
  promettait la scission « au même rythme que les primitives de S2 » (**elle a eu
  lieu, mais ailleurs** : `primitives.html` est née à côté, et cette page-ci n'a
  jamais franchi sa porte) et « S2 est le sous-bloc qui en pose ».
- **Deux énoncés sans date devenus trompeurs** : « le SEUL ajout d'apparence de
  ce sous-bloc » (`base.css`, où S2 a ajouté un second bloc qui change bien
  l'apparence) et « à la fin de ce sous-bloc » (`style.css`).
- **L'encadré `--police-mono` de `tokens.css`** portait « 28 tokens en attente »,
  « les 27 autres », et un chemin qui n'existe plus.

🔴 **ET LA REVUE TRANSVERSE EST ELLE-MÊME UNE SOURCE DE CROISSANCE : elle a
ajouté ~48 lignes de commentaire à la branche, et fait tomber la marge de
`primitives.test.ts` de 30 à 17.** C'est le piège de la ligne précédente rejoué
par la ronde qui le dénonce, pour la deuxième fois du sous-bloc. **Déclaré, et
la règle est armée** (§⑨).

### ⑨ Tailles relevées **par la commande**, au commit `697735e`

⚠️ **Chaque ligne a été mesurée APRÈS la dernière édition de code**, jamais
relue d'un document. Journal : `journaux-design-s2/tailles.txt` (relevé
antérieur, à `070b48b`) ; le tableau ci-dessous est le relevé **final**.

**Dépôt entier, fichiers de plus de 500 lignes : DEUX**, les deux lignes de la
dette gelée — `agent/src/encode.rs` **1536**, `agent/src/windows_source.rs`
**630**. **Aucun fichier de `client/`.**

| Fichier | Lignes | Porte | Marge |
| --- | --- | --- | --- |
| `client/src/design/primitives.test.ts` | **283** | 300 | 🔴 **17** |
| `client/design.html` | **243** | 300 | 57 |
| `client/src/design/tokens.css` | **232** | 300 | 68 |
| `client/outils/tokens-orphelins.mjs` | **202** | 300 | 98 |
| `client/primitives.html` | **199** | 300 | 101 |
| `client/src/style.css` | **184** | 300 | 116 |
| `client/src/design/galerie.ts` | **162** | 300 | 138 |
| `client/src/design/contraste.ts` | **156** | 300 | 144 |
| `client/outils/tokens-orphelins/attente.mjs` | **146** | 300 | 154 |
| `client/src/design/base.css` | **134** | 300 | 166 |
| `client/src/design/contraste.test.ts` | **125** | 300 | 175 |
| `client/src/design/primitives/bouton.css` | **103** | 300 | 197 |
| `client/src/design/selecteur-theme.ts` | **84** | 300 | 216 |
| `client/src/design/primitives/champ.css` | **75** | 300 | 225 |
| `client/src/design/primitives.css` | **73** | **240** | 167 |
| `client/src/design/primitives/message.css` | **52** | 300 | 248 |
| `client/src/design/primitives/surface.css` | **44** | 300 | 256 |
| `client/src/design/galerie-primitives.ts` | **18** | 300 | 282 |

🔴 **MARGE LA PLUS ÉTROITE DE S2 : `primitives.test.ts`, 283 pour une porte à
300, marge 17** — elle valait **30** avant la revue transverse. **Toute addition
substantielle à ce fichier appelle une EXTRACTION, jamais une compression**, et
le point de chute est nommé d'avance : **scinder par objet** — les gardes de
forme (G1 à G5, G7) d'un côté, les trois gardes de famille (G6) de l'autre.
⚠️ **Attention en le faisant** : G5 compare la liste `FAMILLES` du test aux
`@import` de `primitives.css`, et c'est ce qui empêche une cinquième famille
d'échapper à G1-G4.

✅ **`primitives.css` a été EXTRAIT AVANT de franchir sa porte, et c'est une
première dans ce dépôt.** Relevé par `git cat-file -p <commit>:<chemin>` :
**154** (bouton) → **230** (champ, marge **10**) → la tâche surface **extrait
avant d'écrire** → **72** → **73**. **Le plafond n'a jamais été franchi**, là où
ce dépôt l'a franchi trois fois en D10 et deux fois en D9, et rattrapé deux fois
par une compression qu'il interdit.

⚠️ **`client/verify-webrtc.mjs` vaut 494 (marge 6), INCHANGÉ** — aucune tâche de
S2 ne le touche. ⚠️ **`client/src/main.ts` vaut 451, et ce n'est pas de S2** : il
était à 392 en D10, le chantier E l'a porté là.

**Poids CSS bâti** : 1 493 + 2 702 + 2 179 = **6 374** octets, plafond **12 288**,
marge **5 914**. Base S1 : **3 503** — **S2 ajoute 2 871 octets**, dont 2 702
pour la seule feuille des primitives, **que S2 ne lie à aucune page du produit**.
⚠️ **Le plafond de 12 288 n'est calibré par rien**, et il le reste.

**`scripts/verify-all.sh` sort à `0` en entier**, **deux fois** : à la recette
(`verify-all.log`, commit `070b48b`, 3 186 lignes) et **relancé après les
corrections de la revue transverse** (`verify-all-final.log`, commit `7b036b0`),
**17** en-têtes `==>` les deux fois.
⚠️ **Aucune étape étrangère n'est tombée, et c'est à relever plutôt qu'à taire** :
`cargo test --workspace` a passé sur un `agent/` que le chantier voisin du pont
de fichiers modifiait au même instant, et `plateforme : npm run test:postgres` a
trouvé son instance. **Il n'y a donc rien à attribuer à personne.**

### ⑩ Ce que S2 n'établit PAS

- **Aucun taux, nulle part.** Deux exécutions par critère, contrôles
  **déterministes** : reproductibilité, rien de plus.
- ⛔ **AUCUNE SURFACE DU PRODUIT N'EMPLOIE UNE PRIMITIVE.** Les dix-huit tokens
  sortis ont **un appelant écrit, pas un pixel rendu**.
- 🔴 **Aucun jugement visuel n'a été porté**, et les **onze** jugements humains
  attendent tous un œil.
- **Rien hors d'un Chromium de bureau** : ni Firefox, ni Safari, ni mobile, ni
  HiDPI.
- **L'accessibilité au-delà du contraste et du mouvement réduit** n'est pas
  mesurée : clavier complet, lecteurs d'écran, cibles tactiles, ordre de
  tabulation. **Aucune primitive ne porte de rôle ARIA** — c'est du CSS.
- **L'anneau de focus est vérifié NON EFFACÉ, jamais VISIBLE** : rien ne dit
  qu'il se voie sur un bouton principal focalisé, dont le fond est `--accent`.
  `outline-offset` l'en écarte de 2 px — **raisonnement, pas mesure**.
- **Aucun contrôle ne mesure une longueur** : G4 vérifie qu'une longueur passe
  par un token, **jamais que le bon token a été choisi**.
- **La clause « aucune longueur hors échelle » du §8 reste FAUSSE** — les trois
  littéraux de `style.css` vivent dans la fenêtre de session, **c'est S4**.
  ❌ **ELLE EST FAUSSE DE SIX, PAS DE TROIS, DEPUIS S3** (`72rem`, `18rem` dans
  `shell.css` ; `26rem` dans `connexion.css`), et **la spec §8 porte désormais
  l'encadré qui le dit**, avec sa règle de compte. Les trois de `style.css`
  restent bien à S4.
- 🔴 **§7.4 ne compare jamais racine ⊆ clair** (§③c). ✅ **FERMÉ PAR S3
  (tâche 1)** : la clause ③ compare désormais **les COULEURS de racine ⊆ clair**,
  hors six tokens hors thème nommés, et sa rouge est versée
  (`journaux-design-s3/rouge-1-7-4-couleur-hors-clair.log`).
- 🔴 **Les trois re-étiquetages ne sont vus par aucun contrôle.**
- ⚠️ **Ni `primitives.html` ni `galerie-primitives.ts` n'ont de test**, comme
  `design.html` et `galerie.ts` : **une galerie qui cesserait de rendre une
  famille entière ne serait attrapée par aucun contrôle** — seulement par l'œil,
  et l'œil n'est pas passé.
- **Aucune internationalisation** ; **le legacy n'est pas touché** et aucun
  contrôle ne le balaie.

### ⑪ Ce que S2 lègue

**À S3 :**

1. ✅ **FAIT (S3, tâches 4 et 5).** `primitives.css` est lié par `shell.html` et
   `connexion.html`, et **ce n'est pas une intention mais un relevé** : le
   contrôle **§7.9**, né avec S3, mesure que les quatre familles atteignent le
   produit. ~~Lier `primitives.css` aux surfaces du produit.~~
2. ✅ **FAIT (S3, tâches 4 et 5) : neuf sur neuf sortent**, chacune dans le
   commit qui écrit son appelant. La liste tombe de **10 à 1**. ⚠️ Les **trois
   re-étiquetées** vers une famille étiquette/pastille ont bien trouvé cette
   famille — c'est la pastille d'état d'une fenêtre, `.bureau__pastille`.
3. ✅ **FAIT (S3, tâche 6) : le sélecteur est PROMU au produit**, il a des tests,
   et sa promotion a obligé à corriger un défaut déclaré (il ne rappelait pas
   `marquer()` après un `storage` venu d'une autre fenêtre — rouge versée).
4. ❌ **NON FAIT, et ce n'est plus tout à fait le même legs** : ni
   `primitives.html` ni `galerie-primitives.ts` n'ont de test. **Mais §7.9
   assertion ③ B en prend une part** — « une famille cesse d'être rendue par la
   galerie » est désormais attrapé par une commande. **Un module de galerie
   cassé, non.**

**À S4 :**

5. ⛔ **`--police-mono`** — la seule entrée dont le sort soit encore ouvert : ou
   S4 le câble sur `#stats`, ou il le retire. **S2 ne l'a pas rouvert**, et pas
   par omission.
   ✅ **S4 A CÂBLÉ (20 août 2026)**, et la liste d'attente est VIDE.
6. ⛔ **Les trois longueurs hors échelle de `style.css`**, et la clause du §8
   qu'elles rendent fausse. ⚠️ **Elles sont SIX à la fin de S3** — voir le §⑩
   ci-dessus ; les trois de `style.css` restent celles que S4 doit reprendre.
   ✅ **LES SIX SONT TOMBÉES (S4)**, et la clause est mesurée par **§7.10**.

**Sans sous-bloc assigné :**

7. ✅ **CONSTRUITE PAR S3 (tâche 7).** `SOUS_BLOCS_CLOS = {S1, S2, S3}` vit dans
   `attente.mjs`, le sous-bloc est devenu un **champ structuré**, et
   `tokens-orphelins.mjs` rend rouge toute entrée réclamant un sous-bloc clos —
   rouge versée. ⚠️ **PARTIELLE, et le mot reste pesé** : elle juge le sous-bloc
   NOMMÉ, jamais le CONTENU de l'annotation, et dépend d'une liste tenue à la
   main. ⚠️ **Et elle ne garde plus qu'UNE entrée après S3.**
8. ✅ **FERMÉ PAR S3 (tâche 1).** `ecartsEntreBlocs` compare désormais les
   **COULEURS** de racine ⊆ clair, hors six tokens hors thème nommés. ⚠️ **La
   restriction aux couleurs est délibérée** : les échelles ne vivent que dans la
   racine et n'ont aucune contrepartie claire.
9. ⛔ **`tokens.ts` ne sait nommer que trois blocs.**
10. ⛔ **Le plafond de 12 288 octets n'est calibré par rien**, et S2 en consomme
    **6 374**. ⚠️ **S3 en consomme 8 011**, marge **4 277** — le plafond n'est
    toujours calibré par rien.
11. 🔴 **`primitives.test.ts` est à 283 pour une porte à 300, marge 17** : toute
    addition substantielle appelle une extraction, dont le point de chute est
    nommé au §⑨. ⚠️ **INCHANGÉ APRÈS S3, À 283** : aucune tâche n'y a ajouté
    d'assertion, et la revue transverse n'y a modifié que trois mots de
    commentaire (« sept » → « huit »), à longueur égale.
    ✅ **TRAITÉ PAR S4, ET AVANT L'ADDITION** (tâche 1) : le lecteur de feuille
    est extrait vers `client/src/design/css.ts`, **pour que les gardes neufs de
    S4 le réemploient au lieu de le recopier**. Le fichier est à **245** après la
    revue transverse — relevé par la commande. ⚠️ **Le point de chute nommé au
    §⑨ reste ouvert** : cette extraction sort l'OUTIL, pas les gardes, et laisse
    G5 auprès de sa source. Elle est **complémentaire, pas substitutive**.
12. ⛔ **Le défaut à deux réglages de `build-agent.sh`/`run-agent.sh`** ne
    concerne pas ⑥ — ses journaux sont propres —, mais il reste **non corrigé**
    pour les chantiers qui passent par la VM.

---

## 🎨 Sous-projet ⑥ Design system — sous-bloc S3 : les deux surfaces habillées (20 août 2026)

Recette : `docs/superpowers/plans/2026-08-19-design-system-s3-resultats.md`.
Plan : `docs/superpowers/plans/2026-08-19-design-system-s3.md` (`b57e6b8`).
Spécification : `docs/superpowers/specs/2026-08-19-design-system-design.md`
(`5b6b830`) — **MODIFIÉE par S3**, contrairement à S1 et S2 : son §7 gagne le
**§7.9**, son §7.4 porte l'encadré de sa portée élargie, et son §8 celui de la
clause « aucune longueur hors échelle », **fausse de SIX valeurs**.
Journaux : `docs/superpowers/plans/journaux-design-s3/` — **52 fichiers**,
**UNE SEULE FAMILLE DE LECTURE**, et c'est **mesuré, pas supposé**
(`familles-de-lecture.txt`) :

| Famille | État | Ce qu'il faut faire |
| --- | --- | --- |
| **tous** les fichiers | **aucune séquence ANSI** (`grep -lP '\x1b\['` → aucun), **aucun `\r`**, UTF-8 partout (`file`) | **rien** — ils se `grep`ent à plat |

⚠️ **`familles-de-lecture.txt` a dû être REFAIT parce qu'il se polluait
lui-même** : sa première rédaction écrivait ses propres motifs avec
`echo "…\x1b\[…"`, et **zsh interprète les échappements** — le fichier a reçu un
vrai octet ESC et un vrai retour chariot, et `file(1)` l'a classé « with CR, LF
line terminators, with escape sequences ». **Un lecteur pressé aurait conclu que
S3 verse des journaux CRLF.**

⛔ **AUCUNE TÂCHE DE S3 N'A EMPLOYÉ LA VM WINDOWS** (spec §9).
**Variables d'environnement introduites : AUCUNE.** *Une absence se déclare.*

🔴 **S3 EST LE PREMIER SOUS-BLOC DE ⑥ OÙ L'APPARENCE DU PRODUIT BOUGE.** S1
s'interdisait tout changement ; S2 n'a lié ses primitives à aucune surface et
l'a écrit — *« un appelant ÉCRIT, pas un pixel RENDU »*. **Cet écart est
refermé, et c'est un contrôle qui le dit.**

### ① Les cinq critères, avec leur nombre d'exécutions

🔴 **DEUX EXÉCUTIONS ÉTABLISSENT LA REPRODUCTIBILITÉ, JAMAIS UN TAUX.** Les
contrôles de ⑥ sont **déterministes** (spec §9) ; « combien de fois sur
combien » **ne se pose pas ici et n'est pas emprunté** à une campagne qui, elle,
l'aurait posé. Les deux exécutions rendent des sorties **identiques, caractère
pour caractère** — seule la durée de build diffère (721 ms contre 303 ms).

| # | Critère | Verdict | Exéc. | Le chiffre, **relevé** |
| --- | --- | --- | --- | --- |
| ① | **les HUIT contrôles sont verts** | **TENU** | **2** | `7/7 contrôle(s) vert(s)`, `exit=0`, plus §7.5 dans `npm test`. **§7.1 INCHANGÉ : 52 paires, 0 échec, minimum 3,16** |
| ② | la liste d'attente a **RÉTRÉCI**, **10 → 1** | **TENU** | **2** | **48** déclarés / **47** employés / **1** orphelin = **1** en attente, `0 écart`, **13** fichiers au périmètre |
| ③ | 🔴 **la fenêtre de session N'A PAS BOUGÉ** | **TENU sur ses TROIS volets** | **2** | (a) diff de **0 octet** ; (b) `dist/index.html` lie `socle-ZRS7erzW.css` et `main-CXZaIG5L.css`, **et rien d'autre** ; (c) les deux actifs **octet pour octet** ceux de la base |
| ④ | les primitives **atteignent le produit** | **RELEVÉ** | **2** | §7.9 : **52** déclarées / **51** employées, **0 écart** |
| ⑤ | le poids CSS sous le plafond | **TENU** | **2** | **8 011** / **12 288**, marge **4 277** ; **+1 637** contre S2 |
| — | le **jugement visuel** des deux surfaces | ⛔ **NON PORTÉ** | **0** | voir le §⑤ |

**Suites** : `client` **258** tests / **26** fichiers (219 / 24 à l'écriture du
plan), `proto` **130** / **5**, `typecheck` `exit 0` des deux côtés, **deux
exécutions chacun**. ⚠️ **Le mouvement de `proto` n'est pas de S3** — aucune
tâche du sous-bloc ne touche `proto/`.

🔵 **LE CRITÈRE ③ A ÉTÉ MESURÉ CONTRE UNE BASE RECONSTRUITE, PAS SUPPOSÉE** :
`git archive 88bc963 client proto` vers un arbre jetable hors du dépôt,
`node_modules` lié, `npx vite build`. **Base `88bc963`, parent du premier commit
de S3 — et non le `920a1eb` du plan** : des chantiers voisins ont touché
`client/` entre les deux (`connexion.ts`, `fichiers/adaptateur.ts`,
`prefixe.ts`, relevé), et prendre `920a1eb` aurait attribué à S3 leur travail.
🔵 **Et le montage sait voir un changement, dans la même exécution** : **sept**
actifs de `dist/assets` changent de nom entre la base et HEAD, et **deux
feuilles CSS neuves apparaissent**. Un montage qui rendrait « identique » sur
tout ne prouverait rien.

### ② Ce que S3 change VISUELLEMENT — le contrat, et un huitième point hors plan

Le §5.1 du plan est un **CONTRAT** : tout ce qui n'y figure pas est une
régression. Ses **sept** lignes sont livrées — page-shell composée (grille de
cartes), écran de connexion en carte centrée, **trois boutons de thème
apparaissent** sur les deux surfaces, et **trois bandeaux prennent un TON**
(neutre / danger), testé.

🔵 **UN HUITIÈME CHANGEMENT, HORS PLAN, DÉCLARÉ PLUTÔT QUE DISSIMULÉ** :
`.message:empty { display: none }` — **un bandeau vide disparaît, il ne devient
pas un cadre vide.** Sans elle, l'état initial des trois bandeaux et
l'effacement délibéré de `shell.ts::lecteurDemonte` laisseraient un rectangle
bordé sans texte : *l'effacement se lirait comme un défaut d'affichage.*
⚠️ **Elle touche une PRIMITIVE de S2**, donc la galerie — et deux choses la
rendent acceptable, la seconde **mesurée** : le §6.3 du plan exige qu'une règle
écrite à l'identique dans les deux feuilles de surface **remonte** dans les
primitives, et `primitives.html` **n'a aucun `.message` vide**, vérifié avant
l'écriture. ⚠️ **Divergence de point de chute déclarée** : le plan nommait
`primitives/surface.css` ; la règle vit dans `primitives/message.css`, auprès de
ce qu'elle décrit.

### ③ La liste d'attente : 10 → 1, et la mitigation que S2 déclarait impossible

**Commit par commit** (`critere-2-attente.log`) : 10 à la base, **3** après la
tâche 4 (page-shell), **1** après la tâche 5 (connexion), inchangé ensuite.
**Aucune entrée n'est ENTRÉE.** Les neuf sorties, avec leur appelant relevé par
`grep` **dans le périmètre du contrôle** : `--e-1` `--e-5` `--e-6` `--e-7`
`--r-plein` `--t-2xl` `--t-xs` (`shell.css`), `--t-3xl` `--lh-large`
(`connexion.css`). **Aucun appelant n'a été fabriqué pour vider une ligne.**

🔵 **S2 ÉCRIVAIT TROIS FOIS QU'AUCUNE MITIGATION DU RE-ÉTIQUETAGE N'EST
POSSIBLE. C'EST FAUX, ET S3 L'A CONSTRUITE** (tâche 7) : le sous-bloc nommé est
devenu un **champ structuré** (`sousBloc: 'S4'`), et
`SOUS_BLOCS_CLOS = {S1, S2, S3}` rend rouge toute entrée réclamant un sous-bloc
clos — **rouge versée**, `SOUS-BLOC CLOS --police-mono nommait S3, qui est clos`.
⚠️ **PARTIELLE, et le mot reste pesé** : elle juge le sous-bloc **NOMMÉ**, jamais
le **CONTENU** de l'annotation, et dépend d'une liste tenue à la main.
⚠️ **Et après S3 elle ne garde qu'UNE entrée — un mécanisme pour une ligne.**
L'objection est réelle ; la réponse est qu'une mitigation construite **après** la
faute qu'elle devait empêcher n'aurait plus rien à empêcher.

### ④ Les deux contrôles que S3 change — §7.4 élargi, §7.9 neuf

🔵 **§7.4 : L'ANGLE MORT QUE S2 AVAIT MESURÉ EST FERMÉ.** Son énoncé n'est plus
« les trois blocs déclarent le même ensemble de noms » : c'est ① clair ≡ clair,
② clair ⊆ racine, ③ **les COULEURS de racine ⊆ clair**, sauf **six** tokens hors
thème nommés. ⚠️ **La restriction aux couleurs est délibérée** — les échelles ne
vivent que dans la racine. **Rouge versée**, et il a fallu la refaire (§⑥).

🔵 **§7.9 EST UNE ADDITION DE PLAN, PAS DE LA SPEC**, et elle y est désormais
inscrite. Trois assertions : ① toute classe employée est **déclarée** ; ② A **au
moins une famille atteint une surface du PRODUIT** ; ③ B **chaque famille est
rendue par la galerie**. **Les familles sont DÉRIVÉES des fichiers de
`src/design/primitives/`, jamais énumérées** : une cinquième entre dans ② et ③
sans qu'une ligne du script ne change.
🔴 **L'assertion ② A EST NÉE ROUGE SUR L'ARBRE INTACT** (commit `45f5521`) et
la branche l'a portée rouge jusqu'à `83f4bbf` — **c'est sa preuve
d'atteignabilité**, et c'est ce que S1 avait fait entre ses tâches 1 et 9.
Relevé final :

```
② A — les primitives atteignent le PRODUIT :
  client/index.html : aucune famille
  client/shell.html : bouton, message, surface
  client/connexion.html : bouton, champ, message, surface
```

⚠️ **`index.html` n'en emploie AUCUNE, et c'est voulu** : c'est la règle ① des
primitives, et le critère ③ la mesure.

### ⑤ ⛔ Le jugement visuel : NON PORTÉ — et S3 ajoute QUATRE jugements humains

**Personne n'a ouvert `dist/shell.html` ni `dist/connexion.html`.** Déclaré,
**jamais remplacé par un « probablement »**, jamais par une capture que personne
n'a regardée. **Ce n'est pas un critère** (plan, risque n°10), et S2 ne l'avait
pas porté non plus.

🔴 **TOTAL À LA FIN DE S3 : QUINZE JUGEMENTS HUMAINS** — huit de la spec §8,
trois de S2, **quatre de S3** : que la **grille de cartes** soit la bonne forme
pour une liste de fenêtres ; que la **carte de connexion centrée** soit à la
bonne largeur ; que l'état **ouverte / fermée** dit par la seule **encre** d'une
pastille se distingue assez ; que **trois boutons côte à côte** soient la bonne
forme de sélecteur de thème. **Aucun des quinze ne deviendra une mesure.**
⚠️ Pour le troisième, ce qui **est** mesuré est **le contraste** de l'encre
employée, sur les trois fonds (§7.1) — **distinguer deux états n'est pas lire un
texte**, et WCAG ne le mesure pas ici.

### ⑥ 🔴 Le résultat de MÉTHODE : quatre rouges sur seize ne prouvaient rien

**16 rouges et 2 contre-épreuves de blanchiment**, une mutation à la fois, avec
la même discipline **sans exception** : `sha256` avant → mutation → **PREUVE que
le diff est non vide** → contrôle → `git checkout --` → `sha256` identique →
`git status --porcelain` vide. **Le harnais est le vrai livrable de méthode : il
refuse de compter une rouge dont le diff est vide.**

**QUATRE ont dû être REFAITES, et c'est ce qu'il faut retenir :**

- **§7.4** — la mutation portait `^    --bord-fort` (quatre espaces) et
  n'atteignait qu'**UN** des deux blocs clairs, l'autre étant indenté de huit
  sous son `@media`. Le contrôle rougissait bien… **sur la clause ①
  PRÉEXISTANTE**, pas sur la clause ③ que S3 ajoute. **Une rouge qui rougit pour
  la mauvaise raison ne prouve pas ce qu'on lui fait dire.**
- **§7.9 ② A** — elle ne mutait qu'**une** surface sur deux, et y introduisait
  des classes inventées : `connexion.html` gardant ses quatre familles, ② A
  restait **satisfaite**, et l'`exit=1` venait de la clause ①. Refaite sur **les
  deux** surfaces en ne retirant **que** des classes déclarées.
- **§7.9 ③ B** et **le sélecteur de thème** — leurs mutations **n'ont rien
  muté** (0 ligne de diff, `exit=0`). **Le harnais l'a dit lui-même.**

🔵 **LE BLANCHIMENT EST ÉPROUVÉ DANS LES DEUX SENS**, ce que S2 n'avait fait que
dans un : une couleur littérale dans un **commentaire CSS** laisse §7.2 **vert**,
un `class="…"` dans un **commentaire HTML** laisse §7.9 **vert** — **et** une
classe déclarée *seulement* dans un commentaire CSS **ne compte pas comme
déclarée**, donc l'employer **rougit**. C'est le piège maison — *« un garde
satisfait par le commentaire du fichier qu'il analyse »* — attrapé des deux
côtés.

### ⑦ La revue transverse — treize affirmations devenues fausses DANS la branche

Elle a trouvé **cinq** défauts en D7, **trois** en D8, **six** en D9, **douze**
en D10, **sept** en D11, **huit** en P1, **dix** en P2, **cinq** en S1, **neuf**
sur le chantier E, **douze** en P3, **douze** en S2 et **onze** en F1. **Treize
ici**, et toutes ont la même forme : **correctes des deux côtés prises
séparément.**

🔴 **LE COMPTE DE CONTRÔLES, « sept » → « huit » : TREIZE places, et QUATRE où
« sept » est JUSTE et ne doit PAS bouger.** Les places ont été **énumérées par
`grep -n` AVANT toute édition** (`revue-transverse-enumeration.log`), corrigées
**une par une par NUMÉRO DE LIGNE** — jamais par substitution globale — puis
**relues place par place APRÈS**. Les treize : `primitives.css` ×2,
`amorce-theme.js`, `base.css`, `reprise.test.ts` ×2, `galerie.ts`,
`primitives.test.ts` ×3, `primitives/champ.css`, `style.css`, `tokens.css`.
⚠️ **Les quatre intouchées le sont pour une raison mesurée** :
`verifier-design.mjs` ×3 et `classes-employees.mjs` parlent des **SEPT
SCRIPTS** que l'agrégateur lance, pas des huit contrôles — `verifier-design.mjs`
nomme lui-même §7.5 « LE HUITIÈME CONTRÔLE ». **Sept scripts, huit contrôles.**
⚠️ Trois autres « sept » sont les **sept crans typographiques** : rien à voir.

**Les douze autres, chacune avec sa place :**

| Affirmation | Place | Sort |
| --- | --- | --- |
| « ⛔ AUCUNE SURFACE DU PRODUIT NE LE LIE […] C'est S3 qui referme cet écart » | `primitives.css:38-42` | **corrigée** — deux surfaces le lient, et le relevé de §7.9 est inscrit |
| « à la fin du sous-bloc S2, aucune surface du produit n'emploie de primitive » | `primitives.html:114` | **corrigée** |
| « `--e-1` est encore en liste d'attente » | `primitives/champ.css:20` | **corrigée** — il est sorti à la tâche 4 ; le choix `--e-2` ne bouge pas, sa RAISON si |
| « c'est S3 qui en aura besoin […] et c'est à lui de poser `--bord-fort` » | `primitives/surface.css:11` | **corrigée** — S3 a fait AUTREMENT : la carte reste INERTE et c'est un `.bouton--discret` **posé dans** la carte qui porte l'action. **`--bord-fort` n'est toujours pas posé** |
| « le jour où S3 posera un lien en bouton » | `primitives/bouton.css:22` | **corrigée** — S3 n'a posé **aucune ancre** (D11, mesuré) ; la règle reste juste et **non exercée** |
| « le balisage de S3 **devra** donner au message son `role` » | `primitives/message.css:20` | **corrigée** — fait : trois `role="status"` dans le HTML |
| « désormais de **CINQ** valeurs et non de trois » | `shell.css:46` | **corrigée en SIX** — juste à sa date (tâche 4), réfutée par la tâche 5 qui a ajouté `26rem` sans reprendre la phrase |
| la clause « aucune longueur hors échelle » | **spec §8** | **corrigée** — encadré, tableau des six, et **la règle de compte énoncée AVANT de compter** |
| « Sept contrôles » | **spec §7** | **corrigée** — encadré : huit, dont §7.9, **addition de plan** |
| « les trois blocs déclarent le même ensemble de noms » | **spec §7.4** | **corrigée** — encadré de la portée élargie, avec le trou mesuré par S2 |
| « les **dix** entrées ci-dessous » / « les dix restants » (×3) | `attente.mjs:108,114,121` | **corrigées** — déictiques cassés : il en reste **une** |
| « les trois jugements humains que S2 ajoute aux huit » | `primitives.html:34` | **complétée** — ils sont **quinze** à la fin de S3 |

🔴 **ET LA REVUE TRANSVERSE EST ELLE-MÊME UNE SOURCE DE CROISSANCE : +54 lignes
de commentaire.** `shell.css` 159 → **167**, `primitives.css` 73 → **85**,
`connexion.css` 93 → **103**, `bouton.css` 103 → **108**, `champ.css` 75 →
**80**, `surface.css` 44 → **50**, `message.css` 71 → **74**, `primitives.html`
199 → **203**, `attente.mjs` 226 → **227**. **C'est le piège que S2 avait déjà
payé** — sa propre revue avait fait tomber la marge de `primitives.test.ts` de
30 à 17. ✅ **Ce fichier-là n'a PAS bougé cette fois : 283, marge 17** — trois
mots de commentaire, à longueur égale.

🔵 **ET LA REVUE A TROUVÉ QUELQUE CHOSE QU'AUCUNE TÂCHE N'AURAIT PU VOIR :
`client/src/design/amorce-theme.js` PART VERBATIM DANS CHAQUE PAGE BÂTIE,
COMMENTAIRES COMPRIS.** Le greffon `guac-amorce-theme` de `vite.config.ts` lit
son texte **brut** et le rend tel quel dans le `<head>`, sans aucun transform —
le fichier le dit de lui-même. **Conséquence mesurée** : la correction d'**un
mot** dans son commentaire fait différer `dist/index.html` de celui de la base,
**à taille rigoureusement égale (3 661 octets des deux côtés)**, sur **une seule
ligne**. ⚠️ **Le critère ③ tel que le plan l'exige reste TENU** — ses volets (b)
et (c) portent sur les `<link>` et sur les **deux actifs**, tous inchangés ; la
comparaison octet à octet de `dist/index.html` est une exigence que la recette
s'est **ajoutée**. ⛔ **Le mot n'a pas été remis à « sept » pour faire passer le
contrôle : il était FAUX, et ce dépôt n'échange pas une vérité contre un
nombre.**

### ⑧ `verify-all.sh` : DIX-HUIT en-têtes, `exit 0`, deux fois

```
$ grep -c '^==> ' verify-all.log
18
```

**DIX-HUIT, et c'est RELEVÉ, jamais sommé** — dix étapes du script plus les
**huit** en-têtes d'une passe de `design:verifier` (un build et **sept**
contrôles). C'est la divergence D5 de S2, qui se rejoue à chaque addition ; le
plan prédisait dix-huit, **la mesure le confirme, elle ne le remplace pas**. Les
deux exécutions rendent la **même liste** (`diff` vide).

🔵 **AUCUNE ÉTAPE ÉTRANGÈRE N'EST TOMBÉE, ET IL FAUT DIRE POURQUOI C'EST
REMARQUABLE.** Le brief de cette recette annonçait `verify-all.sh` **ROUGE** sur
`plateforme : npm run typecheck` — `agents/canal.ts:147`, du fait de
l'élargissement d'union livré par **G1** (`3bb7487`). **Remesuré au moment
d'écrire : l'étape passe.** La réparation est arrivée **pendant** cette recette,
par le chantier voisin, commit **`9da33a1` — « apps(g1) : les deux branches du
canal, et l'arbre redevient VERT »**. ⚠️ **Ni le rouge ni sa réparation ne sont
de S3** : aucune tâche du sous-bloc ne touche `plateforme/`. **Nommé ici pour
que personne ne le porte au débit de ⑥** — et parce qu'une recette qui trouve
l'arbre vert sans dire qu'elle l'attendait rouge laisse croire qu'elle n'a rien
regardé. ⚠️ Un `test:sqlite` **transitoirement** rouge avait été signalé hors
périmètre de S3 : **il n'a pas été revu ici**, les deux exécutions le passent.

### ⑨ Tailles relevées **par la commande**, APRÈS les éditions de la revue

⚠️ **Le journal `tailles.txt` porte LES DEUX relevés** — avant et après la revue
— pour qu'on voie ce qu'elle a coûté. **Celui-ci est le second, et il fait foi.**

**Dépôt entier, fichiers de plus de 500 lignes : DEUX**, les deux lignes de la
dette gelée — `agent/src/encode.rs` **1536**, `agent/src/windows_source.rs`
**630**. **Aucun fichier de `client/`.**

| Fichier | Lignes | Porte | Marge |
| --- | --- | --- | --- |
| `client/verify-webrtc.mjs` | **494** | 500 | 🔴 **6** — ⚠️ **intouché par S3** |
| `client/src/design/primitives.test.ts` | **283** | 300 | 🔴 **17** — **inchangé** |
| `client/src/design/tokens.ts` | **244** | 300 | 56 |
| `client/design.html` | **243** | 300 | 57 |
| `client/src/design/tokens.test.ts` | **241** | 300 | 59 |
| `client/outils/tokens-orphelins.mjs` | **235** | 300 | 65 |
| `client/src/design/tokens.css` | **232** | 300 | 68 |
| `client/src/design/classes.ts` | **227** | 300 | 73 |
| `client/outils/tokens-orphelins/attente.mjs` | **227** | 300 | 73 — 🔴 **13 avant le seuil d'extraction de 240** |
| `client/src/shell.test.ts` | **216** | 300 | 84 |
| `client/src/shell-page.ts` | **216** | 300 | 84 |
| `client/primitives.html` | **203** | 300 | 97 |
| `client/outils/classes-employees.mjs` | **202** | 300 | 98 |
| `client/src/design/selecteur-theme.ts` | **189** | 300 | 111 |
| `client/src/style.css` | **184** | 300 | 116 |
| `client/src/design/selecteur-theme.test.ts` | **171** | 300 | 129 |
| `client/src/shell.css` | **167** | 300 | 133 |
| `client/src/design/classes.test.ts` | **163** | 300 | 137 |
| `client/src/shell.ts` | **154** | 300 | 146 |
| `client/src/design/primitives/bouton.css` | **108** | 300 | 192 |
| `client/src/connexion.css` | **103** | 300 | 197 |
| `client/shell.html` | **88** | 300 | 212 |
| `client/src/design/primitives.css` | **85** | **240** | 155 |
| `client/src/design/primitives/champ.css` | **80** | 300 | 220 |
| `client/src/design/primitives/message.css` | **74** | 300 | 226 |
| `client/connexion.html` | **66** | 300 | 234 |
| `client/src/design/primitives/surface.css` | **50** | 300 | 250 |
| `client/src/design/amorce-theme.js` | **35** | 300 | 265 |

🔴 **LA MARGE À SURVEILLER EST CELLE D'`attente.mjs` : 227 lignes pour un seuil
d'extraction conditionnel à 240 — treize.** Le plan le nomme : *« si l'addition
de doctrine porte le fichier au-delà de 240, extraire — jamais compresser, et la
doctrine part avec sa donnée »*. S2 y a payé **deux fois** la leçon « une
addition de commentaire annule une extraction » ; S3 l'a porté de 146 à 227.
**La prochaine addition substantielle appelle l'extraction.**

**Poids CSS bâti**, et **la revue ne l'a pas changé d'un octet** — les
minificateurs retirent les commentaires : `546 + 1 493 + 2 730 + 1 063 + 2 179 =
**8 011**`, plafond **12 288**, marge **4 277**. **Base S2 REMESURÉE sur l'arbre
reconstruit, pas reprise du document : 6 374** — donc **+1 637**, dont **+546**
`connexion.css`, **+1 063** `shell.css`, **+28** `primitives.css`
(`.message:empty`). **`main` et `socle` n'ont pas bougé d'un octet.**
⚠️ **Le plafond de 12 288 n'est calibré par rien**, et il le reste.

### ⑩ Ce que S3 n'établit PAS

- **Aucun taux, nulle part.** Deux exécutions par critère, contrôles
  **déterministes** : reproductibilité, rien de plus.
- 🔴 **Aucun jugement visuel n'a été porté**, et les **quinze** jugements humains
  attendent tous un œil. **Aucune page n'a été ouverte dans un navigateur.**
- **Rien hors d'un Chromium de bureau** : ni Firefox, ni Safari, ni mobile,
  **ni HiDPI**.
- **L'accessibilité au-delà du contraste et du mouvement réduit** : clavier
  complet, lecteurs d'écran, cibles tactiles, ordre de tabulation. **Aucune
  primitive ne porte de rôle ARIA** — la sémantique reste au balisage.
- 🔴 **L'anneau de focus reste vérifié NON EFFACÉ, jamais VISIBLE** — et S3 est
  le premier sous-bloc où un `.bouton--principal` focalisé **existe réellement**
  sur une page du produit, **sans que rien ne mesure qu'on le voie sur
  `--accent`**.
- **Aucun contrôle ne mesure une longueur** : G4 vérifie qu'une longueur passe
  par un token, **jamais que le bon token a été choisi**. **La clause du §8 est
  fausse de SIX valeurs**, et la spec le porte désormais.
- **La bascule de thème entre deux fenêtres RÉELLES du produit** n'est pas
  éprouvée : `selecteur-theme.test.ts` la couvre par **injection** (`client/` n'a
  ni jsdom ni happy-dom), et S1 l'avait corroborée hors critère entre deux
  onglets de la **galerie** — **jamais entre une page-shell et les N sessions
  qu'elle ouvre**.
- **`galerie-primitives.ts` et `galerie.ts` n'ont toujours pas de test.** §7.9 ③ B
  attrape « une famille cesse d'être rendue » ; **pas un module de galerie
  cassé**.
- **Le sens « toute classe déclarée est employée » n'existe pas** dans §7.9 : il
  relève l'écart (52 / 51) **sans le juger**.
- **Aucune primitive « lien »**, **aucune ancre** dans aucune des deux entrées.
- **Aucune internationalisation** ; **le legacy n'est pas touché** et aucun
  contrôle ne le balaie.
- ⚠️ **Le legs n°9 de S2 reste entier** : `lireBlocsDeTheme` ne sait toujours
  nommer que **trois** blocs — la tâche 1 ne l'a pas touché.

### ⑪ Ce que S3 lègue

**À S4 :**

1. ⛔ **`--police-mono`** — la seule entrée de liste d'attente dont le sort soit
   encore ouvert : ou S4 le câble sur `#stats`, ou il le retire. **S3 ne l'a pas
   rouvert**, et la mitigation de la tâche 7 empêchera qu'il soit re-étiqueté en
   silence.
   ✅ **CÂBLÉ PAR S4 (tâche 6)** : la liste d'attente est **VIDE**, 52 déclarés
   pour 52 employés, et elle **reste** — supprimer le fichier supprimerait
   l'ÉGALITÉ elle-même. **L'énoncé de S1 « le jour où elle est vide, tout ce bloc
   disparaît avec elle » est CORRIGÉ plutôt qu'exécuté.**
2. ⛔ **Les SIX longueurs hors échelle**, et **non trois** — `6px`, `18px`,
   `0.02em` (`style.css`), `72rem`, `18rem` (`shell.css`), `26rem`
   (`connexion.css`). **Les trois de `style.css` sont celles que S4 doit
   reprendre**, la fenêtre de session lui appartenant.
   ✅ **LES SIX SONT TOMBÉES (S4, tâches 5 à 8)**, et la clause du §8 cesse d'être
   une dette d'énoncé : **§7.10** la mesure, et rend **0 occurrence, 0 valeur**
   contre **8 occurrences / 6 valeurs** à sa naissance.
3. ⛔ **L'écran plein cadre des états terminaux** et le **Window Controls
   Overlay** (spec §6). ⚠️ **WCO dépend du manifest de ②**, que ⑥ ne livre pas.
   ✅ **L'ÉCRAN EST LIVRÉ (S4, tâche 9)**, avec ses six tests, dont celui du SENS
   INVERSE. 🔴 **LE WCO EST LIVRÉ SANS AUCUN CRITÈRE DE RECETTE, et c'est
   délibéré** : aucun manifeste n'existant, aucun état atteignable ne fait agir la
   règle, et un critère vacueux se lirait comme une preuve. Un **garde de forme**
   prouve qu'elle est **INERTE**, jamais qu'elle fonctionne. **Destinataire nommé
   du legs : la recette du sous-bloc G5 de la gestion d'apps.**
4. ⛔ **La fenêtre de session tout entière** — S3 ne l'a pas touchée, et le
   critère ③ le **mesure**. C'est le dernier sous-bloc où cette phrase est vraie.
   ✅ **ET ELLE L'EST : S4 L'A TOUCHÉE**, et son critère ③ le mesure **dans le sens
   inverse, par le même montage** — `git diff --stat` non vide, et **les CINQ**
   actifs CSS bâtis changent de hachage.
5. 🔴 **`attente.mjs` à 227 pour un seuil d'extraction à 240** : la prochaine
   addition de doctrine **extrait**.
   ✅ **TENU PAR S4 (tâche 6)** : la doctrine est partie vers
   `client/outils/tokens-orphelins/sous-blocs-clos.mjs`, et `attente.mjs` est à
   **221** — relevé par la commande après la revue transverse.

**Sans sous-bloc assigné :**

6. ⛔ **Le hub** — il n'existe pas, son contenu dépend de ④, et **⑥ ne le livre
   pas** (spec §6, §9).
7. ⛔ **Aucune primitive « lien »** ; **aucune ancre** dans aucune entrée. Poser
   une famille sans appelant serait le code mort que §7.6 refuse.
8. ⛔ **`galerie-primitives.ts` et `galerie.ts` sans test.**
9. ⛔ **Le legs n°9 de S2** : `lireBlocsDeTheme` ne nomme que trois blocs.
10. ⛔ **Le plafond de poids CSS n'est calibré par rien.**
11. ⛔ **Le sens « toute classe déclarée est employée »** de §7.9 : il exigerait
    une seconde liste d'attente. **C'est `primitives.html` et l'œil qui le
    tiennent** — et l'œil n'est pas passé.
12. ⛔ **`primitives.test.ts` à 283 pour une porte à 300, marge 17** : toute
    addition appelle une **extraction**, dont le point de chute est nommé depuis
    S2 — les gardes de forme d'un côté, les gardes de famille de l'autre, **sans
    séparer G5 de sa source**.
13. ⛔ **Le défaut à deux réglages de `build-agent.sh`/`run-agent.sh`** ne
    concerne pas ⑥ — ses journaux sont propres —, mais il reste **non corrigé**.

### ⑫ Pièges neufs — à connaître avant de toucher à ce terrain

- 🔴 **UNE ROUGE QUI ROUGIT POUR LA MAUVAISE RAISON NE PROUVE RIEN**, et elle
  est **indiscernable d'une bonne** si l'on ne lit que son `exit=1`. Quatre sur
  seize étaient dans ce cas. **Le remède est un harnais** : exiger, entre la
  mutation et le contrôle, une **preuve que le diff est non vide** — et lire
  QUELLE assertion a rougi, jamais seulement le code de sortie.
- 🔴 **`client/src/design/amorce-theme.js` PART VERBATIM DANS CHAQUE PAGE
  BÂTIE, COMMENTAIRES COMPRIS.** Corriger un mot de son en-tête change les cinq
  pages de `dist/`. **Aucun des huit contrôles ne le dirait** : §7.7 ne pèse que
  le CSS.
- ⚠️ **UN JOURNAL PEUT SE POLLUER LUI-MÊME** : `echo` en zsh interprète `\x1b`
  et `\r`, si bien qu'un fichier qui **décrit** ses motifs de recherche les
  **contient**. `file(1)` l'a alors classé « with escape sequences ». **Écrire
  ces motifs avec un heredoc entre quotes.**
- ⚠️ **`"$var:chemin"` EN ZSH MANGE LE `:c`** comme modificateur de paramètre :
  `git show "$c:client/…"` a rendu `88bc963lient/…` et un compte de **0** pour
  **toutes** les lignes d'un tableau — **indiscernable d'une mesure**. Écrire
  `"${c}:client/…"`.
- ⚠️ **LES OUTILS DE `client/outils/` SE LANCENT DEPUIS LA RACINE DU DÉPÔT**,
  pas depuis `client/` : `poids-css.mjs` y résout `--dist` à `client/dist` et
  rend « dist/assets est absent », `exit=2`. C'est `npm run design:verifier` qui
  masque la contrainte.
- ⚠️ **UN `grep` SANS `-a` CLASSE « BINAIRE » ET REND UNE SORTIE VIDE, PAS
  ZÉRO** — piège de D10, à connaître pour tout journal.
- ⚠️ **UN HOOK `chpwd` DU SHELL DE L'HÔTE INJECTE UN `ls` DANS CHAQUE JOURNAL**
  dès qu'un `cd` court dans un sous-shell (S2 a dû reprendre une passe entière).
  **`unset -f chpwd` avant toute collecte** — appliqué ici sans exception.
- ⚠️ **UNE BASE DE COMPARAISON SE CHOISIT SUR LES COMMITS, PAS SUR UNE DATE** :
  le plan nommait `920a1eb`, mais des chantiers voisins avaient touché `client/`
  entre lui et le premier commit de S3. **Prendre le parent du premier commit du
  sous-bloc**, sinon on s'attribue le travail des voisins.


---

## 🎨 Sous-projet ⑥ Design system — sous-bloc S4 : la fenêtre de session (20 août 2026)

Résultats complets :
`docs/superpowers/plans/2026-08-20-design-system-s4-resultats.md`.
Plan : `docs/superpowers/plans/2026-08-20-design-system-s4.md`.
Spécification : `docs/superpowers/specs/2026-08-19-design-system-design.md`.
Journaux : `docs/superpowers/plans/journaux-design-s4/` — **UNE SEULE FAMILLE DE
LECTURE, et c'est la première fois du sous-projet** :

| Famille | État | Ce qu'il faut faire |
| --- | --- | --- |
| **tous** les fichiers de ce répertoire | UTF-8 valide, **ni séquence ANSI, ni `\r`, ni octet NUL** — relevé par la commande APRÈS la dernière écriture (`familles-de-lecture.txt`) | **rien** : ils se `grep`ent à plat, sans `sed`, sans `grep -a` |

⚠️ **Ce n'est pas un mérite** : les trois familles de D6, les quatre de D8 et les
trois de D9 viennent toutes d'un chemin qui passe par la **VM Windows**, dont
`run-agent.sh` et `build-agent.sh` ne posent pas `[Console]::OutputEncoding`.
⛔ **S4 n'a pas touché la VM** — ⑥ est un sous-projet **navigateur** (spec §9).
Le défaut à deux réglages reste entier ; il n'est simplement pas rencontré.

**S4 est le DERNIER sous-bloc de ⑥.** Il reprend la fenêtre de session — la seule
surface que ⑥ n'avait jamais touchée — et solde les trois questions que la
spécification lui laissait.

### ① Le fait qui gouverne le sous-bloc n'était dans aucun document : un défaut d'ENCRE

**Sous le thème clair, la fenêtre de session écrivait du quasi-noir sur un voile
quasi-noir.** `base.css` pose `color: var(--texte-fort)` sur `body` ; en clair
`--texte-fort` vaut `#10131a` ; et les six voiles sont **hors thème**, donc noirs
dans les deux.

**Ce n'est pas une régression du produit d'origine : c'est un effet de bord de
S1.** Avant lui, `style.css` posait `color-scheme: dark` en dur et une encre
unique — **la fenêtre de session n'avait pas de thème clair**. S1 lui en a donné
un, et rien n'a remarqué que les voiles, eux, ne suivaient pas.

**Aucun contrôle ne pouvait le voir, et pour une raison écrite** : les six voiles
sont hors des paires de contraste, « leur lisibilité dépend de la vidéo qui est
dessous, qui n'est pas connaissable ». ⚠️ **L'argument est juste pour le VOILE ; il
ne l'est pas pour l'ENCRE**, qui, elle, est parfaitement connaissable. Remède : un
**septième** token hors thème, `--sur-voile: #e6e8eb`, et une **53ᵉ** paire —
`--sur-voile` sur `--video-letterbox`, la **seule** région où le fond sous l'encre
soit connu (les bandes que laisse `object-fit: contain`).

### ② Les sept critères, avec le nombre d'exécutions dans chaque énoncé

⚠️ **DEUX EXÉCUTIONS ÉTABLISSENT LA REPRODUCTIBILITÉ, JAMAIS UN TAUX.** Les neuf
contrôles de ⑥ sont **déterministes** : la question « combien de fois sur
combien » **ne se pose pas ici et ne doit pas être empruntée** à une campagne qui,
elle, l'aurait posée.

| # | Critère | Verdict | Exéc. | Le chiffre, **relevé** |
| --- | --- | --- | --- | --- |
| ① | les **NEUF** contrôles sont verts | **TENU** | 2 | `7/7` scripts, `exit=0` ; **279** tests / **30** fichiers |
| ② | la liste d'attente est **VIDE** | **TENU** | 2 | **52** déclarés / **52** employés — **ÉGAUX**, `0 orphelin` |
| ③ | la fenêtre de session **A BOUGÉ** | **TENU** | 2 | **182 insertions / 43 suppressions** ; **les CINQ** actifs CSS changent de hachage |
| ④ | elle emploie des primitives | **TENU** | 2 | `index.html : message, surface` ; **les TROIS** surfaces couvertes |
| ⑤ | plus aucune longueur hors token | **TENU** | 2 | **0 occurrence, 0 valeur**, contre **8 / 6** à la naissance du contrôle |
| ⑥ | les contrastes | **TENU** | 2 | **53** paires, **0** échec, minimum global **3,16** — inchangé |
| ⑦ | le poids CSS | **TENU** | 2 | **8 616** octets, plafond **12 288**, marge **3 672** (base : **8 011**) |
| — | le **jugement visuel** | ⛔ **NON PORTÉ** | 0 | voir ⑥ ci-dessous |

**La base du critère ③ est le PARENT DU PREMIER COMMIT du sous-bloc** (`23e9b89`),
jamais une date — leçon que S3 avait déjà payée. ⚠️ **Les deux exécutions ne
diffèrent que par l'ORDRE d'arrivée de deux lignes de `vitest`** : aucun nombre ne
change.

### ③ 🔴 Le WCO : la règle est livrée, et AUCUN critère de recette ne la couvre

**Il n'existe AUCUN manifeste dans ce dépôt** — mesuré. Sans
`display_override: ["window-controls-overlay"]`, les variables `titlebar-area-*`
ne sont **jamais définies** : il n'y a donc **aucun état atteignable** où la règle
agisse. **Un critère qui prétendrait l'exercer serait vacueux PAR CONSTRUCTION,
et un critère vacueux est pire qu'un critère absent : il se lit comme une preuve.**

Ce qui est livré à la place est un **garde de forme** (`client/src/style.test.ts`)
à trois assertions : ① tout `env(titlebar-area-*)` porte le repli `0px` ;
② **aucune** `@media (display-mode: window-controls-overlay)` — car un repli
neutralise un `env()`, mais **rien ne neutralise un bloc `@media`** ;
③ **atteignabilité**.

**Sa rouge a une conséquence RÉELLE aujourd'hui** : un repli non nul descend le
bandeau **maintenant**, sur toutes les sessions. Ce n'est donc pas un contrôle qui
valide sa propre écriture. 🔴 **Mais il prouve l'INERTIE, jamais le COMPORTEMENT.**
**Destinataire nommé du legs : la recette du sous-bloc G5 de la gestion d'apps**,
celui qui pose le manifeste.

### ④ La revue transverse — VINGT-SEPT affirmations, et un plafond franchi

Barème : cinq en D7, trois en D8, six en D9, douze en D10, sept en D11, huit en
P1, dix en P2, cinq en S1, neuf sur E, douze en P3, douze en S2, onze en F1, huit
en P4, **treize** en S3, huit en G1.

🔴 **LE FAIT LE PLUS NET : trois affirmations étaient DÉJÀ FAUSSES LE JOUR OÙ
ELLES ONT ÉTÉ ÉCRITES.** Les trois « aucun des huit contrôles » posés par la
**tâche 4** (`tokens.css`, `style.css`, `design/contraste.ts`) décrivent la suite
de contrôles telle qu'elle était **avant la tâche 2**, qui l'avait déjà changée —
`git merge-base --is-ancestor ee56e1e fb629ea` l'établit. **Deux commits d'écart,
dans la même branche.**

**LE TRI COMPTE AUTANT QUE LES CORRECTIONS**, et c'est la leçon de S3 rejouée :
`grep -rniE 'huit contrôles?'` rendait **17** places — dont **DEUX que le `grep`
sensible à la casse manquait**. **QUINZE** étaient fausses au présent et sont
corrigées ; **TROIS** sont des **citations** en style direct et **restent
justes** ; et **NEUF autres emplois du mot « huit » nomment un AUTRE compte**
(crans d'espacement, jugements humains du §8, dix-huit tokens, huit fenêtres,
huit occurrences), **tous vérifiés INTACTS après coup**. Une substitution globale
les aurait abîmés.

**SEPT étaient fausses EN SUBSTANCE** : « aucun ne mesure une longueur » ne l'est
plus, et **ce qui laisse `client/src/design/` découvert est désormais une PORTÉE**
— §7.10 l'exclut, G4 y garde les quatre familles — **et non une absence de
contrôle**.

**TRANCHÉ PLUTÔT QU'EXÉCUTÉ** : « le raccordement sémantique du micro appartient
au sous-bloc S4 » (S1). Le faire ferait suivre au bouton **le thème du produit**
alors qu'il est posé sur une vidéo qui n'en suit aucun — **mot pour mot
l'argument que la même page emploie six lignes plus haut**. **La phrase promettait
ce que sa propre page réfute.**

**§7.10 EST INSCRIT DANS LA SPEC**, « parce qu'un contrôle qui ne vit que dans un
plan de sous-bloc se perd ». La spec passe de huit à **NEUF** contrôles et dit
**sept scripts, neuf contrôles** ; sa clause §8 cesse d'être fausse ; la réserve
de portée de `--police-mono` est levée ; et **deux numéros de ligne qui avaient
dérivé sont RETIRÉS plutôt que corrigés**.

🔴 **LE PLAFOND A ÉTÉ FRANCHI, ET RATTRAPÉ PAR UNE EXTRACTION.**
`client/src/style.css` est monté à **301 pour une porte à 300** ; les deux boutons
de coin partent **VERBATIM** vers `client/src/session/boutons-de-coin.css`
(**138**), et `style.css` retombe à **194**. Ce dépôt a franchi ce plafond **trois
fois en D10 et deux fois en D9**, et l'a rattrapé **deux fois par une compression
qu'il interdit nommément**.

### ⑤ Les tailles, PAR LA COMMANDE, APRÈS la revue transverse

⚠️ **La revue transverse est une source de croissance connue** — S2 y a perdu 13
lignes de marge, S3 y a ajouté **+54 lignes**. **Le relevé qui fait foi est celui
d'APRÈS**, jamais celui d'avant.

| Fichier | Lignes | Porte | Marge |
| --- | --- | --- | --- |
| 🔴 `client/src/design/tokens.css` | **300** | 300 | **0** |
| `client/src/design/primitives.test.ts` | 245 | 300 | 55 |
| `client/outils/tokens-orphelins/attente.mjs` | 221 | 240 | 19 |
| `client/src/style.test.ts` | 219 | 300 | 81 |
| `client/src/style.css` | **194** | 300 | 106 (**301** avant l'extraction) |
| `client/src/session/boutons-de-coin.css` | 138 | 300 | 162 (**neuf**) |
| `client/src/ecran-terminal.test.ts` | 108 | 300 | 192 (**neuf**) |
| `client/src/ecran-terminal.ts` | 105 | 300 | 195 (**neuf**) |
| `client/src/session/etat-terminal.css` | 80 | 300 | 220 (**neuf**) |
| `client/src/main.ts` | **460** | — | **+9** depuis la base (le plan en autorisait dix) |
| `client/verify-webrtc.mjs` | **494** | 500 | 🔴 **6** — ⛔ **intouché par S4**, comme par S1, S2 et S3 |

🔴 **`design/tokens.css` est à 300 EXACTEMENT : sa marge est NULLE.** Son point de
chute est nommé dans le fichier lui-même — scinder en `tokens/couleurs.css` et
`tokens/echelles.css` — et **la prochaine addition l'exige, jamais une
compression**. ⚠️ **Cette injonction ne peut pas être écrite DANS le fichier
qu'elle concerne : l'y écrire le ferait franchir.** Elle vit donc ici. La
scission a été **délibérément écartée** en fin de branche : **SEPT lecteurs**
nomment `tokens.css` par son chemin (`outils/contraste.mjs`,
`outils/couleurs-litterales.mjs`, `outils/blocs-de-theme.mjs`,
`outils/tokens-orphelins.mjs`, et trois `?raw` — `design/reprise.test.ts`,
`design/tokens.test.ts`, `design/galerie.ts`), et le legs n°9 de S2 est encore
ouvert.

**Aucun fichier du dépôt ne dépasse sa porte du fait de S4.**

### ⑥ Les jugements humains — VINGT-CINQ, et aucun n'a été porté

**Quinze à la fin de S3** (huit de la spec §8, trois de S2, quatre de S3 — relevé
par le document de résultats de S3). **S4 en ajoute DIX, ÉNUMÉRÉS par `git blame`
et non recopiés** : la pile monospace ; 20 px la taille des boutons ; `#e6e8eb`
l'encre sur un voile ; l'écran plein cadre comme forme ; qu'il reste **sans
action** ; les trois mesures de contenant ; le rayon 6 → 4 px ; ⚠️ **la ZONE
occupée** par le bouton, distincte de sa taille ; ⚠️ `--e-8` comme marge du micro
après ce changement ; ⚠️ les **deux libellés de titre** de l'écran terminal.

⚠️ **Le plan en prévoyait SEPT ; il y en a DIX.** Les trois derniers sont des
décisions esthétiques prises à l'exécution, et **les taire les aurait déguisées en
mesures**.

🔴 **ET LE JUGEMENT VISUEL N'A JAMAIS ÉTÉ PORTÉ SUR ⑥, D'UN BOUT À L'AUTRE :
aucune page du sous-projet n'a été ouverte dans un navigateur, ni en S1, ni en S2,
ni en S3, ni en S4.** S4 était la **dernière occasion**, et **il la laisse passer
en le déclarant** — un agent qui prendrait une capture d'écran **ne porterait pas
un jugement**, il produirait une image que personne n'a regardée. Ce n'est pas une
lacune d'exécution : c'est la conséquence assumée du §7.8, qui écarte la
comparaison d'images parce que les polices système rendent différemment d'une
machine à l'autre. **La galerie existe pour cela, et personne ne l'a regardée.**

### ⑦ Les rouges, et celle qui n'a pas de journal

**Onze rouges versées** (`rouges-t9.log`, `rouges-t10.log`, `rouges-t12.log`),
toutes au harnais en sept étapes, **preuve de `git diff --numstat` non vide
comprise** et `sha256` identique après restauration. Le blanchiment est éprouvé
**dans les DEUX sens** : une valeur interdite en commentaire laisse **vert**, et
une déclaration écrite **seulement** en commentaire **ne compte pas comme
déclarée**. **Aucune rouge n'a dû être refaite.**

❌ **LES ROUGES DES TÂCHES 1 À 8 N'ONT AUCUN JOURNAL VERSÉ.** Elles ne sont
rapportées que par leurs **messages de commit** — en git, donc permanents, mais ce
ne sont pas des journaux —, et **deux de ces huit commits (`aa27fb8`, `1c05dde`)
ne contiennent pas même le mot « rouge »**. **La leçon pour un plan suivant : la
consigne de verser doit valoir à la tâche qui JOUE la rouge, pas à celle qui
recette.**

### ⑧ Ce que S4 n'établit PAS

- **Aucun taux, nulle part.**
- 🔴 **Rien du WCO en fonctionnement** — le garde prouve l'inertie, et rien d'autre.
- 🔴 **Aucun jugement visuel**, et les **vingt-cinq** jugements attendent un œil.
- **La lisibilité d'un voile sur une vidéo quelconque** : seule la bande noire est
  mesurée ; **la composition alpha n'est pas outillée**.
- **La zone occupée par les boutons de coin** : l'avance du glyphe `⛶` reste
  inconnue, et le commentaire **cesse de la chiffrer** plutôt que de remplacer une
  estimation par une autre.
- **L'anneau de focus reste vérifié NON EFFACÉ, jamais VISIBLE.**
- **L'accessibilité au-delà du contraste et du mouvement réduit.**
- **Rien hors d'un Chromium de bureau**, ni HiDPI, ni internationalisation.
- **`galerie.ts` et `galerie-primitives.ts` n'ont toujours aucun test.**

### ⑨ Pièges neufs — à connaître avant de toucher à ce terrain

- 🔴 **`[hidden]` PERD CONTRE UNE RÈGLE D'AUTEUR, ET CE N'EST PAS UNE QUESTION DE
  SPÉCIFICITÉ** : le `[hidden] { display: none }` vit dans la feuille de l'**agent
  utilisateur**, et la cascade compare l'**ORIGINE** avant la spécificité. Une
  `.ecran { display: grid }` l'emporte, fût-elle moins spécifique — l'écran serait
  **visible dès le chargement**.
- 🔴 **UNE EXTRACTION DÉPLACE CE QU'UN GARDE D'ABSENCE DOIT SURVEILLER.** Sortir
  une règle d'un fichier laisse le garde **vert** sur le fichier vidé. **Toute
  extraction qui déplace une règle gardée oblige à déplacer, ou à DÉDOUBLER, son
  assertion d'atteignabilité** — payé ici, et la rouge est versée.
- ⚠️ **UN CONTRÔLE À PORTÉE DÉRIVÉE NE SE VIDE PAS EN VIDANT UN FICHIER** : sa
  rouge d'atteignabilité doit vider **TOUS** les porteurs. Une prescription de plan
  qui nomme un fichier unique est **fausse d'une portée dérivée** — mesuré deux
  fois dans ce sous-bloc.
- ⚠️ **UN `grep` SENSIBLE À LA CASSE MANQUE CE QUE LES MAJUSCULES CACHENT** : deux
  des dix-sept places de la revue étaient écrites `HUIT CONTRÔLES`.
- ⚠️ **UN COMPTE DE MENTIONS N'EST PAS UN COMPTE DE CHOSES** : `grep -c 'jugement
  humain'` rend **22** lignes pour **25** jugements. **Énumérer par `git blame`.**
- ⚠️ **ZSH NE DÉCOUPE PAS LES VARIABLES EN MOTS** : `git add $FICHIERS` y passe la
  liste entière comme **un seul chemin**, et échoue sans dire pourquoi.
- ⚠️ **DES BACKTICKS DANS UN `echo` DE JOURNAL EXÉCUTENT UNE COMMANDE** — un
  journal de rouge a porté `command not found: style.css` au milieu de sa prose.
  **Guillemets simples pour toute prose journalisée.**
- ⚠️ **UN FICHIER NE PEUT PAS CONTENIR SA PROPRE TAILLE FINALE** : la ligne que
  `familles-de-lecture.txt` porte sur lui-même est celle de sa rédaction
  **précédente**, conservée telle quelle plutôt que devinée.
- ⚠️ **`design/amorce-theme.js` PART VERBATIM DANS CHAQUE PAGE BÂTIE**,
  commentaires compris : y corriger un mot fait différer les cinq pages de
  `dist/`. Piège de S3, rencontré ici **par la revue transverse**, et une
  **troisième exécution de recette** relève la conséquence exacte — **deux**
  hachages bougent, et **quatre actifs CSS restent octet pour octet identiques**.

### ⑩ Ce que ⑥ laisse ouvert APRÈS S4 — la liste est COMPLÈTE

**S4 étant le dernier sous-bloc, rien de ce qui suit n'a de destinataire dans ⑥.**
**Ce que S4 solde** : `--police-mono`, les six longueurs hors échelle et la clause
§8, l'écran terminal, la reprise des bandeaux sur les primitives, le raccordement
du micro (**tranché non**), et le défaut d'encre du thème clair.

1. ⛔ **Le WCO n'a jamais été rendu** — destinataire nommé : **la recette de ④ G5**.
2. ⛔ **Le hub n'existe pas** ; son contenu dépend de ④.
3. ⛔ **Aucune primitive « lien », aucune ancre.**
4. ⛔ **`galerie.ts` et `galerie-primitives.ts` sans test.**
5. ⛔ **Legs n°9 de S2** : `lireBlocsDeTheme` ne sait nommer que **trois** blocs.
6. ⛔ **Le plafond de poids CSS n'est calibré par rien.**
7. ⛔ **Le sens « toute classe déclarée est employée » n'existe pas** — c'est
   `primitives.html` et l'œil qui le tiennent, **et l'œil n'est pas passé**.
8. ⛔ **`Ton` et `CLASSE_DE_TON` sont dupliqués** entre `shell.ts`, `connexion.ts`
   et `ecran-terminal.ts`. Point de chute d'une unification : la couche `design/`.
9. ⛔ **La lisibilité d'un voile sur une vidéo quelconque n'est pas outillée** — la
   composition alpha est un calcul **pur**, donc à portée de `design/contraste.ts`.
10. ⛔ **L'anneau de focus n'a jamais été vu VISIBLE.**
11. ⛔ **`prefers-reduced-motion` est le seul des quatre manques d'accessibilité
    qui soit pris.**
12. ⛔ **`client/verify-webrtc.mjs` est à 494 pour une porte à 500 — marge 6**, et
    **la divergence de convention que D10 a signalée n'a jamais été tranchée** :
    le § « Portée » ci-dessus ne liste que `client/src/`, la commande l'attrape
    quand même. **C'est une décision de convention, et elle appartient au
    propriétaire du dépôt.**
13. ⛔ **Le défaut à deux réglages de `build-agent.sh` / `run-agent.sh`** ne
    concerne pas ⑥, et reste **non corrigé**.
14. 🔴 **`design/tokens.css` est à 300 pour une porte à 300 : marge NULLE.**
15. ⛔ **Les rouges des tâches 1 à 8 n'ont pas de journal versé.**
16. 🔴 **AUCUN JUGEMENT VISUEL N'A ÉTÉ PORTÉ SUR ⑥, D'UN BOUT À L'AUTRE.**

---

## 🎤 Chantier E — Microphone, bloc E1 : le sens montant (19 août 2026)

Résultats complets :
`docs/superpowers/plans/2026-08-19-micro-resultats.md`.
Plan : `docs/superpowers/plans/2026-08-19-micro.md`.
Spécification : `docs/superpowers/specs/2026-07-28-micro-design.md` — ⚠️ **écrite
le 28 juillet 2026, AVANT les chantiers A, B, C et D** ; le plan porte en tête un
**tableau de vieillissement** qui confronte chacune de ses affirmations portantes
au code d'aujourd'hui. **Ne recopier aucune affirmation de cette spec sans passer
par ce tableau.**
Journaux : `docs/superpowers/plans/journaux-micro/` — **58 fichiers suivis par
git**, et **DEUX familles de lecture** :

| Famille | État | Ce qu'il faut faire |
| --- | --- | --- |
| les neuf `agent-*-plat.log` | UTF-8, **ANSI déjà retirées** | rien |
| les neuf `agent-*.log` bruts | UTF-8, **séquences ANSI PRÉSENTES** | `sed 's/\x1b\[[0-9;]*m//g'`, ou lire le `-plat` jumeau, versé pour chacun |

⚠️ **Un seul fichier exige `grep -a` : `vbcable-etat.log`** (16 octets NUL,
mojibake) — le défaut à deux réglages, **toujours non corrigé**. Sans `-a`,
`grep` rend une sortie **vide**, indiscernable d'un compte nul.

**Trois travaux ont convergé dans cette branche, et il faut les distinguer** :
**E1** (le sens montant), **« A-bis »** (le périphérique audio se désigne), et le
**plafond de dissimulation** (né de la recette E1).

### ① Ce que E1 livre, et où il s'arrête

Le navigateur porte sa voix jusqu'au **PCM décodé dans l'agent**, par une
**seconde m-line audio** `sendonly` côté navigateur donc `recvonly` côté agent,
distincte de celle du chantier A. Offerte **sans piste** ; au clic,
`replaceTrack` la remplit **sans renégociation**.

⛔ **AUCUNE APPLICATION WINDOWS N'ENTEND QUOI QUE CE SOIT** : écrire ce PCM sur
« CABLE Input » est le bloc **E2**, qui n'est pas fait. E3 (l'écho en
multi-fenêtres) est conditionnel à la recette de E2.

Côté agent, la boucle de transport **dépose et rien d'autre** ; tout le travail
— ordre, gigue, dérive, décodage, complément de silence — vit dans
`agent/src/micro.rs`, **pur** et éprouvé sous Linux.

### ② Le résultat le plus utile n'était pas un critère

**Le critère ③ (« le silence ne coupe pas le flux ») est TENU. Et c'est en
lisant les VALEURS des lignes présentes — pas leur nombre — que le défaut est
apparu.** Pendant 60 s de DTX de Chrome (`packetsSent` strictement figé), la
trace ne s'interrompt jamais : 133 lignes, une par seconde. **Mais ce que le
puits rendait n'était pas du silence** — `plc = 50/s`, crête **0,53 à 0,67**,
fréquence errant entre **308 et 393 Hz**. **Un bourdon continu**, qui au bloc E2
serait sorti sur le câble : un utilisateur qui se tait aurait fait entendre un
bourdonnement.

**Ce n'est PAS un défaut de libopus**, et c'est ce qui rend le plafond nôtre.
Lu dans la source vendorée par `audiopus_sys` 0.2.2 : `celt/celt_decoder.c:537`
bascule sur du **bruit** dès la 6ᵉ perte, et `:562`/`:566` font décroître
l'énergie **jusqu'à un plancher où elle se maintient**. **La bibliothèque ne
s'arrête jamais d'elle-même.** Borner la durée dissimulée était à **nous**.

Remède : `agent/src/micro/dissimulation.rs`, **pur** — un budget de **durée
CONSÉCUTIVE** (`PLAFOND_DISSIMULATION = 200 ms`), remis à zéro par toute vraie
trame. Compteur `plc_plafonnees`, **disjoint de `plc`**.

⚠️ **`PLAFOND_DISSIMULATION` vaut le même nombre que `micro::PLAFOND` par
COÏNCIDENCE, pas par dérivation.** Les deux bornent des choses différentes et se
recalibreraient séparément. **Ni l'une ni l'autre n'est calibrée.**

**Mesuré sur la VM, une exécution par bras — aucun taux.** Fenêtre de silence,
**59 lignes de chaque côté** :

| Grandeur | **ROUGE** | **VERT** |
| --- | --- | --- |
| `plc` / s | **50**, sur les 59 s | **0** |
| `plc_plafonnees` / s | **0** | **100** |
| `crete` | **0,526 à 0,673** | **0,000**, sans exception |
| `frequence_hz` | **305,5 à 398,5**, errante | **« aucune »** sur les 59 |

**Le rouge est un vrai rouge par CONDUITE** — le mécanisme observé est
**présent**, le résultat **absent** : c'est la forme que D10 avait nommée après
avoir produit un rouge **vacueux**. La seconde de bascule porte `plc=10`, soit
**les 200 ms à la trame près**. Non-régression : dans la fenêtre d'un ton,
**61 lignes toutes à `plc_plafonnees=0`** — le plafond ne mord jamais tant que la
parole coule. **Six mutations, six tuées.**

### ③ Les critères, avec leur nombre d'exécutions

**SIX exécutions d'agent distinctes** — quatre vertes, **deux rouges de nature
différente** (celui de la **variable** `MICRO_MESURE`, et celui du **binaire**
d'avant E1). **Aucun taux n'est revendiqué nulle part.**

| # | Critère | Verdict | Exéc. |
| --- | --- | --- | --- |
| ① | Deux m-lines audio de **directions opposées** | **TENU** — `Mid(0) Video SendOnly`, `Mid(1) Audio SendOnly`, `Mid(2) Audio RecvOnly` | **5** |
| ② | Le ton traverse, et c'est **LE BON** | **TENU** — écart **0,0 %** sur 440 / 660 / 880 Hz, jugé sur le **PCM décodé** | **3** |
| ③ | Le silence ne coupe pas le flux | **TENU** — et il a exhibé le défaut du §② | **1** |
| ④ | La latence **ajoutée par l'agent** | **TENU**, et **PARTIEL par construction** — `occupation_max_ms` **160 / 180 / 180 / 180** sous un plafond de **200** | **4** |
| ⑥ | La charge ajoutée | **RELEVÉE, sans conclusion** — le micro coûte **≈ 32,4 kb/s**, `packetsLost` = **0** sur les 46 relevés des neuf pilotes | **4** |

⚠️ **Le rouge du BINAIRE discrimine de deux façons** : deux lignes `piste
négociée` au lieu de trois, **et le champ `direction` n'y existe pas du tout**.

⚠️ **L'ÉNONCÉ LITTÉRAL DU CRITÈRE ② N'EST PAS TENU**, et il faut le dire : le
plan exigeait la cible « **à chaque ligne de la fenêtre** », et **1 à 2 lignes
par exécution** sont hors cible **à l'intérieur** de la fenêtre. Ce sont des
**transitoires d'amorçage**, lisibles comme tels sur la ligne même
(`deposees_total=40`, le tampon en train de se remplir). **La conclusion tient ;
la formulation du critère, non** — elle aurait dû exclure l'amorçage.

🔴 **NE PAS LIRE LE CRITÈRE ④ COMME UNE LATENCE.** C'est la borne **dépôt →
retrait**, c'est-à-dire l'occupation du tampon. **La latence de bout en bout
n'est mesurée par AUCUN sous-bloc du chantier D ni du chantier E, depuis D1.**
Et le maximum n'est atteint **qu'une fois par exécution**, en transitoire : le
régime établi est **120 ms**, plat. **La marge est de 20 ms sur 3 exécutions sur
4**, soit 10 % du plafond — **aucune pièce ne dit si c'est confortable**.

⚠️ **Les neuf exécutions se sont jouées SANS RELAIS**, sur candidats `host` :
**exactement un** `WARN allocation TURN impossible` par journal, **neuf au
total**. **Zéro `ERROR` sur les neuf.**

### ④ La correction « A-bis » — le périphérique se désigne, il ne se subit plus

**Le défaut** : l'installation de VB-Cable a fait basculer le **rendu par
défaut** de Windows sur le câble virtuel, **que rien n'alimente**. Le loopback du
chantier A, qui suivait ce défaut, s'est mis à **capter du silence sans qu'aucune
ligne de journal ne dise pourquoi**.

**Le remède n'est PAS « remettre les haut-parleurs par défaut »** : cela
corrigerait l'occurrence en laissant la classe de panne entière, et n'importe
quelle installation audio future la rejouerait. **Le remède est le choix
explicite.**

🔵 **Cartographie établie AVANT de corriger, et elle a réduit le périmètre à un
seul chemin** : `LoopbackCapture::open` (`wasapi.rs`), donc le mode
**mono-fenêtre** et la sonde `AUDIO_PROBE`. **`pour_processus` (multi-fenêtres,
D7+) n'a JAMAIS résolu d'endpoint** — `ActivateAudioInterfaceAsync(VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK)`
vise un **arbre de processus**, jamais un périphérique. **Le multi-fenêtres était
structurellement à l'abri, et il reste hors de portée du remède.**

Livré : **`AUDIO_PERIPHERIQUE`** (voir le tableau des variables), règle **PURE**
dans `agent/src/wasapi/peripherique.rs`, moitié COM dans `agent/src/wasapi/rendu.rs`.
🔵 **Une sous-chaîne AMBIGUË refuse de trancher au lieu de prendre le premier** —
prendre le premier serait retomber sur un **rang d'énumération par la porte de
derrière**, c'est-à-dire le défaut des index DXGI payé en D1 et corrigé en D2.
**La leçon est appliquée d'avance, pas après coup.** Tout repli est
**journalisé** ; aucun n'est silencieux.

**Sept phases sur la VM, une exécution chacune.** Sans la variable
`echantillons=0` ; avec, **440,0 Hz** ; sélection par identifiant, **880 Hz**.
🔵 **Et un témoin décisif** : sans la variable, une tonalité jouée **sur le
câble** est bien captée — **660,0 Hz**. **Le silence du rouge est donc un autre
endpoint, pas une capture morte.** Sans ce témoin, les deux seraient
indiscernables.

🔵 **Instrument neuf : `agent/src/spectre.rs`** — **pur, racine nue, filtre de
Goertzel, aucune dépendance neuve**. `AUDIO_PROBE` ne rendait qu'une **crête**,
qui distingue « du son » de « rien » mais **jamais « MON son » d'un autre**.
Elle rend désormais la **fréquence dominante**, et **rend sa résolution avec son
résultat**.

### ⑤ VB-Cable — installé, et ses formats sont ASYMÉTRIQUES

**VB-Cable n'était PAS installé au 19 août 2026** (relevé avant : zéro
périphérique VB-Audio, et **zéro endpoint de capture local** sur cette VM). Il
l'a été **par le propriétaire du dépôt**, avec ajout du certificat aux magasins
`TrustedPublisher` et `Root`.

⚠️ **La SILENCIOSITÉ de l'installation reste INCONNUE** — elle n'a pas été
tentée par le chantier. **E2 ne peut pas supposer une installation non
interactive sur une machine neuve** (risque R2, ouvert).

🔴 **Les formats sont asymétriques, et c'est le fait le plus important pour E2**
(`IAudioClient::GetMixFormat`, mode partagé) :

| Endpoint | Sens | Format |
| --- | --- | --- |
| **CABLE Input** | **rendu** — c'est là qu'E2 écrira | **48000 Hz**, 2 canaux, 32 bits flottant |
| **CABLE Output** | **capture** — c'est ce que l'application lira | 🔴 **44100 Hz**, 2 canaux, 32 bits flottant |

**Notre chemin est à 48 kHz** — le risque R3 n'est donc pas éliminatoire. **Mais
VB-Cable rééchantillonne 48000 → 44100 en interne, hors de notre code et hors de
toute mesure.** ⚠️ **Le remède est ÉCRIT et NON APPLIQUÉ** : l'écriture au
registre et le redémarrage d'`Audiosrv` ont été **refusés par le bac à sable**.

✅ **Sonde 3 répondue favorablement, et sans aucune API non documentée** : **CABLE
Output est DÉJÀ le microphone par défaut** aux **trois** rôles (`eConsole`,
`eMultimedia`, `eCommunications`). `IPolicyConfig` n'a pas été nécessaire.
⚠️ **Relevé en session 0 (WinRM), pas dans la session interactive** où tourneront
les applications : concordance **plausible, non mesurée**.

### ⑥ Trois décisions qu'un successeur ne devinera pas

**`set_reordering_size_audio(2)`** (`agent/src/transport/initialisation.rs`).
str0m retient jusqu'à `reordering_size_audio` segments **sur un trou**, et ce
réglage vaut **15 par défaut** : à 20 ms par paquet — la durée de trame de
Chrome —, cela fait **jusqu'à 300 ms de rétention**, qui (1) crèvent le budget de
100 ms que le micro s'accorde en tout, et (2) **annulent le FEC in-band**, dont
toute la mécanique est de reconstruire une trame perdue **à partir de la
suivante**. ⚠️ **Sans effet sur l'existant** : c'est un réglage de **réception**,
et avant E l'agent ne recevait **aucun** média. **Coût assumé** : une rafale de
trois pertes consécutives est délivrée comme un trou plutôt qu'attendue — 300 ms
de silence attendu seraient pires que 40 ms de dissimulation.

🔴 **Le défaut LATENT des deux m-lines audio, qui EXISTAIT AVANT ce chantier.**
`Event::MediaAdded` ne discriminait **que sur `kind`** : avec deux m-lines audio,
la seconde **écrasait `audio_mid`**, et le son descendant du chantier A serait
parti sur une piste `recvonly` de notre côté — **c'est-à-dire nulle part, muet et
sans un `WARN`**. Le remède tient à la **direction**, que `MediaAdded` porte déjà
et que str0m **inverse** à l'acceptation d'une offre (la direction vue par
l'agent est **la sienne**) — vérifié **par la mesure**, la sonde 1 voyant le
récepteur annoncer `RecvOnly` sur une piste offerte en `SendOnly`.
**`Audio + SendOnly|SendRecv` → `audio_mid` ; `Audio + RecvOnly` → `mic_mid` ;
`Audio + Inactive` → aucun des deux.**

**L'exclusivité par mutex nommé — décidée, et son coût écrit.** La spec §9
matérialisait « il n'y a qu'un câble » par **un drapeau atomique**, ce qui **ne
garde plus rien depuis D1** : N fenêtres sont N **processus**. Retenu : un
**mutex nommé Windows**, acquis paresseusement au premier paquet montant, tenu
pour la vie du processus enfant — Windows l'abandonne à la mort du propriétaire
et le suivant l'obtient avec `WAIT_ABANDONED`, **exactement la sémantique de
libération que la spec demande**. ❌ **Router le micro par le capteur a été refusé
SUR PIÈCE** : la connexion média du tube est **unidirectionnelle par
construction**, et c'est ce qui garantit son absence de concurrence — y ajouter
un flux montant continu rouvrirait le défaut de canal que D4 a mis **deux
recettes** à fermer. ⚠️ **Le coût, écrit plutôt que découvert** : une seconde
fenêtre qui allume son micro obtient un **refus**, et **ce refus n'est PAS dit au
client** — `ReadyMessage.mic` est décidé à l'établissement. **E1 ne pose que la
couture** ; le mutex est E2.

### ⑦ L'AEC est structurellement incomplète en multi-fenêtres — et ce n'est pas réparable ici

La spec §12 (sonde 4) laissait la question « à réexaminer au moment du chantier
D ». **Le moment est venu, et la réponse est défavorable** :

- l'AEC de Chrome n'annule que ce que **son propre onglet** restitue ;
- depuis **D7**, **chaque fenêtre porte le son de sa propre application** ;
- si l'utilisateur porte le micro dans la fenêtre A **sans casque**, le son que
  restitue la fenêtre B sort des mêmes haut-parleurs, revient dans le micro, et
  **l'AEC de A ne le connaît pas**.

**Aucune correction n'est proposée, et ce n'est pas un oubli** : la corriger
demanderait soit de rassembler la restitution de toutes les fenêtres dans
l'onglet qui capte — **ce qui défait D7** —, soit une AEC côté agent (hors
périmètre). **Son exercice est un critère de E2**, avec **deux** fenêtres qui
jouent du son, pas une. Si E2 le confirme, c'est E3.

### ⑧ La revue transverse — NEUF affirmations devenues fausses, plus une dans le plan

Barème : D7 **5**, D8 **3**, D9 **6**, D10 **douze**, D11 **sept**, P1 **huit**,
P2 **dix**, S1 **cinq**, **E1 neuf**. Toutes franchissent une frontière de tâche.

🔴 **Une seule porte un risque d'ACTION, pas seulement de lecture** : le plan
**RÉSERVAIT** `agent/src/wasapi/rendu.rs` au bloc E2 (rendre le micro sur CABLE
Input), et **la correction A-bis a pris ce nom entre-temps**, pour la
**résolution** du point de terminaison que le loopback doit *capter*. **E2 doit
choisir un autre nom** — les trois places du plan portent la marque.

⚠️ **DEUX des neuf sont des ASYMÉTRIES INTERNES**, et c'est la forme la plus
discrète du défaut : l'en-tête d'un module **avait bien été corrigé**, et la doc
de la fonction qu'il décrit — ou du fichier voisin qui aiguille vers lui — ne
l'avait pas été. **Corriger un en-tête ne corrige pas ce qu'il chapeaute**, et
c'est le cas où quelqu'un **a vu le problème** et l'a traité à un seul endroit.

**Et une dixième, dans le plan** : le critère ③ prescrivait « **et `remplir`
continue de rendre du silence** ». **Réfuté par sa propre recette** — il rendait
un bourdon. Le plan porte son encadré.

✅ **Cinq pistes nommées d'avance étaient DÉJÀ traitées** par les tâches
antérieures. **C'est une information utile** : la discipline par tâche a
fonctionné là où elle pouvait fonctionner.

### ⑨ Pièges neufs — à connaître avant de toucher à ce terrain

- 🔴 **`cargo clean --release -p agent` NE SUFFIT PAS** — voir la parade corrigée
  en section D4. Il faut **`-p proto -p agent`**.
- 🔴 **Un rendu audio lancé depuis WinRM (session 0) n'atteint AUCUN endpoint de
  la session 1**, et le symptôme est `echantillons=0` — **indiscernable d'une
  capture morte**. **Jouer le son par tâche planifiée `/it`**, comme l'agent. Le
  témoin de méthode est versé, avec son journal de tonalité **tronqué** comme
  pièce.
- 🔴 **`waveOutOpen`, `waveOutPrepareHeader` et `waveOutWrite` peuvent rendre `0`
  tous les trois et NE RIEN JOUER** : une `WAVEHDR` passée par `[ref]` en
  PowerShell est une **copie marshalée** dont l'adresse meurt au retour.
  **Trouvé par le crête-mètre `IAudioMeterInformation`, jamais par un code de
  retour** — la doctrine « juger sur la relecture, jamais sur le code de retour »
  (D8), appliquée à l'audio.
- ⚠️ **Un critère de CONTINUITÉ ne dit rien du CONTENU de ce qui continue.** Le
  critère ③ demandait « aucune ligne ne manque » et l'a obtenu, pendant que le
  puits fabriquait un bourdon. **Lire les valeurs, pas seulement les comptes.**
- ⚠️ **Un critère qui exige une propriété « à chaque ligne de la fenêtre » doit
  EXCLURE l'amorçage**, sans quoi son énoncé littéral est faux alors que sa
  conclusion tient.
- ⚠️ **Un instrument de fréquence doit rendre sa RÉSOLUTION avec son résultat.**
  `spectre.rs` le fait ; la trace `micro mesuré` ne le fait pas, et son pas n'est
  qu'**inféré**.
- ⚠️ **Un binaire témoin doit s'identifier lui-même** : le rouge du plafond de
  dissimulation ne se distingue du vert **que par le nom de son fichier** —
  l'attribution n'est pas rejouable sur pièces.
- ⚠️ **Quatre mutations ont SURVÉCU au premier jet** sur les sept campagnes de
  tâche, et **deux ont mis au jour un défaut RÉEL** : une mutation **prescrite
  par le plan** qui ne pouvait pas échouer (elle changeait une grandeur qui était
  à la fois l'entrée et l'attente), et trois tests qui exerçaient **un jumeau du
  chemin de production**. **Un plan n'immunise pas contre le contrôle vacueux —
  il en est une source.**

### ⑩ Ce que E1 n'établit PAS

- **Aucun taux, nulle part** : 3 exécutions au mieux par critère, **1 par bras**
  pour le plafond, **1 par phase** A-bis, **1 par sonde**.
- **Aucune application Windows n'entend rien** : c'est E2.
- **Aucun microphone réel n'est exercé.** L'instrument est un `OscillatorNode` —
  `getUserMedia`, la permission, le choix du périphérique, l'AEC, la suppression
  de bruit et **le DTX d'un vrai locuteur** ne sont éprouvés que par leurs tests
  d'injection.
- 🔴 **La latence de bout en bout n'est mesurée par RIEN**, ni ici ni par aucun
  sous-bloc du chantier D depuis D1.
- **Cinq constantes non calibrées** — `CIBLE`, `PLAFOND`, `SEUIL_SAUT`,
  `SEUIL_INSERTION`, `PLAFOND_DISSIMULATION` —, qui rejoignent `BPP_MIN`,
  `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`, `HYSTERESIS`, `TAILLE_MAX_SORTIE`,
  `REPIT_REARMEMENT_AUDIO` et `REARMEMENTS_MAX`. **Aucun jugement d'écoute n'a
  jamais été porté sur aucune constante de ce dépôt.**
- **La dérive d'horloge n'est pas observée sur une durée longue** : testée sur
  des seuils, jamais sur dix minutes de conversation. C'est E2.
- **L'exclusivité inter-processus n'est pas exercée**, et **le refus n'est pas
  dit au client**.
- **Le rééchantillonnage 48000 → 44100 de VB-Cable** est hors de notre code et
  **hors de toute mesure**.
- **Le périphérique par défaut de la SESSION INTERACTIVE n'a jamais été relevé.**
- **Une seule fenêtre** : aucun multi-fenêtres exercé sur le chemin du micro.
- **Les trois couches inconnues du chantier D le restent.**

**Ce que les pièces laissent inexpliqué, et qu'il faut nommer** : un `WARN`
`paquet micro dont la durée Opus est illisible` aux **quatre** exécutions vertes
et à aucune autre, **une fois par exécution**, cause et volume inconnus
(écart navigateur → agent de 30, 31 et 13 paquets) ; un `framesDropped: 172`
isolé ; et une entrée `erreursPage: ["Uncaught"]` **tronquée à ce seul mot**.

### ⑪ Vérifications de fin de branche

`cargo test -p agent` → **534 passed; 0 failed**.
`cargo check --target x86_64-pc-windows-gnu` → **sortie 0, 11 avertissements**.

🔴 **`scripts/verify-all.sh` REND 1, et l'étape qui échoue n'est pas celle de ce
chantier.** Les **sept** premières étapes sont vertes — `cargo test --workspace`
(**534 + 52**), `cargo clippy --workspace`, `client : npm test` (**179**),
`client : npm run typecheck`, `client : npm run design:verifier` (les six
contrôles du socle S1), `proto : npm test` (**57**), `proto : npm run typecheck`.
La **huitième**, `plateforme : npm run test:sqlite`, tombe sur **un seul test**
— `plateforme/src/agents/canal.test.ts`, **fichier NON SUIVI PAR GIT**, dont le
nom porte le marqueur **🔴** de la discipline rouge-d'abord : c'est le travail
**en cours** du sous-bloc **P3**, qu'un agent concurrent écrivait dans le même
arbre. Le script s'arrêtant au premier échec, **les étapes 9 et 10 n'ont pas été
jouées par lui**.

✅ **Établi plutôt que supposé** : les deux suites `plateforme` passent
**intégralement** dès qu'on exclut ce seul fichier, **sans rien modifier** —
**186 passed** en `sqlite` comme en `postgres`. **Aucune régression du chantier E
sur `plateforme`.** ⚠️ **L'étape 10 (`tsc --noEmit` sur `plateforme`) n'a été
jouée sous AUCUNE forme**, et n'est donc **pas** déclarée verte.

⚠️ **Ce qu'il faut retenir pour la prochaine clôture** : `verify-all.sh` est un
filet **de dépôt**, pas de chantier. Quand deux chantiers partagent l'arbre, son
verdict global ne dit plus rien du travail qu'on clôt — **il faut le lire étape
par étape, et nommer celle qui appartient à l'autre**. Le présenter comme vert
aurait été faux ; le présenter comme rouge l'aurait été tout autant.

🔴 **« TOUS `dead_code` » N'EST PLUS VRAI, et c'est une propriété que ce dépôt
affirmait à chaque clôture depuis D9.** Sur les 11 : **10 `dead_code`**, et
**1 `unused_variables`** — `agent/src/micro.rs:184`, une liaison `let Some(tete)
= … else` jamais lue (le code ne s'en sert que comme test de vacuité). **Aucune
conséquence de comportement**, remède d'un caractère (`_tete`). **Non corrigé
par périmètre** — la tâche de clôture ne modifie le code que pour redresser une
affirmation fausse — et **légué plutôt que dissimulé**.

### ⑫ Ce que le chantier E lègue

**E2, entier** : le rendu sur « CABLE Input » (⚠️ **le nom `wasapi/rendu.rs` est
PRIS**), `windows_micro.rs`, le **mutex nommé**, la recette d'écoute (dix minutes
sans dérive, appel réel sans écho), le **format asymétrique** dont le remède est
écrit et non appliqué, la **silenciosité inconnue** de l'installation, et la
**licence VB-Audio personnelle seulement** (à régler avant mise sur le marché).

**E3, conditionnel** : l'**AEC structurellement incomplète en multi-fenêtres**,
dont l'exercice est un critère de E2 — **avec deux fenêtres qui jouent du son**.

**Propres à E1** :

1. ⛔ **La latence de bout en bout n'est mesurée par rien**, et ne l'a jamais été.
2. ⛔ **Cinq constantes non calibrées**, aucun jugement d'écoute.
3. ⛔ **Le `WARN` « durée Opus illisible »** : cause et volume inconnus.
4. ⛔ **`agent/src/micro.rs:184`** — l'avertissement `unused_variables` qui rompt
   la propriété « tous `dead_code` ».
5. ⛔ **La trace `micro mesuré` ne rend pas sa résolution en fréquence.**
6. ⛔ **Un binaire témoin doit s'identifier lui-même.**
7. ⛔ **Le périphérique par défaut de la session INTERACTIVE n'a jamais été
   relevé** — tous les relevés WinRM sont ceux de la session 0.

---

## 🗂️ Sous-projet ③ Pont fichiers — sous-blocs F0 et F1 : un lecteur en lecture seule (20 août 2026)

Résultats complets :
`docs/superpowers/plans/2026-08-19-pont-fichiers-f1-resultats.md`.
Plan : `docs/superpowers/plans/2026-08-19-pont-fichiers-f1.md`.
Conception : `docs/superpowers/specs/2026-08-19-pont-fichiers-design.md`.
Journaux : `docs/superpowers/plans/journaux-pont-fichiers/` — **DEUX familles de
lecture seulement**, et une seule demande un `sed` :

| Famille | État | Ce qu'il faut faire |
| --- | --- | --- |
| `agent-*-plat.log`, tous les `.txt`, tous les `.json` | UTF-8, **ANSI déjà retirées** | rien |
| `agent-*.log` (bruts) | UTF-8, **séquences ANSI PRÉSENTES** | `sed 's/\x1b\[[0-9;]*m//g'` — ou lire le `-plat` jumeau, versé pour chacun |

⚠️ **Toujours `grep -a`** : un journal à queue d'octets NUL est classé
« binaire » et `grep` rend alors une sortie **vide**, indiscernable d'un compte
nul (piège de D10).

⚠️ **COLLISION DE NOM, à connaître avant de lire ce fichier au `grep`** : « F1 »
désigne ici le **sous-bloc** du pont fichiers, mais **`**F1**` désigne aussi, à
la ligne 4867 et dans la section D7, le premier défaut de la revue transverse de
D7** (le détecteur de panne muette). Les deux n'ont aucun rapport.

F0/F1 remplacent, pour la première fois, le pont FUSE historique
(`src/file.js`) : un processus `PONT=1` tient une racine **ProjFS** sur la VM et
sert chaque rappel par une requête au **navigateur**, via la File System Access
API, sur une `RTCPeerConnection` **dédiée** et **données seules**.

### ⛔ Le fait le plus réutilisable : ProjFS s'active par le CATALOGUE, pas par les RÔLES

```powershell
Enable-WindowsOptionalFeature -Online -FeatureName Client-ProjFS -NoRestart -All
```

- Sur cette VM (**Windows Server 2022**, build 20348), l'activation a rendu
  **`RestartNeeded : False`** : aucun redémarrage nécessaire, aucun n'a eu lieu.
  ⚠️ **Ne pas lire le `RestartRequired : Possible` du catalogue comme une
  prédiction** : c'est ce que le catalogue annonce **avant** activation, pas ce
  que l'activation exige. **Deux champs différents, deux questions différentes.**
- ⚠️ **`Install-WindowsFeature Projected-File-System` ne doit PAS être employé.**
  Le nom vit au catalogue des **fonctionnalités facultatives**
  (`Get-WindowsOptionalFeature`) et **pas** au gestionnaire de rôles :
  `Get-WindowsFeature | Select -Expand Name` filtré sur `Proj` ne rend **aucune
  correspondance**. ⚠️ **Que la commande « échoue en silence » est une
  INFÉRENCE de ce relevé, pas une mesure : elle n'a jamais été lancée.**
- ⚠️ **Le contrôle qui manque à la spec, et qui coûte une ligne** : le
  mini-filtre doit être **RÉELLEMENT CHARGÉ**, pas seulement présent sur le
  disque — `fltmc filters` doit porter `PrjFlt` (altitude **189800**) **et**
  `Get-Service PrjFlt` rendre `Running`. Un filtre présent et non chargé ferait
  échouer `PrjStartVirtualizing` très loin de là, avec un `HRESULT` que personne
  ne rattacherait à ProjFS. C'est le risque **R1bis**.
- **État courant, relevé par la commande** : `State : Enabled`,
  `ProjectedFSLib.dll : True`, `PrjFlt.sys : True`, filtre chargé, service
  `Automatic`. **Le rouge d'avant est versé** (`f0-avant.txt` : `Disabled`, les
  deux `False`) — c'est lui, et lui seul, qui rend le critère falsifiable.

### 🔴 `cd client && npx vitest run` ne couvre PAS `proto/ts/` — DEUX commandes, pas une

La racine Vitest est `client/`. **Aucun document du dépôt ne le disait avant
F1**, et sans cela `proto/ts/fichiers-entetes.test.ts` n'aurait jamais tourné :
le vecteur partagé `proto/fichiers-vectors.json`, dont tout l'intérêt est
qu'un renommage n'ait **qu'un seul côté à casser** pour être vu rouge, n'aurait
épinglé qu'une implémentation sur deux.

```bash
cd client && npx vitest run                # client/src/ seul
cd client && npx vitest run --dir ../proto # proto/ts/
```

### La variable `PONT`, et les QUATRE modes d'`agent.exe`

⚠️ **La phrase « `agent.exe` a trois sortes de processus » est désormais FAUSSE
partout où elle figure.** Il en a **quatre** : superviseur, capteur, **pont**,
enfant.

| Variable | Convention | Où elle est lue |
| --- | --- | --- |
| `PONT=0` | **`=0` DÉSARME ; une simple présence n'active pas** — même convention que `SUPERVISEUR`, `CAPTEUR`, `AUDIO` et `PLEIN_ECRAN`, et pour la même raison : tester `is_ok()` ferait qu'écrire `PONT=0` pour **couper** le pont l'allumerait | `agent/src/main.rs`, branche placée **après** `CAPTEUR` et **avant** le superviseur |

- **Transmise par `scripts/run-agent.sh`** — tâche **dédiée**, jouée **avant**
  que quiconque en ait besoin. *Piège payé en D1 (`SUPERVISEUR`), en D2
  (`MULTIFENETRE_REPRISE`), évité en D3 et en D6 : toute variable neuve doit y
  être ajoutée explicitement, sinon l'agent démarre sans elle et sans rien
  signaler.*
- **La symétrie d'environnement va dans les TROIS sens** : le pont reçoit un
  `env_remove("CAPTEUR")` (sans quoi il serait un second capteur), les enfants
  un `env_remove("PONT")` (sans quoi ils ne captureraient rien), et le
  superviseur un `env_remove("SUPERVISEUR")` pour ses enfants. **C'est le Step 4
  de la recette qui le rend visible**, et c'est le seul endroit où l'oubli d'un
  `env_remove` se voit.
- **Traces** : `pont fichiers lancé` (`superviseur::lanceur::pont`, `INFO`) et
  `pont fichiers lancé ou relancé` (`superviseur::boucle::surveillance_pont`,
  `WARN`, **silencieux ensuite tant que le cycle se répète**).

### 🔵 La liaison ProjFS est RÉSOLUE À L'EXÉCUTION, et le contrôle qui le prouve PASSE

**Décision D1** : `LoadLibraryW` + `GetProcAddress` sur les **treize** entrées,
jamais un import statique — sans quoi `agent.exe`, **un seul binaire pour les
quatre modes**, ne se chargerait plus du tout sur une machine sans ProjFS, et
cela tuerait **la capture et la vidéo**, pas seulement le pont.

**Contrôle en conditions réelles, sur la VM** : `ProjectedFSLib.dll` renommée,
une exécution complète. **Relevé** — `capteur lancé` **1**, session vidéo
établie, `framesDecoded` **15 122** monotone, `packetsLost` **0**,
`chargement de ProjectedFSLib.dll` + `0x8007007E` **366** fois, **367** relances
du pont, et **`0 ERROR` sur 3 064 lignes** : *la panne du pont est un `warn!`,
jamais un `error!`*. **D1 est validée par la mesure, pas par le raisonnement.**

⚠️ **`ERROR_MOD_NOT_FOUND` N'EST ÉMISE NULLE PART** — la chaîne ne vit que dans
la spec et le plan. **Grepper `ProjectedFSLib`.** Le `grep` prescrit rendait `0`,
ce qui se serait lu comme un contrôle échoué sur une exécution parfaite.

### La discipline de fil des rappels ProjFS — pourquoi une panique y est un abandon de processus

Les rappels sont appelés **par le système**, sur des fils que nous ne possédons
pas, à travers une frontière FFI. **Une panique Rust qui traverserait cette
frontière est un comportement indéfini** : d'où un `catch_unwind` **à chaque**
frontière. Et c'est la décision **D2** — le pont est un **processus séparé** —
qui rend le pire cas acceptable : *le pont meurt, le superviseur le relance, la
vidéo ne bronche pas.* **Mesuré** : pont tué par PID relevé,
`delai_apres_mort_ms=44`, 4 processus `agent` avant et **4 après** avec un PID
neuf.

**Ce que cela achète, et c'est la pièce la plus forte du chantier** : sur une
exécution, **le pont est resté calé neuf minutes** pendant que la session vidéo
de la même VM continuait — `framesDecoded` monotone, `packetsLost` **0**, **0**
`clôture de session amorcée`, **0** `ERROR`.

⚠️ **« Vidéo intacte » ne s'étend PAS à l'application.** Pendant ces neuf
minutes l'Explorateur était figé : *les images arrivent, le contenu ne bouge
plus.* **`framesDecoded` ne dit rien de cela.**

### Ce que F1 livre, et les quatre critères

| # | Critère | Verdict | Exéc. |
| --- | --- | --- | --- |
| 1 | l'arborescence, **aux deux niveaux** | **TENU** — 6 entrées sur 6, ensembles de noms identiques à `find` côté hôte, nom accentué avec espace compris | **3** |
| 2 | même **condensat SHA-256** pour le fichier > 10 Mio | **NON ÉTABLI** — aucune copie menée à terme. ⚠️ **PAS RÉFUTÉ** : la même lecture a rendu **12 582 912 octets en 1 937 ms** sur l'état rouge, et le condensat OPFS côté page a été relevé **égal** à celui de l'hôte | **0** |
| 3 | aucun dépassement de budget | **NON DÉMONTRABLE, contrôle VACUEUX** | 5 |
| 4 | la vidéo ne perd pas une image | **TENU** | **6** |

**Aucun taux.** Cinq exécutions du chemin nominal — dont **trois** exploitables —,
une du Step 3, deux du rouge (i).

### 🔴 Les trois défauts que seule la recette pouvait trouver

1. **LA CASSE REND LE MAUVAIS FICHIER, EN SILENCE — pire que le legs annoncé.**
   Le legs promettait `Introuvable` ; **trois exécutions sur trois** montrent que
   `casse.txt` **et** `CASSE.TXT` rendent le **contenu** de `Casse.txt`, sans
   erreur. **Et le comportement n'est pas cohérent avec lui-même** : `GROS.BIN`
   rend bien « introuvable » dans la même exécution. ⚠️ **Le mécanisme est une
   HYPOTHÈSE** : l'écart suit l'**hydratation** (`racine hydratee … octets=42
   entrees=1` — `Casse.txt` seul vivait localement, et NTFS le retrouve **sans
   jamais atteindre le pont**). **Observé, non corrigé.**
2. **LE REFUS D'ÉCRITURE NE REFUSE PAS — mais c'est la SPEC qui promettait
   trop.** Une création locale **RÉUSSIT** (2 exécutions versées sur 2). Ce n'est
   **pas** une divergence du code : `agent/src/pont/notifications.rs`
   documentait déjà que `NEW_FILE_CREATED` est une notification **POST, donc
   irrefusable**, et seuls les trois chemins `PRE_` sont refusés. La formulation
   juste : *écrire dans un fichier PROJETÉ rend `ERROR_WRITE_PROTECT` ; un
   fichier créé de toutes pièces vit sur la VM et n'est jamais poussé.*
   ⚠️ **Et `PRE_CONVERT_TO_FULL` — l'écriture d'un fichier EXISTANT — n'a JAMAIS
   été exercé.**
3. **ÉNUMÉRATION VIDE PAR INTERMITTENCE**, sur racine neuve, sans erreur ni
   trace. ⚠️ **Une occurrence porte un CONFONDEUR** : deux exécutions se sont
   recouvertes de 2 min 23 s — voir le piège ci-dessous — et **le journal
   d'agent versé sous le nom de la première est en réalité celui de la
   seconde**. Cause ouverte.

### 🔴 Quatre contrôles de recette étaient incapables de rendre leur verdict

**Trois cherchaient des chaînes que le produit n'émet pas** (`pont lancé` — la
trace est `pont fichiers lancé`, et **deux** lignes la portent, pas une ;
`ERROR_MOD_NOT_FOUND` ; `ERROR_SEM_TIMEOUT\|delai depasse`, dont le témoin réel
est `commande expirée`). **Deux d'entre eux auraient fait lire un succès comme
un échec.**

**Le quatrième, l'`objdump` qui prouve l'absence d'import statique, portait DEUX
défauts** : le chemin publié était faux (le dépôt est un espace de travail
cargo, la cible vit à la **racine**) et le `|| echo "AUCUN import"` **traduisait
un fichier absent en VERT** ; et sa recette de rouge était insuffisante — **une
`pub fn` sans appelant ne rend pas ce contrôle rouge**, `agent` étant un binaire,
l'éditeur de liens élimine l'inatteignable et `raw-dylib` n'émet alors aucun
import. **Mesuré : le rouge n'est pas apparu.** Il l'est devenu une fois l'appel
placé sur un chemin atteignable depuis `main`.

### La revue transverse — ONZE affirmations, toutes franchissant une frontière de tâche

Barème du dépôt : D7 5, D8 3, D9 6, D10 douze, D11 sept, P1 huit, P2 dix, S1
cinq, E neuf, P3 douze, S2 douze. **Onze ici.** Les plus instructives :

- **cinq commentaires disaient l'état de la TÂCHE 13, que la TÂCHE 14 de la même
  branche a réfuté** — `agent/src/pont.rs` (« la racine est montée et VIDE,
  aucune requête ne part vers le navigateur ») et **quatre** rappels de
  `pont/projfs/rappels.rs` documentés comme rendant `S_OK` vide ou
  `ERROR_FILE_NOT_FOUND`, alors que **la ligne suivante** appelle
  `etat.demander(…)` et rend `EN_COURS`. *Ce sont les quatre points d'entrée du
  pont, et leur documentation disait littéralement l'inverse de leur corps ;*
- **`agent/src/pont/transport.rs` décrivait AU PRÉSENT** le défaut d'aiguillage
  qu'une autre tâche de la même branche venait de corriger ;
- **`agent/src/transport.rs` disait porter « la boucle qui l'anime »** alors que
  la même branche avait extrait `Session::run` vers `transport/boucle.rs` ;
- **`agent/src/pont/notifications.rs` citait en en-tête un absolu que son propre
  corps réfutait cinquante lignes plus bas** (le refus d'écriture) — c'est le
  seul cas où *le module avait raison contre la spec qu'il citait* ;
- **la spec comptait « trois modes » et renvoyait à `main.rs:303-327`**, plage
  qui ne désigne plus l'aiguillage mais un `#[cfg(test)] mod tests`.

**Et le sous-projet a lui-même trouvé deux nombres faux dans son plan** :
`evenements.rs` y était annoncé à **391** lignes (il en faisait **279** à la
rédaction, **363** aujourd'hui — *le 391 n'a jamais été vrai*), et une ligne de
tableau déclarait douze vérifications « **toutes exactes** » alors qu'au moins
une ne l'était pas. *Une ligne qui affirme la complétude d'une vérification est
une affirmation de complétude comme une autre.*

### Les chiffres, RELEVÉS PAR LA COMMANDE après la dernière édition

| Vérification | Référence d'entrée | **Relevé** |
| --- | --- | --- |
| `cargo test -p agent` | 467 | **614 passed, 0 failed** |
| `cargo check --target x86_64-pc-windows-gnu` | 9 avertissements | **16**, tous famille `dead_code` |
| `cd client && npx vitest run` | 107 | **223 passed** (24 fichiers) |
| `cd client && npx vitest run --dir ../proto` | 35 | **111 passed** (5 fichiers) |

**Les 16 avertissements se décomposent, et la décomposition est vérifiée** : 11
préexistants, **2** venus du chantier Microphone (`micro.rs:196`,
`micro/dissimulation.rs:146` — F0/F1 n'a touché ni l'un ni l'autre), **5**
imputables à F1 et tous délibérés (quatre dans `pont/erreurs.rs`, les variantes
réservées à F2–F3 ; un dans `pont/table.rs`).

⚠️ **L'arbre est PARTAGÉ avec un agent concurrent** (sous-projet ⑤). Un premier
passage de `npx vitest run` a rendu 4 échecs, disparus au passage suivant sans
qu'aucune de mes éditions ne les concerne.

**Tailles, relevées par la commande de ce fichier après la dernière édition** —
**aucun fichier de code source ne dépasse 500 lignes** hors les deux entrées de
dette gelée, inchangées (`encode.rs` **1536**, `windows_source.rs` **630**) :

| Fichier | Lignes | Remarque |
| --- | --- | --- |
| `agent/src/encode/arret.rs` | **500** | marge 0, inchangé |
| `client/verify-webrtc.mjs` | **494** | ⚠️ voir l'encadré ci-dessous |
| `agent/src/capture.rs` | 492 | |
| `agent/src/demarrage.rs` | 491 | |
| `agent/src/pont/projfs/rappels.rs` | **488** (marge **12**) | neuf — le plus serré du sous-projet. Il était à **489** : les corrections de la revue transverse lui ont **rendu** une ligne |
| `agent/src/pont/transport/tests.rs` | **474** | neuf |
| `agent/src/transport.rs` | **448** | 495 → 501 (plafond FRANCHI) → **440** par **extraction** de `transport/boucle.rs` (93), puis 448 par les corrections de la revue transverse. **CINQUIÈME fois que ce dépôt paie « la marge regagnée par une extraction se reperd à la ronde suivante »** — D10 l'avait déjà porté à 501 et en avait sorti `initialisation.rs` |
| `agent/src/transport/evenements.rs` | **363** | |
| `agent/src/pont/projfs/chargement.rs` | **338** | neuf — les treize transcriptions |
| `agent/src/pont/service.rs` | **325** | neuf |
| `agent/src/pont/projfs.rs` | **323** | neuf |
| `client/src/design/primitives.test.ts` | **283** | inchangé par F1 |
| `agent/src/pont/transport.rs` | **290** | neuf |
| `agent/src/pont/projfs/etat.rs` | **267** | neuf |
| `agent/src/pont/table.rs` | **174** | neuf |
| `agent/src/superviseur/boucle/surveillance_pont.rs` | **172** | neuf |
| `agent/src/pont/erreurs.rs` | **163** | neuf |
| `proto/src/fichiers.rs` | **157** | neuf |
| `agent/src/pont/projfs/racine.rs` | **153** | neuf |
| `agent/src/pont.rs` | **151** | neuf |
| `agent/src/pont/resolution.rs` | **146** | neuf |
| `agent/src/pont/chemins.rs` | **143** | neuf — **pur**, testé sur l'hôte |
| `agent/src/pont/enumeration.rs` | **140** | neuf |
| `agent/src/pont/notifications.rs` | **134** | neuf — **pur** |
| `agent/src/superviseur/lanceur/pont.rs` | **117** | neuf |
| `proto/src/fichiers/entetes.rs` | **109** | neuf — les sept en-têtes, épinglées par `proto/fichiers-vectors.json` que **les deux** implémentations lisent |
| `agent/src/transport/boucle.rs` | **93** | neuf — l'extraction ci-dessus |
| `agent/src/pont/decoupe.rs` | **62** | neuf — **pur** |
| `agent/src/pont/entetes.rs` | **61** | ne garde que `filetime_depuis_ms` : les formes sont parties dans `proto/` |

> ⚠️ **`client/verify-webrtc.mjs` vaut 494, et `CLAUDE.md` le publie à 497 en
> QUATRE endroits** — l. **598**, **666**, **669** et **750**, énumérés par
> `grep -n '497' CLAUDE.md` **avant** d'écrire cette ligne. **Ils ne sont pas
> réécrits, et c'est délibéré** : ce sont des énoncés **datés** (relevés D10 et
> D11) qui étaient vrais à leur date. Le changement vient du **sous-projet ⑤**
> (commit `69f3442`, revue transverse de P2), **pas de F1**. La marge n'est donc
> plus 3 mais **6**, et c'est ce chiffre-ci qui fait foi.

### Ce que F0 et F1 n'établissent PAS

- **Aucun taux.** Cinq exécutions nominales dont **trois** exploitables, une du
  Step 3, deux du rouge (i).
- **Le condensat SHA-256 de bout en bout** — le critère que la spec désigne comme
  *le seul qui ne puisse pas être satisfait par accident*. Ni établi, ni réfuté.
- **La latence n'est mesurée par rien** (objet de F4, risque R2 : *le lecteur
  peut fonctionner et rester inutilisable*). La seule mesure de débit disponible
  est incohérente d'un **facteur ~120** — 6,5 Mio/s contre 52–55 Kio/s —, **sans
  explication**.
- **Des lectures CALENT sans jamais expirer** : `commande expirée` reste à **0**
  pendant qu'un `Get-ChildItem` ne rend pas la main en 540 s. **On ne sait pas où
  le blocage se produit**, faute d'une trace à l'inscription en table.
- **Aucune constante n'est calibrée** : `TAILLE_TRAME_MAX`, `DELAI_ATTRIBUTS`,
  `DELAI_LIRE`, `DELAI_LISTER` — elles rejoignent `BPP_MIN`, `FACTEUR_FOCUS`,
  `PART_DORMANTE_BPS`, `HYSTERESIS`, `REPIT_APRES_ECHEC`, `TAILLE_MAX_SORTIE`,
  `REPIT_REARMEMENT_AUDIO` et `REARMEMENTS_MAX`.
- **Aucun test d'hôte ne couvre `pont/projfs.rs`** ; `cargo check --target
  x86_64-pc-windows-gnu` en vérifie types, emprunts et durées de vie — **jamais
  le comportement**.
- **R7 reste OUVERT** : les treize transcriptions ont tenu sur les **sept**
  journaux qui portent `racine du pont fichiers montée`, mais **cinq entrées
  n'ont aucun jumeau `PRJ_*_CB`**, et un mauvais `transmute` de fonction est un
  défaut que rien n'attrape avant l'exécution. ⚠️ **Les ~367 relances du Step 3
  ne comptent pas** : la DLL n'y chargeait pas, **aucune transcription n'a jamais
  été appelée**.
- 🔴 **`showDirectoryPicker()` N'A JAMAIS ÉTÉ APPELÉ**, et c'est MESURÉ :
  **aucune commande CDP n'existe pour ACCEPTER un sélecteur de fichiers**
  (`Page.handleFileChooser` et `Page.fileChooserAccepted` rendent `-32601`,
  `Page.setInterceptFileChooserDialog` **intercepte = annule**), et l'hôte n'a ni
  `DISPLAY`, ni `Xvfb`, ni `xdotool`. **La parade est OPFS**, dont
  `navigator.storage.getDirectory()` rend une **vraie**
  `FileSystemDirectoryHandle` : tout le code produit tourne inchangé derrière,
  le point d'injection étant `globalThis.showDirectoryPicker` lu **à l'appel**.
  **Non couverts** : l'appel lui-même, le modèle de permission
  (`queryPermission`/`requestPermission`), et l'activation utilisateur
  transitoire.
- **Deux des trois rouges** (fermer la page-shell pendant une copie ; démarrer le
  pont avant le choix) **n'ont pas été provoqués**. ⚠️ **Et le troisième décrit
  un état inatteignable par ce chemin** : le pont ne monte sa racine qu'**après**
  avoir accepté l'offre SDP de la page-shell.
- **Aucun relais TURN pour le pont** — divergence assumée d'avec la session
  vidéo, à rouvrir le jour où la page-shell et la VM ne se voient pas directement.
- **Rien d'un client réel** : Chrome sans interface, décodage logiciel, sur
  l'hôte qui porte la VM. La File System Access API n'existe ni sur Firefox ni
  sur Safari — limite du **produit**.
- **Rien de plusieurs utilisateurs** : une VM, une racine, `SESSION_DU_PONT` non
  namespacé. **Rien à travers une reconnexion WebRTC.**
- **Rien de l'ancien pont** : ni modifié, ni retiré, ni comparé chiffre à chiffre.
- **Le chemin d'extinction propre du superviseur** n'a toujours jamais été
  exercé, depuis D1. ⚠️ **Et une racine ProjFS peut survivre à un arrêt brutal**
  — le `Drop` ne court pas sur un `TerminateProcess` —, exactement comme les
  sorties virtuelles de D5, et rien dans F1 ne la démonte.

### Pièges neufs — à connaître avant de toucher à ce terrain

- ⚠️ **DEUX EXÉCUTIONS DE RECETTE NE DOIVENT JAMAIS SE CHEVAUCHER, et le
  symptôme n'est pas celui qu'on croit.** Deux se sont recouvertes de 2 min 23 s :
  la seconde a tué l'agent de la première **en pleine mesure**, la première a
  fait échouer la seconde au signaling (« premier message invalide »), et **le
  journal versé sous le nom de la première est celui de la seconde** — même
  horodatage de début à la microseconde près. On lit alors une panne de produit
  là où il y a une collision de protocole. *C'est le piège de D8 rejoué à
  l'envers : là-bas on relisait le journal PÉRIMÉ, ici celui de la SUIVANTE.*
- ⚠️ **Une heuristique de PID est un piège.** « Le pont est le plus jeune des
  `agent` » est **faux** : l'ordre est superviseur, capteur, pont, **puis** les
  enfants. Le rouge de mise à mort a d'abord tué un **enfant** et rendu un relevé
  qui se lisait comme un succès. **Relever le PID dans le journal d'agent.**
- ⚠️ **Deux messages d'interface qui partagent une sous-chaîne font un instrument
  faux.** « Lecteur … **mont**é » et « n'a pas pu être **mont**é » : le pilote
  testait `includes('mont')` et a lancé une mesure de neuf minutes sur un pont
  **non monté**. Corrigé côté **pilote** ; **le produit garde l'ambiguïté**, et
  le prochain instrument tombera dedans.
- ⚠️ **Un `||` de repli transforme « fichier absent » en « contrôle vert ».**
- ⚠️ **Une `pub fn` sans appelant ne rend pas rouge un contrôle d'import** :
  l'éditeur de liens élimine l'inatteignable, et `raw-dylib` n'émet alors rien.
- ⚠️ **Le nom d'un `grep` de recette se vérifie contre le CODE, jamais contre la
  spec.** Trois des quatre contrôles de F1 cherchaient des chaînes inexistantes.
- ⚠️ **Une sonde peut ne pas borner ce que son nom annonce** : la longueur passée
  à `FileStream.Read` **n'est pas** celle que ProjFS demande au rappel
  `GetFileData` — ProjFS choisit sa propre granularité et peut hydrater bien
  au-delà de la tranche demandée.
- ⚠️ **Nettoyer `agent` seul ne suffit pas quand le chantier touche `proto`** :
  `cargo clean --release -p proto -p agent` avant `scripts/build-agent.sh`, et
  **vérifier la taille du binaire** — une compilation de 0,13 s est un aveu.

### Ce que F1 lègue

**Défauts observés, NON corrigés :**

1. 🔴 **La casse rend le mauvais fichier en silence**, de façon **incohérente
   avec elle-même**. ⚠️ **Le legs a changé de nature : ce n'est plus “on ne
   trouve pas”, c'est “on rend autre chose”.** Remède : une table de
   correspondance alimentée par l'énumération (**F3**).
2. 🔴 **Une création locale réussit.** Le produit la journalise ; il ne peut pas
   l'empêcher — notification POST. **F2.**
3. 🔴 **Énumération vide par intermittence**, cause inconnue, journal d'agent
   perdu pour l'occurrence exploitable.
4. 🔴 **Des lectures calent sans jamais expirer** — le legs le plus proche de
   rendre le lecteur inutilisable. **F4.**
5. **Le débit varie d'un facteur ~120** entre deux exécutions, sans explication.

**Mesures dues :**

6. **Le condensat de bout en bout** (critère 2) — premier geste de toute recette
   suivante.
7. **Les rouges (ii) et (iii)**, et **(iii) doit d'abord être RÉÉCRIT**.
8. **`PRE_CONVERT_TO_FULL` n'a jamais été exercé** : la moitié vraie de la
   promesse de lecture seule n'a aucun témoin.
9. **R7** : cinq entrées sans jumeau `PRJ_*_CB`.
10. **Une racine ProjFS survit à un arrêt brutal**, et rien ne la démonte.

**Choix de conception assumés, à rouvrir le jour venu :**

11. **Aucun cache d'énumération en F1** — délibérément écarté : `Rafraichir`,
    seul moyen de l'invalider, est un livrable de **F5**, et un cache que rien ne
    vide reproduirait le défaut de l'ancien pont (`src/file.js`, cache **sans
    TTL**).
12. **Un seul morceau en vol à la fois** : le contrôle de flux par
    `bufferedAmount`/`SEUIL_TAMPON` relève de **F3**. *Déclaré, pas implémenté à
    moitié.*
13. **Cinq verbes ne sont pas livrés.** `proto/src/fichiers.rs` ne définit que
    `TYPE_LISTER`, `TYPE_ATTRIBUTS` et `TYPE_LIRE` ; `Ecrire`, `Creer`,
    `Renommer`, `Supprimer` et `Tronquer` **n'existent nulle part dans le code**,
    non plus que `Rafraichir`, qui va dans l'autre sens. `FICHIERS_VERSION` vaut
    **1** précisément pour que leur arrivée soit une rupture visible.

---

## 📦 Sous-projet ④ Gestion d'apps — sous-bloc G1 : le catalogue naît, et on peut lancer ce qu'il contient (20 août 2026)

Résultats complets :
`docs/superpowers/plans/2026-08-19-gestion-apps-g1-resultats.md`.
Plan : `docs/superpowers/plans/2026-08-19-gestion-apps-g1.md`.
Conception : `docs/superpowers/specs/2026-08-19-gestion-apps-design.md`.
Journaux : `docs/superpowers/plans/journaux-gestion-apps/` — **dix fichiers
suivis par git, UTF-8, séquences ANSI DÉJÀ RETIRÉES** : ils se `grep`ent à plat,
sans `sed`, comme ceux de D4, D5 et D10. **Une seule famille de lecture**, la
plus simple depuis le début du dépôt.

**Binaire mesuré** : `agent.exe` **10 035 712** octets, rebâti après
`cargo clean --release -p proto -p agent` ; le binaire d'avant G1 pesait
**9 914 368** octets. **Plateforme** lancée au commit `0bb1d87`.

### Le fait n°1 : un catalogue de 154 applications, et un lancement qui ouvre la bonne fenêtre

L'agent lit les quatre racines de raccourcis par `SHGetKnownFolderPath`, résout
chaque `.lnk` par **`IShellLinkW`**, filtre, déduplique par un triplet
(cible, arguments, répertoire), et pousse un **diff** à la plateforme sur le
canal `/agent`. `GET /applications?vm=…` le rend ;
`POST /application/:id/lancer` fait lancer, **par le `.lnk` lui-même**.

**Les six critères sont TENUS**, avec leur nombre d'exécutions :

| # | Critère | Verdict | Exéc. |
| --- | --- | --- | --- |
| ① | 154 applications pour 218 raccourcis | **TENU** | **2** |
| ② | Un raccourci neuf apparaît sans redémarrer l'agent | **TENU** | **2** |
| ③ | Un raccourci retiré quitte le catalogue **sans que sa ligne disparaisse** | **TENU**, par le renvoi complet, **pas** par le chemin incrémental | **2** |
| ④ | Les raccourcis sans cible sont exclus **ET** journalisés | **TENU** — **7** | **2** |
| ⑤ | Un lancement ouvre la bonne fenêtre, **par le raccourci** | **TENU** sur ses deux assertions | **2** + 5 tentatives HTTP |
| ⑥ | Le lancement honore le répertoire de travail | **TENU** | **3** lancements |

⚠️ **Aucun taux n'est revendiqué nulle part.**

### ✅ Le corpus de la spec est CONFIRMÉ par `IShellLinkW` — la divergence E16 est fermée

Les **218 / 167 / 154** de la spec avaient été mesurés par `WScript.Shell` ;
`agent/src/apps/lecture.rs` est `#[cfg(windows)]` et **n'avait jamais tourné**.
Les deux voies s'accordent **à l'unité** sur les quatre chiffres observables :

| Grandeur | Compte |
| --- | --- |
| raccourcis lus | **218** |
| écartés `cible-vide` | **7** |
| écartés `extension` | 41 |
| écartés `cible-absente` | 3 |
| **retenus** (218 − 51) | **167** |
| **clés distinctes** | **154** |

**Aucun écart à écrire.** ⚠️ **Portée exacte** : ce sont les **agrégats** qui
sont confirmés. Les champs **par entrée** du corpus versé
(`agent/testdata/gapps-corpus-vm.json`) restent ceux de `WScript.Shell` — un
désaccord sur un raccourci **individuel** qui se compenserait dans les totaux ne
serait pas vu. L'agent n'a **aucun mode de vidage de corpus**, et en écrire un
n'est pas une tâche de recette.

⚠️ **Le champ `retenus` de la trace de réconciliation ne compte PAS les 167** :
il vaut `lancables.len()`, une table indexée par **clé**, donc toujours égal à
`cles`. Les 167 se dérivent de `218 − écartés` et **ne sont émis nulle part**.

### 🔴 Le défaut n°1, trouvé PAR LA MESURE : le pont s'évince avec son père

**94** `agent enrôlé` et **93** fermetures `reason: "remplace"` en **64 s**,
pour **zéro** `enfant lancé`.

**Attribution, sur pièces** : trois `agent.exe` relevés par `Win32_Process`, le
superviseur et ses deux enfants ; `pont fichiers lancé pid=…` à `04:54:37.305`,
**première** fermeture `remplace` **93 ms plus tard**. Le capteur est hors de
cause (il retourne avant l'enrôlement, divergence E3). Le **pont**, lui, est
placé **après** l'enrôlement délibérément — `main.rs` l'écrit — et
`lancer_pont` retire `SUPERVISEUR`, `CAPTEUR`, `TEST_FILE`, `WINDOW_TITLE`,
**mais pas `AGENT_VM` ni `AGENT_SECRET`** (`grep -rn env_remove agent/src/` ne
rend aucune occurrence de ces deux noms).

Le pont s'enrôle donc sous le **même `vm_id`** que son père, et la décision D8
de G1 — « le dernier enrôlement gagne, l'ancien socket est fermé » — les fait
s'évincer mutuellement **sans terme**, à ~1,5 Hz. Le pont meurt en outre sur
« aucune offre SDP » faute de page-shell, et `surveillance_pont` le relance
toutes les 500 ms : c'est ce qui entretient le cycle.

**Ce que cela coûte, mesuré** : le catalogue complet est renvoyé à chaque cycle ;
les messages montants **incrémentaux se perdent** (c'est la réserve de ③) ; les
réponses `Lancee` se perdent (c'est le `504` de ⑤) ; et **deux boucles de
découverte tournent au lieu d'une**, `apps::brancher` étant appelée avant
l'aiguillage `PONT`.

**Défaut de FRONTIÈRE entre trois chantiers** — l'enrôlement de P3, le pont de
F1, le registre de G1 — **correct de chaque côté pris séparément**. **NON
CORRIGÉ** : donner au pont sa propre identité, l'empêcher de s'enrôler, ou faire
tolérer au registre plusieurs sockets par VM sont trois décisions différentes.

### 🔴 Le défaut n°2 : un refus de version ne peut pas être LU, et la VM boucle

Un agent **v1** contre une plateforme **v2**, **UNE exécution** : **0** ligne
`la plateforme REFUSE la version`, **10** couples
`message de la plateforme illisible (version divergente ?)` /
`reprise du canal /agent`, jusqu'au palier de 30 s, **sans terme**.

**Cause structurelle** : `verifie_version` (`proto/src/plateforme.rs`) est un
`deserialize_with` posé sur le champ `v` de **tout** message, **le refus
compris**, et la plateforme émet son refus avec **sa** version —
`{"type":"refus","v":2,"motif":"version"}`. Un agent de version N ne peut donc
**jamais lire** le refus d'une plateforme de version M ≠ N : il tombe dans la
branche « illisible », qui est **reprenable**. Le bras `MotifCanal::Version` de
`sur_refus` n'est atteignable que si les deux bouts s'accordent déjà sur `v` —
**c'est-à-dire jamais dans le seul cas pour lequel il existe.**

⚠️ **Le fait de D10 tient — la plateforme REFUSE bien.** C'est sa **conséquence**
qui est fausse, et les deux commentaires qui la promettaient **nommaient
eux-mêmes le mode de panne obtenu** : « sans quoi une incompatibilité de version
se déguiserait en **boucle de reconnexion infinie**, qui est le mode de panne le
plus coûteux à diagnostiquer », et « la VM **se tait sans boucler** ».

✅ **Ce que la mesure confirme sans réserve** : déployer agent et plateforme **au
même commit** est la **seule** parade qui existe aujourd'hui.

### La variable neuve : `APPS`, et son dernier saut est ENFIN vérifié

| Variable | Effet |
| --- | --- |
| `APPS=0` | **Désarme** la découverte d'applications. ⚠️ **`=0` désarme ; une simple présence n'active pas** — même convention que `PLEIN_ECRAN`, `AUDIO`, `SUPERVISEUR`, `CAPTEUR`, `PART_SONDAGE`, et pour la même raison : tester `is_ok()` activerait le mécanisme en écrivant `APPS=0` pour le couper. Lue dans `agent/src/apps.rs`. Trace : `decouverte d'applications DESARMEE (APPS=0)` |

🔴 **Le dernier saut — PowerShell → `agent.exe` — n'avait JAMAIS été vérifié**,
faute de VM au moment des tâches 1 à 12. **Il l'est** : `scripts/run-agent.sh`
écrit bien `$env:APPS = '0'` (ligne 12 du `run-agent.ps1` généré), l'agent
journalise son désarmement, et rend **0** `catalogue reconcilie` et **0**
`raccourci ecarte` sur **63 s**, soit plus de deux périodes. C'est le piège payé
en D1 (`SUPERVISEUR`), D2 (`MULTIFENETRE_REPRISE`) et D7 (`AUDIO`) — évité ici
par une tâche dédiée, **et confirmé par la mesure**.

### Les relevés annexes

| Relevé | Valeur | Exéc. |
| --- | --- | --- |
| Durée d'une réconciliation complète | **84** et **92 ms** à froid ; **2 032 ms** au tout premier tour d'un processus (init COM) ; **49 à 63 ms** à chaud | 2 + 1 |
| Taille d'un `Catalogue` complet **SUR LE FIL** | **56 145 octets** de charge utile TCP montante, dont **160** de handshake HTTP, soit **55 985 octets**. **MESURÉ par `tshark`**, pas calculé | 1 |
| — à opposer à | **~46 Kio CALCULÉS** par la décision D3 | — |
| `.lnk` de **zéro octet** | catalogue **complet**, et une trace le nomme (`motif="cible-vide"`) | 1 |

✅ **La décision D3 est EXERCÉE et TENUE dans son rôle de filet** : aux **deux**
exécutions de ③, c'est le **renvoi complet du réenrôlement** qui a posé
`disparue_a`, le message incrémental s'étant perdu. « Ce renvoi complet rend la
perte d'un message montant sans conséquence » — c'est la première fois que ce
filet est éprouvé sur le chemin réel.

### 🔴 La divergence de sécurité `403` / `404`, VIVANTE dans le produit et NON TRANCHÉE

**Le même service rend aujourd'hui deux réponses différentes selon la route,
pour la même situation.**

| Route | Chantier | Réponse à une VM qu'on n'a pas le droit de voir |
| --- | --- | --- |
| `plateforme/src/http/routes-applications.ts` | **G1** (décision D9) | `403 { refus: 'vm-etrangere' }`, **distinct** de `vm-inconnue` |
| `plateforme/src/http/routes-vm.ts` | **P4** | `404 { refus: 'vm-inconnue' }`, **indistinguable** |

Le `403` de G1 est un **ORACLE D'ÉNUMÉRATION** : un utilisateur apprend par
tâtonnement quelles VMs existent. Le `404` de P4 suit `routes-auth.ts` et
`agents/enrolement.ts`, qui refusent tous deux de distinguer « inconnu » de
« faux ». **Les deux chantiers ne peuvent pas avoir raison en même temps.**

Chacun a écrit la divergence **en tête de son propre fichier** ; **aucun ne l'a
tranchée**. ⚠️ **Unifier est une DÉCISION, pas une correction, et elle
appartient au propriétaire du dépôt.** Elle est signalée plutôt que prise en
douce.

### La revue transverse — huit défauts, seize places

Barème : **5** en D7, **3** en D8, **6** en D9, **douze** en D10, **sept** en
D11, **huit** en P1, **dix** en P2 (sur **vingt-trois** places), **cinq** en S1,
**neuf** dans le chantier E, **douze** en P3, **douze** en S2, **onze** en F1,
**huit** en P4, **huit** en G1 (**seize** places).

**Les six premiers franchissent une frontière de tâche ou de chantier**, et sont
corrects de chaque côté pris séparément : les trois affirmations de
`0003-agents.sql` que ④ a prises au mot (« RESTE VIDE », « **empruntera** le
canal », « PAS un oubli si **aucun code ne l'écrit** ») ; l'en-tête de
`agents/canal.ts` qui annonçait un canal d'identité seule ; son « ni **registre
d'appartenance** », vrai du registre du RELAIS mais le canal en tient désormais
**un autre**, le sien ; l'en-tête d'`agent/src/plateforme.rs` (« porte une
IDENTITÉ ») ; sa doc de `Canal` (« le lâcher arrête le battement de cœur » — il
arrête aussi la **découverte**) ; et **la mesure qui réfute le commentaire du
protocole** (défaut n°2 ci-dessus, **trois** places).

Deux documents **ANNOTÉS plutôt que réécrits**, ce sont des relevés datés : la
spec de ④ (**4** places : §3.3 complété par E7 et E8, §5 critères ④ et ⑤, §6
portée et hub) et le plan de P3 (**2** places : « ses **neuf** étapes » pour
**dix**, et le legs « ④ empruntera le canal », **CLOS**).

**Vérifié PAR LA COMMANDE et non supposé** : aucun des **47** fichiers touchés
par les commits `(g1)` n'est sous `agent/src/capteur/`,
`agent/src/superviseur/`, `src/`, `web/` ni `client/` — E18 et D12 tiennent, et
le bras catch-all `Ok(autre)` de `capteur/pont_media.rs` est intact.

### Ce que G1 n'établit PAS

- **Aucun taux, nulle part.** Deux exécutions par critère au mieux, **une seule**
  pour plusieurs relevés annexes et pour la rouge de version.
- **Le chemin INCRÉMENTAL de `disparues` n'est pas démontré** : aux deux
  exécutions, c'est le renvoi complet qui a réparé.
- **La recette a été conduite SOUS le défaut n°1**, non corrigé. Toutes les
  mesures de catalogue portent cette condition.
- **Aucun mode mono-processus long n'existe sans navigateur** : le mono-fenêtre
  meurt sur « le signaling s'est fermé avant l'offre », `PONT=1` sur « aucune
  offre SDP ». **`SUPERVISEUR` est le seul mode qui tienne**, et c'est celui qui
  porte le churn.
- **Une seule VM, un seul catalogue** de 218 raccourcis. Rien de la charge, rien
  de plusieurs VMs, rien d'un catalogue plus grand, rien de la latence.
- **Aucune isolation entre utilisateurs** (D9) : `vm.utilisateur_id` est NULL, et
  la ligne `vm non attribuee, acces accorde sans isolation …` a bien été
  observée **à chaque requête**.
- **Aucune icône, aucun téléversement, aucune surveillance, aucune PWA, aucune
  page de hub** — G2 à G5.
- **Aucune constante calibrée** : `PERIODE_RECONCILIATION` (30 s),
  `DELAI_LANCEMENT_MS`, `FILE_EMISSION` (32). Elles rejoignent `BPP_MIN`,
  `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`, `HYSTERESIS`, `REPIT_APRES_ECHEC`,
  `TAILLE_MAX_SORTIE`, `SEUIL_INJOIGNABLE_MS`, `REPLI_MIN_MS`, `REPLI_MAX_MS`,
  `OCTETS_PREFIXE`.
- **Rien d'un antivirus** : l'état de celui de la VM n'a **pas** été relevé.
- **Aucun audit de sécurité** de l'authentification introduite par D9.

### Pièges neufs — à connaître avant de toucher à ce terrain

- 🔴 **`scripts/verify-all.sh` N'EST PAS HERMÉTIQUE, et il échoue précisément
  pour qui suit la procédure du dépôt.** Avec `TURN_URL`/`TURN_SECRET` dans
  l'environnement — c'est-à-dire **après le `set -a && source .env` que tout
  travail sur la VM exige** — **six** tests de `src/signaling/server.test.ts`
  échouent (ils attendent un premier message et reçoivent `ice-config`) ;
  **12 passed** sans eux. **Sensibilité PRÉEXISTANTE**, vérifiée : aucun fichier
  de signaling n'a été touché par G1. Le **même arbre, au même commit**, sort à
  **0** depuis un shell propre et à **1** depuis un shell où `.env` a été sourcé.
  **Lancer `verify-all.sh` depuis un shell propre, ou `env -u TURN_URL -u
  TURN_SECRET`.**
- ⚠️ **`AGENT_VM` attend l'IDENTIFIANT de la VM, pas son nom.** `npm run
  admin:agent -- --vm g1 …` prend un **nom** et affiche un `vm_id=<uuid>` ;
  c'est **cet uuid** que l'agent doit recevoir (`verifierEnrolement` fait
  `lireParVm(p, vmId)` sur `agent_enrole.vm_id`). Posé au nom, l'enrôlement est
  refusé **indistinctement**, et le journal de la plateforme dit
  `enrôlement refusé pour la VM g1` — c'est-à-dire le nom qu'on lui a donné, ce
  qui **ressemble à une VM trouvée**.
- ⚠️ **Le corps de `POST /auth/connexion` attend `motdepasse`, en un mot** — pas
  `motDePasse`. Un `400 {"refus":"forme"}` sans plus de détail est la seule
  indication.
- ⚠️ **Le jeton d'accès expire en quelques minutes** : une recette qui enchaîne
  des `curl` doit le **redemander à chaque appel**, sinon elle lit
  `401 {"refus":"jeton-expire"}` au milieu d'une série et croit à un défaut
  d'autorisation.
- ⚠️ **Un `grep` de contrôle qui compte un symbole compte AUSSI les commentaires
  qui disent qu'on ne l'emploie pas.** Le contrôle « `Resolve` n'est jamais
  appelée » rendait `2` avant mutation et `3` après : **non discriminant**. La
  forme corrigée **blanchit les commentaires de ligne** avant de compter, et rend
  **0** sur l'arbre réel contre **1** dès qu'un appel est injecté.
- ⚠️ **Un `rename_all` est INOBSERVABLE sur un enum dont toutes les variantes
  sont d'un seul mot.** `IssueLancement` (`raccourci`, `cible`, `inconnue`,
  `echec`) : passer `kebab-case` en `snake_case` laisse `cargo test -p proto` à
  **75 passed, 0 failed**. La **même** mutation sur l'enum qui porte
  `BattementRecu` — deux mots — fait **échouer**
  `conformite_aux_vecteurs_partages`. **Lacune inscrite dans le code** ; la
  première variante écrite en deux mots la refermera d'elle-même.
- ⚠️ **`agent.log` ne peut pas être supprimé depuis l'hôte tant qu'un agent le
  tient**, et un `rm` qui échoue laisse lire un journal **périmé** mélangé au
  neuf. Le supprimer **depuis Windows**, après avoir tué l'agent.
- ⚠️ **Le binaire ne peut pas être réécrit tant qu'un agent tourne** :
  `build-agent.sh` échoue en `Accès refusé (os error 5)`. Tuer l'agent **avant**
  toute reconstruction.
- ⚠️ **Un `cd` dans une commande de journalisation fait injecter un `ls` par le
  hook `chpwd` du shell hôte.** `unset -f chpwd` avant tout relevé.
- ⚠️ **La VM s'est hibernée seule en cours de recette** (05:08:41), piège déjà
  documenté depuis D1. Vérifier `virsh list --all` après toute séquence longue.

### Les tailles, RELEVÉES PAR LA COMMANDE APRÈS la dernière édition de la ronde

Relevé le **20 août 2026**, sur l'arbre au commit **`ee56e1e`** — l'arbre est
partagé, et deux autres chantiers y ont commité pendant celui-ci.

**Le tableau de dette est INCHANGÉ, et ses deux lignes portent les mêmes
nombres qu'avant G1** : `agent/src/encode.rs` **1536**,
`agent/src/windows_source.rs` **630**. **Aucun autre fichier de code source ne
dépasse 500 lignes.** G1 n'a fait grossir ni l'un ni l'autre.

**Aucun fichier de G1 n'approche le plafond** — le plus gros est
`agent/src/plateforme.rs` à **429** (marge 71), suivi de
`proto/src/plateforme.rs` **343**, `plateforme/src/agents/canal.ts` **287**,
`agent/src/apps/lecture.rs` **281**,
`plateforme/src/http/routes-applications.ts` **251**,
`agent/src/apps/boucle.rs` **213**, `plateforme/src/depot/application.ts`
**186**, `plateforme/src/agents/registre.ts` **175**,
`agent/src/apps/sha256.rs` **168**, `agent/src/apps/raccourci.rs` **159**,
`agent/src/apps.rs` **137**.

✅ **Deux extractions ont précédé leur addition**, comme la règle l'exige et
sans qu'aucune compression ne soit employée : le module de tests de
`proto/src/plateforme.rs` (tâche 1, **avant** toute addition, le fichier étant à
410 lignes pour 90 de marge), et le harnais de `plateforme/src/agents/canal.test.ts`
(tâche 17, **avant** d'y ajouter une famille de cas).

⚠️ **`plateforme/` EST DÉJÀ au § « Portée » de ce fichier** (ligne 20, relue
avant d'écrire) : la spec de ④ demandait de l'y ajouter, **c'était fait**, et il
n'y avait rien à faire (divergence E1).

### Les legs de G1

1. 🔴 **Le pont hérite de l'identité d'enrôlement de son père.** Trois remèdes
   possibles, aucun tranché. **C'est le legs le plus lourd** : il mord en
   permanence, **en configuration livrée**.
2. 🔴 **Un refus de version ne peut pas être lu par le pair qui en a besoin.**
   Décision de protocole : lire `v` et `type` avant de valider, par exemple.
3. 🔴 **La divergence `403 vm-etrangere` / `404 vm-inconnue`.** Décision du
   propriétaire du dépôt, signalée dans les deux fichiers concernés.
4. ⛔ **Le chemin incrémental de `disparues` n'est pas démontré** : il le sera
   quand le legs n°1 sera fermé.
5. ⛔ **`verify-all.sh` n'est pas hermétique.**
6. ⛔ **Le corpus reste celui de `WScript.Shell` par entrée.** Le fermer demande
   un mode de vidage dans l'agent.
7. ⛔ **Le champ `retenus` de la trace ne compte pas ce que son nom dit.**
8. ⛔ **Deux boucles de découverte tournent** quand le superviseur est actif —
   conséquence du legs n°1.
9. ⛔ **La lacune de nommage d'`IssueLancement`** est inscrite, pas fermée.

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
fichier a été écrit tout du long jusqu'au ~~**6 août 2026** (sous-bloc D9)~~
**19 août 2026** (sous-blocs D10, D11, P1, P2, P3, S1, et le **chantier E**).
Elle ne date que le pied de page hérité du Guacamole historique, ci-dessous,
qu'aucun chantier du projet agent n'a touché.
⚠️ **Le « 6 août 2026 » avait à son tour dormi SEPT sous-blocs**, dans la phrase
même qui dénonçait une date endormie. **Une annotation qui corrige une date
vieillit exactement comme la date qu'elle corrigeait** — la seule défense est de
la reprendre à chaque clôture, comme n'importe quel compte.

**Contributeurs**:

- Développement initial: [Original dev name]
- Documentation et bugfixes: Session Claude (21 Oct 2025)

**Version du projet**: 1.1 (post-bugfixes)

---

> 💡 **Rappel Important**: Toute nouvelle découverte, bug résolu, configuration importante, ou décision architecturale DOIT être ajoutée à ce fichier pour référence future.
