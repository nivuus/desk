# Sous-bloc D7 — l'audio par fenêtre

**Date** : 3 août 2026
**Chantier** : D (multi-fenêtres), sous-bloc 7
**Prédécesseur** : D6 (partage de la capacité réseau) —
`docs/superpowers/specs/2026-08-03-multifenetres-partage-capacite-design.md`
**Successeur annoncé** : D8 (plein écran et Keyboard Lock)

---

## 1. Objet

Aujourd'hui, une seule fenêtre porte du son, et ce son est le **mix de toute la
machine**. D7 livre l'**isolation stricte** : chaque fenêtre porte le son de
**son** application, et de rien d'autre.

L'arbitrage retenu, décidé au cadrage :

- **isolation stricte** ; quand elle échoue pour une fenêtre, le repli est le
  **silence**, jamais le mix global ;
- la **capture vit dans l'enfant**, l'**arbitrage dans le capteur** ;
- entre plusieurs fenêtres d'un **même processus**, c'est la **focalisée** qui
  porte le son ;
- **l'audio survit au sommeil** de D5 : le vivier ne concerne que la vidéo.

## 2. État des lieux, vérifié

| Fait | Pièce |
| --- | --- |
| La capture vise le **rendu par défaut de la session** (`eRender`/`eConsole`), donc le mix de la machine | `agent/src/wasapi.rs:129` (`LoopbackCapture::open`) |
| Le superviseur réserve le son à la **première fenêtre détectée**, et ne le rend jamais | `agent/src/superviseur/table.rs`, champ `audio_libre` ; le code documente lui-même ce défaut |
| La consigne descend par une variable d'environnement booléenne | `agent/src/superviseur/lanceur.rs:234` (`AUDIO`), lue en `agent/src/main.rs:138` |
| L'enfant construit sa source audio, ou continue sans son en cas d'échec | `agent/src/demarrage.rs:109-117` |
| Le **DTX Opus est actif** : une trame de silence retombe à quelques octets | `agent/src/opus.rs:76`, test `le_silence_prolonge_retombe_a_quelques_octets_par_trame` |

> ⚠️ **Ce fait, exact en lui-même, a nourri une inférence NON CONFIRMÉE par la
> recette.** Le cadrage de ce sous-bloc en tirait qu'une fenêtre porteuse dont
> l'application ne joue jamais rien « ne coûtera quasiment rien », en pariant
> que le DTX s'engage aussi sur le flux du *process loopback*. La mesure du §3
> a trouvé un **plancher de 1 LSB** sur ce chemin (ni A ni B silencieux ne
> rendent 0), et **un flux à ±1 LSB n'est pas du silence numérique** — rien
> n'établit que le DTX s'y engage. La recette de la tâche 13 ne l'a pas non
> plus tranché : ses sources jouaient toutes un son continu, et le cas d'une
> fenêtre porteuse dont l'application se tait n'a jamais été monté. **Reste une
> inférence, jamais mesurée** — voir
> `docs/superpowers/plans/2026-08-03-multifenetres-audio-par-fenetre-resultats.md`
> §1.5 et §4.

| Le capteur reçoit le **`hwnd`** de chaque fenêtre | `agent/src/capteur/protocole.rs:38` (`VersCapteur::Attache`) |
| Le capteur tient le **focus** | `agent/src/capteur/sommeil.rs:70` (`focalisee`), alimenté par `signaler` (l. 205) depuis `VersCapteur::Visibilite` (protocole l. 64) |
| Le capteur pousse déjà des ordres par fenêtre | `DepuisCapteur::Sommeil` (protocole l. 85), `DepuisCapteur::Part` (l. 108) |
| Le plafond d'éveil du vivier vaut 8 | `agent/src/capteur/vivier.rs:23` (`PLAFOND_EVEIL`) |

### 2.1 L'inconnue, et ce qui est réellement acquis

`agent/src/wasapi.rs:395` porte déjà `probe_process_loopback`, écrite au
chantier A pour répondre à une question de celui-ci. Le relevé
(`docs/superpowers/plans/2026-07-28-audio-resultats.md` §5) est explicite sur sa
portée :

> **Ce qui est acquis** : Windows accepte d'**activer** une interface
> `AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK` pour un PID donné, sur ce build
> (20348) — reproduit deux fois.
> **Ce qui reste ouvert** : `Initialize`, `GetService`,
> `IAudioCaptureClient::Start` et **le moindre octet capturé**. La sonde rend
> `Ok(...)` dès l'obtention de l'interface, sans jamais l'utiliser (`_client`
> n'est ni initialisé ni lu).

**Tout D7 repose sur la moitié non mesurée.** D'où le §3.

## 3. Tâche 1 — la mesure qui gouverne

Elle est **la première tâche du sous-bloc**, et non un chantier séparé. Elle
pousse la sonde au-delà de l'activation : `Initialize` →
`GetService(IAudioCaptureClient)` → `Start()` → lecture réelle, sur un processus
qui joue effectivement du son.

**Trois relevés, et le second est celui qui compte :**

1. **crête non nulle** sur le processus cible ;
2. **crête nulle pendant qu'un *autre* processus joue.** C'est cette moitié-là
   qui prouve l'**isolation**. La première ne prouve que « des octets
   arrivent » — une capture qui rendrait en réalité le mix global la passerait
   aussi, et le sous-bloc entier serait bâti sur une illusion ;
3. le **format de mixage** rendu par un client de *process loopback* — rien ne
   garantit qu'il soit le 48 kHz / 2 canaux / 32 bits flottant relevé le
   28 juillet sur le mix de session — et la tenue d'un cycle `Stop()` puis
   `Start()`.

**Règles de décision, écrites avant la mesure :**

| Relevé | Conséquence |
| --- | --- |
| ② réfuté (le voisin s'entend) | **D7 est reconçu.** L'isolation stricte n'existe pas par cette API ; le sous-bloc s'arrête et son document de résultats porte la réfutation |
| ③ réfute le cycle `Stop()`/`Start()` | repli sur l'approche **B** (§4.4) : la source reste vivante, l'arbitrage cesse d'écrire dans l'anneau |
| ③ rend un format différent | le chemin de conversion de `wasapi.rs` (`convertir_flottant` / `convertir_entier`) le couvre déjà ; à confirmer, pas à supposer |

**Piège de méthode, hérité du chantier A** : `[Console]::Beep` passe par
`kernel32!Beep` et ne traverse pas le périphérique de rendu — il a produit un
faux négatif documenté. Employer `Media.SoundPlayer` ou toute API multimédia
réelle.

## 4. Architecture

### 4.1 Vue d'ensemble

```
navigateur ──focus──> enfant ──Visibilite──> capteur
                                              │
                                    capteur/audio.rs (pur)
                                    session→pid, focus, historique
                                              │
                        DepuisCapteur::Audio { actif } (canal de commandes)
                                              ▼
                                            enfant
                                              │
                              LoopbackCapture::pour_processus(pid)
                                     Start() / Stop()
                                              ▼
                                    piste audio de SA session
```

**Aucun octet audio ne traverse le tube du capteur.** C'est ce qui distingue
l'audio de la vidéo : le *process loopback* n'a aucune des propriétés qui
avaient forcé la mutualisation en D4 (une seule duplication DXGI par sortie, un
plafond de 8 encodeurs). Il n'y a donc rien à mutualiser, et la couche de
transport qui a coûté une recette entière à D4 n'est pas touchée.

### 4.2 Le PID ne circule pas

Personne ne se le transmet : le **capteur** le dérive de chaque `hwnd` qu'il
reçoit déjà dans `Attache`, l'**enfant** du sien qu'il reçoit déjà dans
`FENETRE_HWND`, tous deux par `GetWindowThreadProcessId`. **Aucun champ de
protocole ajouté pour ça.**

### 4.3 L'arbitrage — `agent/src/capteur/audio.rs`

Module **pur, sans aucun `cfg`, testé sur l'hôte**, comme `vivier.rs` et
`repartiteur.rs` avant lui. Il ne connaît ni COM, ni fenêtre, ni encodeur.

- **Entrée** : la table `session → pid`, le focus courant, l'ordre des focus
  passés.
- **Sortie** : l'ensemble des sessions qui portent le son.

**La règle :**

1. une seule session par PID porte le son ;
2. c'est celle du groupe qui a le focus, si l'une du groupe l'a ;
3. sinon **la dernière du groupe à l'avoir eu** ;
4. à défaut de tout historique, la première attachée ;
5. **le sommeil n'entre pas dans la règle** — l'audio survit au sommeil de D5,
   les deux mécanismes restent orthogonaux.

La règle 3 est une décision de confort assumée : `focalisee` est un
`Option<String>` **global** — au plus une fenêtre focalisée sur toute la
session —, donc un groupe entier peut perdre le focus quand l'utilisateur
clique ailleurs. Sans la règle 3, son son se couperait.

**Où vit la table, et où elle s'accroche.** La table `session → pid` vit dans
l'`Etat` de `agent/src/capteur/sommeil.rs`, aux côtés de `focalisee` et du
vivier — c'est le registre unique du capteur, et en faire un second ailleurs
créerait deux vérités à tenir synchronisées. `sommeil::inscrire`
(`sommeil.rs:179`) prend donc un `pid` en plus de la session ; l'appelant, sur
le fil de fenêtre, le dérive du `hwnd` reçu dans `Attache`. Le **retrait**
passe par `sommeil::oublier` (`sommeil.rs:170`), et par rien d'autre. C'est le défaut M1 que D6 a payé : `sommeil::retirer` vidait
`focalisee`, ni `distribuer` ni le chemin `rompus` ne le faisaient, et un
rattachement réinscrivant le même nom héritait d'un focus jamais réémis.

### 4.4 Le cycle de vie dans l'enfant

**Approche retenue : activer une fois, `Start()`/`Stop()` ensuite.**

L'activation COM — seule étape qui puisse refuser — a lieu **une fois, au
démarrage de l'enfant**, à un instant prévisible où l'échec est lisible dans le
journal. Ensuite `IAudioClient::Stop()` arrête le remplissage du tampon (coût
nul quand la fenêtre est muette) et `Start()` le reprend, sur bascule
d'arbitrage.

Les deux approches écartées, et pourquoi :

- **construire/détruire à chaque bascule** — chaque bascule de focus rejouerait
  une activation COM asynchrone, donc chaque bascule pourrait **échouer en
  pleine session**, sur le chemin qu'on connaît le moins ;
- **source toujours vivante, paquets jetés** — sûre mais gaspilleuse : capture
  et encodage tournent en permanence sur toutes les fenêtres muettes. C'est le
  **repli** si la tâche 1 réfute le cycle `Stop()`/`Start()`.

**L'enfant naît muet** et n'émet que sur ordre du capteur — exactement comme
`SourceDistante` naît `endormie = true` depuis D6. C'est ce qui évite que deux
fenêtres d'un même PID soient toutes deux audibles pendant les millisecondes qui
précèdent le premier arbitrage. **Au rattachement d'un canal, l'enfant redevient
muet** et attend un ordre neuf, même doctrine.

`LoopbackCapture::pour_processus(pid)` naît **à côté** de `open()`, qui reste
tel quel : un agent lancé à la main, sans `FENETRE_HWND`, garde le mix de
session. C'est le mode mono-fenêtre d'aujourd'hui, et il ne doit pas régresser.

### 4.5 Le protocole

Un seul message neuf, sur le canal de commandes qui porte déjà `Sommeil` et
`Part` :

```rust
DepuisCapteur::Audio { actif: bool }
```

**Émis seulement quand la décision change** — même discipline que les parts de
D6, qui n'émet que les parts qui bougent.

## 5. Le budget

`Controleur::new` pose `audio_bps: opus::BITRATE_BPS` **inconditionnellement**
(`agent/src/transport.rs:316`), et `observer` le retranche du budget vidéo
(`agent/src/congestion/controleur.rs:95`). Aujourd'hui, les sept fenêtres sur
huit qui n'ont **aucune** piste audio amputent quand même leur budget vidéo de
128 kb/s — soit ≈ 8,5 % d'une part de 1,5 Mb/s (12 Mb/s ÷ 8), pour une piste qui
n'existe pas. **Défaut préexistant, corrigé ici.**

`audio_bps` suit désormais l'arbitrage : **128 000 quand la session porte le
son, 0 sinon.**

**Effet de bord nommé plutôt que découvert en recette** : gagner le son ampute
le budget vidéo de 128 kb/s **au moment même** où gagner le focus le majore par
`FACTEUR_FOCUS`. Le net reste une hausse, mais **un franchissement de barreau à
la bascule de focus est un effet attendu, pas un défaut** — et la recette doit
le savoir d'avance.

## 6. Erreurs et replis

**Le repli est le silence, et jamais le mix global.** Si l'activation échoue au
démarrage d'un enfant, cette fenêtre est muette et sa session vidéo continue —
c'est déjà ce que fait `demarrage.rs` (un `warn!`, puis on continue). Retomber
sur `LoopbackCapture::open()` ferait entendre à une fenêtre le son de toutes les
autres, sous couvert d'isolation.

**Le mode de défaillance silencieux, et sa recette d'entrée.** Si l'arbitrage se
figeait — aucune session ne portant jamais le son —, le symptôme serait le
silence total : pas un `WARN`, pas une erreur, aucun symptôme observable. Même
piège que D6 a rencontré avec `endormie`, même remède : la trace `compteurs
audio` de `windows_audio.rs` porte désormais `pid=` et `actif=`, et **D8 s'ouvre
par ce `grep`** :

```bash
grep -c 'compteurs audio' agent.log
grep 'compteurs audio' agent.log | grep -c 'actif=true'
```

❌ **CE `grep` TEL QU'ÉCRIT A TROIS DÉFAUTS — ne pas le recopier tel quel.** Les
deux premiers ont été trouvés en l'exécutant (tâche 12, document de résultats
§3) ; le troisième, **de loin le plus grave**, par la revue finale de branche
(F1), et il visait le CODE autant que ce paragraphe :

1. **Il rend 0 sur un `agent.log` brut.** Les séquences ANSI de `tracing`
   séparent le nom du champ de sa valeur (`[3mactif[0m[2m=[0mtrue`), donc
   `actif=true` ne matche jamais littéralement. Il faut
   `sed 's/\x1b\[[0-9;]*m//g'` **avant** le `grep`.
2. **Il rend 0 sur toute session de moins de `REPORT_INTERVAL` (30 s)**, la
   trace `compteurs audio` étant périodique — un zéro **bénin**, indiscernable
   du défaut qu'il existe pour révéler.
3. ❌ **Et sa condition de déclenchement était INSATISFIABLE, parce que le code
   ne pouvait pas produire la ligne qu'elle cherchait.** Dans
   `windows_audio.rs`, le bloc `REPORT_INTERVAL` vivait **après** le
   `if !emettait { sleep; continue; }` de la branche muette : la trace n'était
   atteignable que quand `emettait` valait vrai, donc son champ `actif` valait
   **structurellement `true`**. Le second compte était donc **toujours égal**
   au premier, et « le second vaut zéro alors que le premier ne le vaut pas »
   ne pouvait jamais se produire. Pire : le défaut que ce contrôle existe pour
   révéler — plus aucune fenêtre ne porte le son — rend `0` et `0`, que le
   point 2 ci-dessus classe comme **bénin**. Un opérateur lisait 0/0, concluait
   correctement que rien n'allait mal, dans l'état exact où tout allait mal.
   **Corrigé par la revue finale de branche (F1)** : le bloc a été remonté
   au-dessus du gate, une fenêtre muette rapporte donc elle aussi toutes les
   30 s, avec `actif=false`.

**Forme corrigée** :

```bash
sed 's/\x1b\[[0-9;]*m//g' agent.log > agent-plat.log
grep -c 'compteurs audio' agent-plat.log                      # A
grep 'compteurs audio' agent-plat.log | grep -c 'actif=true'  # B
```

sur une session vivante depuis au moins 30 s. Voir `CLAUDE.md`, section
« Sous-bloc D7 », pour le détail.

**Lecture, sur un binaire portant le correctif F1** — chaque fenêtre vivante
émet une ligne par période, porteuse ou non :

- `A = 0` : **le contrôle n'a rien à dire.** Aucune session n'a vécu 30 s ; ce
  n'est pas un verdict, c'est une mesure non prise.
- `A > 0` et `B = 0` : **le défaut est là.** Des fenêtres vivent, aucune ne
  porte le son.
- `A > 0` et `B > 0` : l'arbitrage désigne bien un porteur. Sur `N` fenêtres
  d'un même PID, attendre `B ≈ A / N`.
- ⚠️ `B == A` avec plusieurs fenêtres d'un même PID vivantes : **suspect** —
  c'était précisément la signature du code d'avant F1.

Un champ qui rend un défaut muet observable **pour rien** est un bénéfice, pas
un coût — mais **encore faut-il que le code puisse l'émettre dans les deux
états.** C'est la leçon de F1, et elle est plus générale que ce contrôle-ci :
*un contrôle dont on n'a pas vérifié qu'il PEUT échouer ne contrôle rien* —
même piège que D6, ici rejoué une seconde fois sur le même paragraphe.

Les erreurs de lecture et d'encodage gardent le comportement d'aujourd'hui :
elles arrêtent l'audio de **cette** fenêtre, journalisent les compteurs
accumulés, et ne touchent ni la vidéo ni les autres sessions.

⚠️ **Ce paragraphe sous-estimait la portée d'une erreur de LECTURE, et la revue
finale de branche l'a corrigé (F3).** « Elles n'affectent que cette fenêtre »
est faux depuis que l'audio est arbitré : le capteur continue de tenir la
session comme **porteuse de son groupe de PID**, donc sa voisine reste muette et
n'est jamais promue — une seule erreur transitoire (changement de périphérique,
redémarrage du service audio, changement de format) silençait **tout le
groupe**, définitivement, pendant que `set_actif` continuait de réussir et que
`ordre audio applique` continuait d'annoncer `actif=true`. Depuis F3 :

- les erreurs de **lecture** sont retentées avec temporisation croissante, et
  l'abandon n'a lieu qu'après `LECTURES_ECHOUEES_MAX` (10) échecs d'affilée
  (`agent/src/audio.rs`, éprouvé sur l'hôte) ;
- un abandon définitif, de lecture **ou** d'encodage, pose un témoin que
  `appliquer_audio` journalise (`capture_morte=true`) : la trace cesse de
  mentir.

⚠️ **Ce qui reste ouvert, et qui est un changement de PROTOCOLE hors périmètre
de D7** : le capteur ne voit pas ce témoin, qui vit dans l'enfant. La promotion
de la fenêtre voisine demande un signal enfant→capteur puis un réarbitrage — à
cadrer dans le sous-bloc suivant.

## 7. Deux dettes soldées dans le mouvement, et une règle qui l'exige

- **`superviseur/table.rs`** perd son champ `audio` et son `audio_libre`, dont
  le code documente lui-même le défaut (« ne redevient jamais vrai une fois une
  porteuse désignée »). Le superviseur cesse de décider du son. `AUDIO=0` reste
  un interrupteur **global** — un agent lancé à la main doit pouvoir couper le
  son — mais cesse d'être une consigne par fenêtre : `lanceur.rs` ne pose plus
  cette variable. `table.rs` est à **489 lignes, marge 11** : cette suppression
  lui en rend.
- **`wasapi.rs` est à 543 lignes, dette gelée** (`#[cfg(windows)]`, aucun test).
  Y ajouter `pour_processus` sans extraire violerait la règle des 500 lignes
  telle qu'elle est écrite dans `CLAUDE.md` (« toute addition substantielle
  s'accompagne d'une extraction »). L'extraction est mûre et évidente : les
  ~180 lignes de machinerie COM du *process loopback* — `EtatActivation`,
  `ResultatActivation`, `GestionnaireCompletion`, `probe_process_loopback` —
  partent dans **`agent/src/wasapi/process_loopback.rs`**, où le code neuf a sa
  place. `wasapi.rs` retombe autour de **360** et **sort de la dette gelée**.

**Une consignation de D6 est absorbée, les trois autres ne le sont pas.** La
n°2 — un span `tracing` porteur de `session` sur le fil de fenêtre du capteur —
sert directement D7 : sans elle, les traces audio du capteur sont anonymes dans
un `agent.log` partagé depuis D4, et la recette devrait imputer à la main comme
D6 a dû le faire **en pleine mesure**. Les n°1 (télémétrie par session et son
extraction vers `windows_source/telemetrie.rs`), n°3 (rejeu du critère ④ de D6
avec un palier de 45 à 60 s) et n°4 (A/B différentiel de `set_desired_bitrate`)
**restent des suites de D6** et attendent.

## 8. Tests

**Sur l'hôte, sans la VM** :

- `capteur/audio.rs`, module pur : une fenêtre seule de son PID porte le son ;
  deux fenêtres d'un même PID n'en portent qu'une ; la focalisée gagne ; quand
  le groupe perd entièrement le focus, la dernière focalisée garde ; l'oubli
  d'une session recompose le groupe et fait passer le son à la suivante ; le
  sommeil ne change rien à la décision.
- `Controleur` : une session muette ne retranche rien de son budget vidéo.
- `cargo check --target x86_64-pc-windows-gnu` **avant toute compilation
  distante** — l'acquis d'outillage de D3. Il couvre types, emprunts,
  visibilités et durées de vie ; **pas** l'édition de liens.

## 9. Recette et critères

### 9.1 L'instrument, et pourquoi il ne peut pas être déclaratif

Prouver « chaque fenêtre entend son application et elle seule » par
`bytesReceived` ne prouve **rien** : du son arrive, on ignore lequel.
L'instrument branche donc un `AnalyserNode` sur la piste reçue de chaque page et
**relève la fréquence dominante**. Deux applications jouant deux tonalités
distinctes rendent le critère mesurable au lieu de le laisser à l'oreille —
même exigence que la tâche 1 s'impose avec sa crête nulle sur le processus
voisin.

### 9.2 Les critères

Chaque énoncé du document de résultats portera **son nombre d'exécutions** :
aucun taux n'est revendiqué qui n'ait été mesuré.

| # | Critère | Relevé attendu |
| --- | --- | --- |
| ① | **Isolation** — deux applications, deux tonalités | page A ≈ 440 Hz et **pas** 880 ; page B l'inverse |
| ② | **Arbitrage par PID** — deux fenêtres d'un même processus | une seule piste audio croît, l'autre stagne ; l'inverse après bascule de focus |
| ③ | **L'audio survit au sommeil** — au-delà de 8 fenêtres | `framesDecoded` figé **et** `bytesReceived` audio en croissance, sur la même session |
| ④ | **Le budget rendu** — mesuré, non érigé en critère | `audio_bps=0` tracé sur les sessions muettes |
| ⑤ | **Aucune régression mono-fenêtre** | agent sans `FENETRE_HWND` : le mix de session est toujours capté |

### 9.3 Les pièges de protocole, repris parce qu'ils ont chacun coûté une mesure

- **Dimensionner les paliers APRÈS lecture des constantes de temporisation du
  code**, jamais avant (D6 : palier de 25 s contre `DELAI_REMONTEE` de 20 s,
  trois échecs imputés au produit alors qu'ils venaient du protocole).
- **La source doit jouer un son à fréquence connue et affichée par elle-même** :
  sans ce chiffre, une capture muette et une source muette se lisent pareil.
- **Aucune capture d'écran CDP pendant une mesure**, et toute évaluation CDP sur
  une page portant un flux WebRTC actif doit être **bornée** (elle peut ne
  jamais rendre).
- **La visibilité et le focus sont imposés page par page** : un Chrome sans
  interface rapporte `document.hidden = true` pour toute fenêtre d'arrière-plan.
  Faire passer la cible par `blur` puis `focus` — la déduplication de
  `client/src/visibilite.ts` peut sinon faire **disparaître** le focus.
- **Attendre le FAIT, jamais une durée.**
- **Purger les sorties virtuelles entre deux exécutions**
  (`MULTIFENETRE_VDD_PURGE=1`) : elles survivent à un `Stop-Process -Force`.
- **Contrôler la survie de la VM après chaque rang** : elle s'hiberne d'elle-même,
  déclencheur non identifié.
- **Toute variable d'environnement neuve est ajoutée à `scripts/run-agent.sh`
  dans sa propre tâche** — piège payé en D1 (`SUPERVISEUR`) et D2
  (`MULTIFENETRE_REPRISE`), évité en D3 et D6.
- **Copier `agent.log` après la fin réelle de l'exécution**, pas à la fin du
  pilote : les enfants meurent après.

## 10. Ce que D7 n'établira PAS

À écrire tel quel dans le document de résultats :

- **le plafond d'activations *process loopback* concurrentes**, et plus encore
  depuis des processus appelants distincts. La recette à N fenêtres l'exercera
  **sans le mesurer** — c'est la forme exacte de l'inférence que D3 a dû payer
  sur DXGI, et elle est nommée ici plutôt que découverte ;
- **la latence de bout en bout**, toujours mesurée par aucun sous-bloc du
  chantier D ;
- **les applications UWP** dont le rendu audio ne vit pas dans l'arbre du
  processus propriétaire de la fenêtre : `INCLUDE_TARGET_PROCESS_TREE` ne les
  couvrirait pas, et aucune n'est au protocole ;
- **la qualité perçue** — aucun jugement d'écoute n'est prévu, pas plus que ne
  l'étaient les jugements visuels que `BPP_MIN` attend depuis le chantier C
  volet 1 ;
- **les trois couches inconnues** le restent : celle du plafond de 8 encodeurs,
  celle du plafond de 4 processus, et le mécanisme de l'abandon du mutex DXGI ;
- **le chemin d'extinction propre du superviseur** n'a toujours jamais été
  exercé, et **la mort d'un enfant pendant que les autres diffusent** pas
  davantage.

## 11. Risques

| Risque | Portée | Parade |
| --- | --- | --- |
| Le *process loopback* ne capte rien, ou capte le mix global | **Éliminatoire** — D7 est reconçu | Tâche 1, relevé ② |
| Le cycle `Stop()`/`Start()` n'est pas supporté | Approche 4.4 invalide | Repli sur B, coût CPU seulement |
| Plafond d'activations concurrentes | Borne le nombre de fenêtres sonores | Non mesuré, nommé au §10 |
| Le format de mixage diffère du mix de session | Conversion | `convertir_flottant`/`convertir_entier` couvrent déjà ; à confirmer en tâche 1 |
| Deux fenêtres d'un même PID, focus jamais posé | Aucune ne porte le son | Règle 4 de l'arbitrage (première attachée) |
