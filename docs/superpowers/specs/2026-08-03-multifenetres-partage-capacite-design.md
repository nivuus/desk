# Sous-bloc D6 — partage de la capacité réseau entre N flux

**Date** : 3 août 2026
**Chantier** : D (multi-fenêtres), sous-bloc 6
**Prédécesseur** : D5 — le vivier d'encodeurs
(`docs/superpowers/specs/2026-08-02-multifenetres-vivier-encodeurs-design.md`)

---

## 1. Renumérotation des sous-blocs à venir

D6 était nommé « partage de la capacité réseau entre N flux, **et audio par
fenêtre** ». Ces deux sujets sont indépendants et n'ont ni les mêmes inconnues
ni le même risque :

- le partage réseau est un travail d'**arbitrage**, largement testable à froid,
  dont la couture existe déjà (le capteur voit toutes les fenêtres) ;
- l'audio par fenêtre est un travail de **mesure d'API Windows** : WASAPI
  loopback capte le mix du bureau, pas un processus. Le rendre par fenêtre
  exigerait le loopback par processus (`ActivateAudioInterfaceAsync` +
  `VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK`), jamais éprouvé sur cette VM, sur un
  chemin (`agent/src/wasapi.rs`, 543 lignes, dette gelée, aucun test) qui ne se
  compile que sur la VM.

Les mettre dans la même branche y mêlerait un résultat prévisible et un
résultat inconnu. Les sous-blocs se renumérotent donc :

- **D6** — partage de la capacité réseau entre N flux *(ce document)* ;
- **D7** — audio par fenêtre ;
- **D8** — plein écran et Keyboard Lock.

---

## 2. Le défaut, et la prémisse sur laquelle il repose

### 2.1. Ce que le code fait aujourd'hui

La dette écrite au chantier C volet 1 — « `Event::EgressBitrateEstimate` est une
estimation **de session**, pas de piste, il faudra une couche de répartition
avant de la remettre à N contrôleurs » — **ne décrit plus l'architecture**.
Depuis D1, chaque fenêtre est un processus portant **sa propre
`PeerConnection`**, donc son propre BWE et son propre `congestion::Controleur`.

Le problème réel est l'inverse de celui qui était prévu : **N contrôleurs
indépendants estiment chacun la capacité entière du même lien et la réclament
chacun.** Deux faits de code l'établissent :

- `agent/src/transport.rs:276` — `rtc.bwe().set_desired_bitrate(plafond_bps)` ;
- `agent/src/superviseur/lanceur.rs` — `BITRATE` est hérité **tel quel** par
  chaque enfant (12 Mb/s par défaut, `agent/src/demarrage/source.rs:51`).

À huit fenêtres, **huit sondages BWE visent donc 96 Mb/s cumulés** sur un lien
unique, et le sondage à la hausse de chacun est lu par les autres comme de la
congestion.

### 2.2. Le symptôme, déjà relevé mais jamais traité comme tel

Seconde recette de D4 (2 août 2026), rang 8 : le capteur produit **494,4 i/s
cumulées**, le navigateur n'en décode que **224,9** au palier de 30 s, et le RTT
monte de **2 ms à 104 ms**.

### 2.3. ⚠️ La prémisse est une INFÉRENCE, et D6 doit commencer par l'éprouver

Le rapport de D4 attribue cet écart au réseau **en le déclarant explicitement
comme une inférence** : « aucune mesure de charge du pont ». D6 est bâti sur
elle. Si le pont interne porte en réalité plusieurs centaines de Mb/s, huit
sondages visant 96 Mb/s ne l'ont pas saturé, l'effondrement de D4 avait une
autre cause (décodage navigateur, CPU de l'hôte), et un répartiteur de débit ne
la corrigerait pas.

**La mesure du §6.3 est donc une porte, pas un contrôle final** : première tâche
du plan, avant toute ligne de code. Le §6.4 dit ce qu'on fait si elle réfute.

---

## 3. Arrangement retenu

### 3.1. Le choix, et les deux écartés

**A — Répartiteur pur dans le capteur. RETENU.** Un module
`agent/src/capteur/repartiteur.rs` sans `cfg`, sans objet COM, sans canal : une
fonction qui prend l'état des fenêtres et rend une part par session,
entièrement testable sur l'hôte. C'est le patron posé par D4 pour
`capteur/protocole.rs` et par D5 pour `capteur/vivier.rs` — *ce qui décide se
teste à froid, parce que c'est la pièce la plus coûteuse à se tromper*. Il vit
dans le même verrou que le vivier, donc sur la même source de vérité
(`capteur/sommeil.rs`), et la part redescend par un `DepuisCapteur::Part` frère
de `DepuisCapteur::Sommeil`.

**B — Répartiteur dans le superviseur. ÉCARTÉ.** Le superviseur ne parle à ses
enfants que par variables d'environnement au lancement — aucun canal vivant — et
il ne voit pas la visibilité, qui n'existe que sur le chemin
client → enfant → capteur. Il faudrait dupliquer le signal de D5.

**C — Pas de répartiteur, le capteur diffuse l'état et chaque enfant calcule sa
part. ÉCARTÉ.** La règle de focus exige de diffuser aussi *qui* est focalisée,
donc le même trafic ; et l'état global se réplique en N exemplaires qui peuvent
diverger. On perdrait la fonction pure unique.

### 3.2. La couture existe déjà, et le protocole la nomme

`VersCapteur::Visibilite` remonte du client, **le capteur arbitre globalement**,
et l'effet redescend par `DepuisCapteur::Sommeil` poussé sur la connexion média.
La documentation de `Visibilite` dans `capteur/protocole.rs` dit déjà, mot pour
mot : « le capteur arbitre globalement, et la décision peut concerner une AUTRE
fenêtre que celle qui a signalé ». Le débit se branche sur cette couture sans en
inventer une.

### 3.3. Le flux

```
client → visibilité/focus ──▶ enfant ──▶ capteur ┐
                                                 ├─ sommeil.rs (verrou unique)
                                                 │   ├─ vivier      → qui dort   (D5)
                                                 │   └─ repartiteur → qui reçoit quoi (D6)
                            enfant ◀── DepuisCapteur::Part { bps } ◀─┘
                              ├─ rtc.bwe().set_desired_bitrate(part)
                              └─ Controleur::changer_plafond(part)
```

Le répartiteur se recalcule **au changement d'état seulement** — arrivée,
départ, endormissement, réveil, changement de focus — jamais périodiquement,
comme `Etat` et `Sommeil`.

### 3.4. Côté enfant : deux applications, dont la plus importante n'est pas la plus visible

1. `Controleur::changer_plafond(part)` — `config.plafond_bps` est **déjà**
   exactement la borne haute de la décision
   (`controleur.rs:86` : `estimation × MARGE − audio_bps`, borné par
   `plafond_bps`). L'échelle de barreaux ne dépend que de la taille source et du
   `fps` (`Echelle::depuis`), jamais du plafond : `changer_plafond` sera donc un
   frère de `changer_source` qui **ne reconstruit rien**, il mute une borne.
2. `rtc.bwe().set_desired_bitrate(part)` — **c'est celui qui compte le plus**.
   C'est lui qui, figé à `BITRATE`, fait viser 12 Mb/s à huit sondages
   simultanés. Le seul point 1 bornerait ce que l'encodeur produit sans rien
   changer à ce que le sous-système BWE injecte dans le lien pour sonder.

---

## 4. La règle de part

### 4.1. La fonction

Entrées : le budget `B`, la liste des sessions avec leur état (éveillée /
endormie), et **au plus une** session focalisée (le vivier la connaît déjà).
Sortie : une part en bits par seconde, par session.

1. Chaque **endormie** reçoit `PART_DORMANTE`, un plancher, **jamais zéro** :
   elle n'encode plus rien (D5 a relâché son encodeur) mais sa `PeerConnection`
   vit, et `set_desired_bitrate(0)` n'est pas un réglage que str0m est censé
   recevoir.
2. Le reste, `R = B − D × PART_DORMANTE`, se divise entre les `E` éveillées :

   ```
   part_base   = R / (E − 1 + FACTEUR_FOCUS)
   focalisée   = FACTEUR_FOCUS × part_base
   les autres  = part_base
   ```

   Sans focalisée éveillée, parts égales : `R / E`.

### 4.2. Deux constantes neuves, toutes deux NON CALIBRÉES

- **`FACTEUR_FOCUS = 2,0`.** Le raisonnement qui la fonde, et qui **n'est pas
  une mesure** : deux barreaux voisins de l'échelle sont dans un rapport de
  pixels de `1,25² ≈ 1,56` (`DIVISEURS` de `congestion/echelle.rs`). Un facteur
  2 garantit donc plus d'un barreau d'écart en faveur de la fenêtre regardée.
  **C'est le critère ② qui la jugera**, pas cette intuition.
- **`PART_DORMANTE`, de l'ordre de 256 kb/s** (l'audio vaut 128 kb/s,
  `opus::BITRATE_BPS`). ⚠️ **Point à vérifier à l'implémentation, pas à
  affirmer ici** : on ignore si str0m émet réellement du bourrage de sondage
  vers `set_desired_bitrate` quand aucun média ne part. Si oui, ce plancher est
  ce qui empêche des fenêtres endormies de manger le lien pour rien ; si non, il
  ne coûte que sa ligne.

### 4.3. Ce que la règle ne fait volontairement PAS

**Aucun plancher par fenêtre éveillée.** Avec `vivier::PLAFOND_EVEIL = 8`, la
part la plus basse vaut `B / (7 + FACTEUR_FOCUS)`. Si elle tombe sous le débit
minimal du barreau plancher, le contrôleur y est **déjà** et annonce
`Qualite::Insuffisante` au navigateur — mécanisme acquis au chantier C volet 1.
Dire deux fois la même chose introduirait un second seuil à calibrer.

Ordre de grandeur pour situer, **calculé et non mesuré** : à `B = 12 Mb/s` et
huit éveillées, la part de base vaut 1,33 Mb/s et celle de la focalisée
2,67 Mb/s, quand le barreau plancher d'une source 1280×720 exige
`0,05 × 640 × 360 × 60 ≈ 691 kb/s` et le barreau plein ≈ 2,8 Mb/s. **Huit
fenêtres descendront donc d'un ou deux barreaux** — comportement juste si le
lien vaut vraiment 12 Mb/s, régression visible s'il en vaut cent. C'est
précisément pourquoi le budget doit être calibré (§6.3) et non hérité.

### 4.4. Aucun travail conservateur

Une fenêtre qui réclame moins que sa part **ne rend pas** le surplus aux autres.
La reprise de l'inutilisé introduirait une seconde boucle de rétroaction dont la
stabilité devrait être éprouvée ; hors périmètre. À nommer dans les résultats.

---

## 5. Chemins dégradés

| Situation | Comportement | Pourquoi c'est sûr |
| --- | --- | --- |
| Enfant pas encore attaché | Démarre sur `BITRATE` hérité | La part part **dès l'appariement de la connexion média** (`VersCapteur::Identite`), avant tout média : fenêtre d'exposition ramenée à ~0 |
| Enfant mort, fenêtre fermée | `sommeil::retirer` existe déjà → redistribution | Les survivantes montent, jamais l'inverse |
| Canal rompu sans retrait | L'enfant garde sa **dernière** part | Conservateur par construction : il ne réclame jamais plus qu'avant |
| Capteur tué puis relancé | La part cesse, puis revient au rattachement (538 à 689 ms mesurés en D4) | D4 a prouvé que `Canal::commander` rend une erreur (`ERROR_NO_DATA`) au lieu de se suspendre |

**Le budget total** devient une variable d'environnement lue par le **capteur**
seul (`BUDGET_BPS`). `BITRATE` reste côté enfant comme valeur d'amorçage et de
repli, ce qu'il est déjà. ⚠️ **Rappel du piège payé en D1, D2 et D3** : toute
variable neuve doit être ajoutée **explicitement** à `scripts/run-agent.sh`,
sinon l'agent démarre sans elle et sans rien signaler.

---

## 6. Recette

### 6.1. Les trois critères

| # | Ce qui est mesuré | Le seuil, et d'où il vient |
| --- | --- | --- |
| ① | À 8 fenêtres : écart entre i/s produites au capteur et décodées au navigateur, plus le RTT | Navigateur ≥ 90 % du capteur, RTT médian ≤ 20 ms sur le palier. **Seuils CHOISIS, pas mesurés** — à opposer aux 45 % et 104 ms de D4 |
| ② | Barreau de la focalisée contre celui des autres ; octets RTP d'une endormie | La focalisée tient un barreau **strictement** supérieur ; une endormie reste au plancher. C'est ce qui juge `FACTEUR_FOCUS` et `PART_DORMANTE` |
| ③ | Capacité réelle du chemin, et budget par défaut qui en découle | Un nombre **relevé**, de laboratoire |

### 6.2. Portée que les résultats devront porter

**Une exécution par critère donne un relevé, jamais un taux.** Le nombre
d'exécutions doit apparaître dans chaque énoncé — leçon de tous les sous-blocs
depuis D2.

### 6.3. La mesure ③ vient en premier

- **Méthode** : une seule fenêtre, `BITRATE` porté très haut (100 Mb/s), source
  animée à cadence connue, sur le chemin exact du produit (VM → pont →
  navigateur hôte). On relève vers quoi le BWE converge en régime établi et le
  débit RTP réellement sortant.
- **Témoin qui départage** : au même moment, `framesDecoded` / `framesDropped`
  du navigateur et la charge CPU de l'hôte. C'est ce témoin qui dit si l'écart
  de D4 était du réseau ou du décodeur.
- **Le budget par défaut de `BUDGET_BPS`** se dérive de cette mesure, avec
  marge. ⚠️ C'est un nombre **de laboratoire** : un déploiement réel a un autre
  lien, d'où la variable d'environnement plutôt qu'une constante.

### 6.4. Si la mesure ③ réfute la prémisse

On l'écrit dans les résultats. Le répartiteur reste juste sur le fond — huit
flux ne doivent pas viser chacun le lien entier — mais **le critère ① retombe
sur un fait mesurable côté agent** : *le sondage cumulé ne dépasse plus le
budget*, au lieu d'un chiffre côté navigateur qui ne bougerait pas.

### 6.5. Protocole — les leçons déjà payées, qu'on ne repaiera pas

- **Source animée à cadence connue et affichée par la source elle-même** : sans
  quoi Desktop Duplication n'émet aucune trame et une capture lente se lit comme
  une source lente (`instrument/anim-d4.html`, 90 Hz de rAF mesurés).
- **Un `--user-data-dir` par fenêtre**, sans quoi Chrome rejoint son instance
  existante et l'on compte des lancements au lieu de fenêtres.
- **Visibilité imposée page par page par le pilote de recette.** Un Chrome sans
  interface rapporte `document.hidden = true` pour toute fenêtre d'arrière-plan.
  C'est **structurel ici et non cosmétique** : la règle de part *dépend* du
  focus. `Page.addScriptToEvaluateOnNewDocument` ne court pas sur une page
  ouverte par `window.open` — poser l'override explicitement, page par page.
- **Lire `#status`** (page d'application) et non `#statut` (page-shell).
- **Purger les sorties virtuelles entre exécutions** (`MULTIFENETRE_VDD_PURGE=1`) :
  elles survivent à un `Stop-Process -Force`.
- **Contrôler la survie de la VM après chaque rang** (elle s'hiberne seule) et
  **vérifier `Get-Process agent`** avant chaque exécution.
- **Copier `agent.log` après la fin réelle**, pas à la fin du pilote.
- **Aucune capture d'écran CDP pendant une mesure** ; toute évaluation CDP sur
  une page portant un flux WebRTC actif doit être **bornée**.
- Si netem est posé : **reposer `off` en fin de mesure**.
- Si une mesure dépasse 5 minutes : les trois `--disable-*-throttling` de Chrome.

---

## 7. Tests à froid

`repartiteur.rs` est une fonction pure : elle se couvre entièrement sur l'hôte,
sans VM.

- **L'invariant central** : la somme des parts ne dépasse jamais le budget.
- 0, 1 et 8 sessions ; aucune focalisée ; toutes endormies.
- **Le cas limite que le produit rend atteignable** : une focalisée que le vivier
  a **évincée** — elle est endormie, donc au plancher, et la majoration ne
  revient à personne.
- Aucune part n'est jamais nulle ; la focalisée éveillée reçoit strictement plus
  que ses voisines éveillées.

`Controleur::changer_plafond` : la borne s'applique, l'échelle reste intacte, une
baisse fait descendre un barreau, une remontée repasse par l'hystérésis
existante.

Puis **`cargo check --target x86_64-pc-windows-gnu` avant toute compilation
distante** (acquis de D3), et `scripts/build-agent.sh` avec `.env` **sourcé** —
sans quoi il s'arrête en silence après « sources synchronisées ».

---

## 8. Dette de taille de fichier

Relevé **par la commande**, le 3 août 2026, et non recopié :

```bash
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
```

Trois fichiers dépassent 500 lignes, et ce sont les trois du tableau de dette
gelée : `agent/src/encode.rs` (1536), `agent/src/windows_source.rs` (631),
`agent/src/wasapi.rs` (543). **Aucun autre.**

⚠️ **`agent/src/capteur/serveur.rs` est à 490 lignes, marge 10 — et c'est
exactement le fichier où le câblage du répartiteur voudrait naturellement
atterrir.** Toute addition y appelle une **extraction**, jamais une compression :
la leçon a déjà été payée deux fois par ce dépôt (`capture.rs`, `encode/arret.rs`),
et une troisième fois sur ce fichier même, passé de 433 à 490 en une seule ronde
de correction en D4.

Marges des autres fichiers touchés, **relevées** : `transport/adaptation.rs`
472 (28), `capteur/protocole.rs` 312, `capteur/sommeil.rs` 263,
`capteur/vivier.rs` 261, `congestion/reconfiguration.rs` 142.
`repartiteur.rs` naît neuf, donc sous 500 par obligation.

---

## 9. Ce que D6 laissera ouvert

À écrire dans les résultats, pour qu'on ne le croie pas fait :

- **L'audio par fenêtre** — devenu D7, avec son inconnue d'API Windows.
- **Le plein écran et Keyboard Lock** — devenus D8.
- **Le travail conservateur** (§4.4) : une fenêtre qui n'use pas sa part ne la
  rend pas.
- **La latence de bout en bout** — jamais mesurée par aucun sous-bloc du
  chantier D, et D6 ne la mesure pas davantage.
- **Les trois couches inconnues** : celle du plafond de 8 encodeurs, celle du
  plafond de 4 processus, et le mécanisme de l'abandon du mutex DXGI.
- **`BPP_MIN` et le `fps` de `Config`** — toujours non calibrés, et ils se
  recalibrent ensemble.
- **`HYSTERESIS` et `REPIT_APRES_ECHEC`** du vivier — toujours non calibrées
  depuis D5.
- **Le chemin d'extinction propre du superviseur** — toujours jamais exercé.
- **La mort d'un enfant pendant que les autres diffusent**, et la fermeture d'une
  fenêtre en cours de diffusion — toujours jamais exercées.
