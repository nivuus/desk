# Sous-bloc D9 — solder la dette du chantier D : résultats

**Date** : 6 août 2026
**Branche** : `chantier-multifenetres-d9`, partie de `4692e32` (main)
**Conception** : `docs/superpowers/specs/2026-08-06-multifenetres-solder-la-dette-design.md`
**Plan** : `docs/superpowers/plans/2026-08-06-multifenetres-solder-la-dette.md`
**Journaux** : `docs/superpowers/plans/journaux-multifenetres-d9/` (118 fichiers
suivis par git)

---

## 0. Le verdict, en cinq lignes — et il ne se simplifie dans aucun sens

1. **Le changement de mode de sortie est RETIRÉ, sur mesure.** L'inconnue
   « éliminatoire » de D8 est tranchée dans le sens favorable — le pilote
   **accepte**, duplication ouverte — mais le changement **ne survit pas**
   (`n = 4` exécutions propres) et `CDS_UPDATEREGISTRY` **pollue le registre**
   (confirmé, attribuable par GUID, 3 transitions probantes). **Les legs 12 et
   13 cessent d'exister** au lieu d'être différés.
2. **🔴 La pollution déjà écrite bloque le produit à TROIS fenêtres sur cette
   VM**, aux six exécutions de la recette ③ sans exception. C'est la Critique C1
   de D8 — que le désarmement devait rendre inatteignable — **observée en train
   de mordre**.
3. **🔴 La réélection après répit du remède audio est INERTE.** La branche
   promotion est valide ; le cas majoritaire — une application, une fenêtre —
   **reste sans remède**.
4. **Le leg 10 est décidable, et les journaux le tranchent** : deux sessions sur
   quatre, **canal démontré vivant**, n'émettent aucun `Resize`. ⚠️ Cela
   **écarte** « canal mort » pour elles ; cela ne **désigne** pas le client.
5. **La capture *process loopback* suit l'ARBRE DE PROCESSUS**, jamais le
   service audio ni le périphérique de rendu — ce qui a rendu la recette ②
   non exerçable, et explique d'un coup ses quatre échecs de déclenchement.

**Aucun taux n'est revendiqué nulle part.** Chaque énoncé de ce document porte
son nombre d'exécutions.

---

## 1. Sort de chacun des douze legs — fermé, disparu, ou non exercé

Trois états, jamais d'entre-deux.

| Leg | Origine | État | Pièce / raison |
| --- | --- | --- | --- |
| **1** — signal enfant→capteur quand une capture audio meurt | D7 (F3) | **FERMÉ SUR PIÈCES côté code — NON EXERCÉ sur la VM**, et **partiellement INERTE** | 12 tests d'hôte purs (`capteur/audio.rs`), 2 tests de registre, 2 tests de `tick.rs` ; **zéro `AudioMort` dans un seul journal** ; branche réélection inerte (§4) |
| **2** — identité de session par génération monotone | D7 (F5), préexistant | **FERMÉ SUR PIÈCES sur le registre de SOMMEIL SEUL** | 3 tests d'hôte sur `retirer_est_perime` + `registre.rs` ; ⚠️ **le jumeau de `capteur/serveur.rs` reste ouvert** (§7, transverse n°5) |
| **3** — `TICKS`/`CAPTURED`/`PRODUCED` par session | D6 n°1 | **FERMÉ SUR PIÈCES** | `windows_source/telemetrie.rs` (72 l., pur, 2 tests) ; `windows_source.rs` 638 → **628** ; lecteur déplacé dans le capteur, sous le span `session` |
| **4** — critère ④ de D6 à palier 45–60 s | D6 n°3 | **FERMÉ SUR PIÈCES** | 3 promotions sur 4 déplacements, palier 60 s, **2 exécutions** (§5) |
| **5** — A/B différentiel sur `set_desired_bitrate` | D6 n°4 | **JOUÉ — N'ÉTABLIT RIEN. Reste dû** | +23,2 % entre bras contre **+83,1 % de variance intra-bras**, 4 exécutions (§6) |
| **6** — les trois inconnues du changement de mode | D8 | **SANS OBJET** — la première est **tranchée**, le mécanisme est retiré | §2 |
| **7** — défaut HiDPI côté client | D8 | **CORRIGÉ sur l'unité ; sa CONSÉQUENCE a disparu avec le mécanisme** | §3 ; ⚠️ la raison écrite dans le code était fausse — transverse n°2 |
| **8** — instrumenter la chaîne viewport → `Resize` | D8 | **FERMÉ SUR PIÈCES côté code — le rejeu différé NON EXERCÉ** | `client/src/resize.ts` (pur, 5 tests) ; la course n'a pu être provoquée ni naturellement ni artificiellement (§3) |
| **9** — trois défauts de l'instrument de recette | D8 | **FERMÉ SUR PIÈCES** | seuil `audio_survit` sur la dominante, `verdict_partie_mesurable` cohérent, `window.__pleinEcran` borné par horodatage |
| **10** — pourquoi si peu de `Resize` | D8 | **RENDU DÉCIDABLE ; la réponse contredit le rapport qui l'a mesurée** | §8 |
| **12** — C1, pollution du registre par le produit | D8 | ⛔ **DISPARU comme dette de CODE** — ⚠️ **mais la pollution déjà écrite mord** | §2, §9 |
| **13** — C2, reprise D2 court-circuitée | D8 | ⛔ **DISPARU** — `reconstruire_sur_la_sortie` n'avait qu'un appelant | §2 |

*(Le leg 11 — annoter le §4.1 du cadrage jeux — avait été fait par D8 au commit
`2856f2a`.)*

---

## 2. Phase P — l'éliminatoire est tranché, et il tranche contre l'armement

### 2.1 Le montage, et ce qu'il corrige de D8

La sonde P1 de D8 n'ouvrait **jamais** de `DuplicateOutput` : c'était l'écart
banc/produit béant. La sonde de D9
(`agent/src/diagnostics/multifenetre/mode_sortie.rs` et ses cinq enfants)
ouvre une `DesktopCapture` sur la sortie sous test et **la tient** pendant toute
la tentative, rejoue le même geste **sans duplication** dans la même exécution
(le témoin), éprouve une **quatrième combinaison de drapeaux** (`flags = 0`, la
seule qui n'écrive pas au registre), compte les **pertes d'accès infligées à
deux voisines**, et relit le nom de la sortie.

Elle **exclut structurellement la taille courante** des cibles candidates et rend
`NON MESURABLE` si aucun mode annoncé n'en diffère — l'acquis de la tâche 3bis de
D8, conservé —, et elle **juge sur le mouvement relu par DXGI, jamais sur une
égalité**.

### 2.2 L'éliminatoire : ACCEPTÉ (2 exécutions sur 2)

| Question | Réponse | Exéc. |
| --- | --- | --- |
| Le pilote accepte-t-il un changement de mode sur une sortie **à duplication ouverte** ? | **OUI** — mouvement DXGI réel, duplication tenue pendant l'appel | **2/2** |
| La sortie garde-t-elle son nom `\\.\DISPLAYn` ? | **OUI** | **2/2** |
| Combien de pertes d'accès `0x887a0026` infligées aux voisines ? | **NON MESURÉ** — le compteur a **saturé** à 2, qui est le **plafond de l'instrument** (1 sonde × 2 voisines), pas une mesure. Et leur attribution au changement de mode n'est pas établie : la création de la sortie de la voisine 2 les explique entièrement (doctrine D1/D2) | — |
| Le témoin sans duplication diffère-t-il ? | **Aucun écart** — donc l'attribution à la duplication n'est **pas tranchée** | 2 |

**Fait annexe non planifié, et il compte** : un changement de mode **déplace
silencieusement les coordonnées bureau des autres sorties** (`DISPLAY6` →
`y = -720`, `DISPLAY7` → `x = 6240`, puis retour). Aucun document ne l'annonçait.

### 2.3 La persistance : NE SURVIT PAS (`n = 4` exécutions propres)

C'est ce qui renverse le verdict. **La sortie revient à sa taille de création dès
qu'une sortie virtuelle de plus est créée** — c'est-à-dire **à chaque ouverture
de fenêtre**, donc en marche nominale du produit.

| Exécution | Cible | Sous | `survit` |
| --- | --- | --- | --- |
| R6 | 1920×1080 | `CDS_UPDATEREGISTRY` | **false** |
| R7 | 2560×1440 | `CDS_UPDATEREGISTRY` | **false** |
| R8 | 1600×900 | `flags = 0` | **false** |
| R9 | 1600×900 | `flags = 0` | **false** |

**Les quatre cibles sont distinctes de la taille de création** — la
non-satisfaction du critère est donc informative, contrairement à un cinquième
run (cible 1280×720 = taille de création) que l'implémenteur a lui-même identifié
comme **confondu**, conservé et **étiqueté** comme tel.

⚠️ **Aucune exécution n'a jamais vu `survit = true`.** Le contrôle n'a donc pas
été observé capable de l'autre valeur — la réserve est portée telle quelle. Ce
qui l'atténue, sans la lever : le verdict est **cohérent en direction sur quatre
tirages indépendants** et sous **deux jeux de drapeaux**.

⚠️ **Deux replis d'instrument corrigés en cours de tâche, et il faut le dire** :
`survit = true` pouvait être rendu par une sortie **qui avait disparu**
(sentinelle `(0,0) == (0,0)`), et `mouvement_observe = true` par un **tour vide**.
Un verdict positif doit exiger que la chose mesurée existe encore.

### 2.4 La pollution du registre : CONFIRMÉE et ATTRIBUABLE PAR GUID

**3 transitions probantes**, chaîne `avant(N) = après(N-1)`. **Les GUID jamais
visés par l'API ne sont jamais pollués** — vérifié sur 9 exécutions, par
correspondance de `guid=` dans les journaux. C'est la réfutation directe de
l'agnosticisme que la première rédaction du rapport de tâche défendait.

⚠️ **La quatrième transition n'est PAS probante** (cible 1280×720 = taille de
création, non discriminante). ⚠️ **Et « survit à une purge » n'est établi par
rien** : les six purges des journaux portent toutes `retirees=0` — **le chemin de
retrait n'a jamais couru**.

### 2.5 Le mécanisme n'est PAS expliqué

Il est **séparé en deux régimes observables**, et un **confondeur covarie
exactement avec leur frontière** : la duplication de la sortie sous test est
**ouverte** aux créations de voisines, **fermée** à la création du témoin.
**Séparé, pas expliqué.**

### 2.6 La décision, et sa forme

La règle écrite d'avance (§3.6 de la conception) couvrait « le pilote refuse ».
Elle ne couvrait pas « il accepte, puis défait ». **Décision du propriétaire du
dépôt** : l'abandon est établi, et la forme retenue est le **retrait du code**,
pas un second désarmement.

Ce qui disparaît : `windows_source/redimensionnement/mode_sortie.rs` (458 l.),
`PLEIN_ECRAN_MODE_SORTIE`, `changement_de_mode_arme`,
`reconstruire_sur_la_sortie`, `DesktopCapture::cible()` (orphelin non
revendiqué), et la ligne correspondante de `scripts/run-agent.sh`.

Ce qui reste **actif et livré** : la **détection** du plein écran par le style de
fenêtre, l'**annonce** `PleinEcran` → `Fullscreen`, l'**armement** client et
Keyboard Lock. Le constat de mesure vit **en tête de
`agent/src/capteur/plein_ecran.rs`**, et **cinq commentaires du dépôt y
renvoient** : c'est le point de référence à ne pas déplacer.

⚠️ **Deux symboles sont conservés sans appelant, délibérément** :
`TAILLE_MAX_SORTIE` et `borner_a_la_taille_max` (`windows_source/sortie.rs`).
Ce sont deux des onze avertissements `dead_code` de la compilation croisée, et ils
sont justifiés dans le code. **Voir la réserve du §7, transverse n°4.**

---

## 3. Recette ① — la chaîne `Resize`, et le HiDPI

**Deux exécutions vertes concordantes, une exécution ROUGE, une tentative
supplémentaire non concluante.** Instrument :
`journaux-multifenetres-d9/instrument/pilote-critere1.mjs` (neuf, six défauts
d'instrument trouvés et corrigés **en cours d'exécution**, chacun documenté à
l'endroit du code qu'il concerne).

| Point | Verdict | Exéc. |
| --- | --- | --- |
| **(a)** rejeu du `Resize` différé à l'ouverture du canal | **NON EXERCÉ** | 2 vertes + 1 tentative forcée |
| **(b)** chaque `contrôle reçu Resize` porte son champ `session` | **TENU** — 4 lignes / 5 lignes / 4 lignes | 2 + 1 rouge |
| **(c)** même unité annonce et `Resize` à `deviceScaleFactor = 2` | **TENU sur vert, RÉFUTÉ sur rouge** | 2 + 1 rouge |
| **(d)** détection plein écran **exclusive** (non-régression) | **TENU** — 3 bascules de style, 3 sessions distinctes (`w-3`/`w-5`/`w-7`), aucune fuite | 2 |

**(c) en détail** : sur ROUGE (client d'avant la tâche 5), l'annonce vaut
1280×720 et le premier `Resize` reçu par l'agent vaut **2560×1440** — exactement
un facteur `devicePixelRatio`. Sur VERT, les deux valent 1280×720. **Le contrôle
a été vu ROUGE**, ce que le §7.1 de la conception exigeait.

⚠️ **Mais l'A/B n'est PAS propre, et c'est une réserve portée, non corrigée** :
« seul le client diffère » entre rouge et verts est **FAUX** — le rouge a tourné
avec `DIVISER_CSS_PAR_DPR=0`, donc une géométrie **double** (1280×720 CSS contre
640×360). Le verdict (c) survit parce que le critère est **intra-run** (comparer
l'annonce au `Resize` de la même exécution), mais **la ligne de commande du
rouge n'est versée nulle part**.

⚠️ **(a) n'est pas démontré de bout en bout.** Le réseau local de cette VM est
trop rapide pour provoquer la course naturellement, et la forcer par latence
artificielle a **cassé la reconnexion** au lieu de la ralentir (1 exécution,
versée, non concluante, **mécanisme d'échec non diagnostiqué**). La mécanique
reste couverte par ses **5 tests unitaires** (`client/src/resize.test.ts`).
**Ne pas lire « rejeu prouvé en conditions réelles ».**

⚠️ **La pièce de (a) ne couvre qu'UNE page sur treize** — `STATS` n'est évalué
que sur la cible —, et **pas les deux sessions muettes du §8**, pour qui
« aucun `Resize` » est précisément la forme du défaut.

⚠️ **Le HiDPI reste structurellement partiel** : `deviceScaleFactor = 2` a été
exercé sur la **symétrie d'unité**, jamais sur le **coût** (§7, transverse n°4).

---

## 4. Recette ② — l'audio mort : non exerçable, et pour deux raisons

**Six exécutions officielles, plus trois exécutions de validation
méthodologique — neuf au total.**

### 4.1 Le constat central : le déclencheur n'atteint jamais le code

**Quatre méthodes de disruption, zéro ligne `lecture audio échouée`, sur les neuf
exécutions versées.**

| Déclencheur | Cause nommée par `agent/src/audio.rs` | Effet sur la SOURCE | Effet sur `capture.read()` |
| --- | --- | --- | --- |
| `Restart-Service Audiosrv -Force` (celui du brief) | « redémarrage du service audio » | silence, ne revient jamais dans la fenêtre observée | **zéro erreur** (5 exécutions) |
| `Stop-Service` + 1,5 s + `Start-Service` | idem, en soutenu | silence, puis remontée lente | **zéro erreur** |
| `Stop-Process audiodg -Force` | hors motif — le moteur audio | aucun effet audible | **zéro erreur** ⚠️ effet réel sur `audiodg.exe` **non établi** (aucun PID versé) |
| `Disable-PnpDevice` / `Enable-PnpDevice` sur les deux endpoints | « changement de périphérique » | aucun effet audible | **zéro erreur** |

**🔵 L'explication, et c'est le fait le plus réutilisable de la branche : la
capture *process loopback* est liée à l'ARBRE DE PROCESSUS, jamais au service
audio ni au périphérique de rendu.** Toute disruption au niveau service ou
endpoint est donc **structurellement le mauvais levier**.

**Corroboration** que le rapport de tâche ne donnait pas : `compteurs audio
actif=true` et `paquets_recus` passant de **194 à 6156** établissent que `read()`
**était bien appelée** — le zéro n'est pas l'artefact d'une fenêtre qui ne lit
jamais.

**Cinquième déclencheur nommé et JAMAIS ESSAYÉ** : tuer le `chrome.exe` **cible**
du process loopback.

### 4.2 Les verdicts, et pourquoi ils ne jugent pas le remède

| Montage | Verdict | Exéc. |
| --- | --- | --- |
| **A — réarmement après répit** | **NON DÉMONTRABLE tel que prescrit, ET INSATISFIABLE PAR CONSTRUCTION** | 2 |
| **B — promotion d'une voisine** | **NON DÉMONTRABLE** — la porteuse ne change jamais (`w-4` avant, `w-4` après), faute de toute sollicitation | 2 |
| **contrôle ROUGE** (commit `03ea49a`, avant les tâches 7-9) | **NON DISCRIMINANT** — le groupe reste muet 58,8 s, mais le **même** silence se reproduirait sur un binaire au remède parfait | 1 + 3 validations |
| **non-régression du rattachement** (D4) | **TENU** — capteur relancé en **0,05 s**, canal rattaché **0,25 s** après, **0** clôture de session, **0** enfant terminé | 1 |

⚠️ **Le montage A avait DEUX causes d'échec, et seule la première était connue
avant la revue.** Même avec un déclencheur qui aurait fait échouer
`capture.read()`, le critère n'aurait **pas pu** être satisfait : la réélection
ne reconstruit rien (§4.3).

**Le piège de D8 n'a pas été rejoué** : les deux fenêtres du montage B partagent
un `--user-data-dir`, donc un seul `chrome.exe` — PID **18408** puis **11900**,
vérifié par la voie agent (`grep 'audio activé'`, deux lignes par PID), et non
supposé d'un nom d'exécutable.

### 4.3 🔴 La réélection après répit est INERTE

Découverte par la revue de la tâche 15, **vérifiée sur le code** :

- `WindowsAudioSource` n'est construite **qu'une fois**, au démarrage de l'enfant
  (`demarrage/audio.rs::brancher`, appelé sans boucle) ;
- le fil de capture (`windows_audio.rs`), une fois `capture_morte` posé, exécute
  un `return` **définitif** et ne relit plus jamais rien ;
- réélire la même session ne fait que pousser `Audio { actif: true }` →
  `AudioSource::set_actif(true)`, **lequel n'écrit qu'un booléen atomique que ce
  fil mort ne lira plus jamais**.

**Rien, nulle part, ne reconstruit la source.**

✅ **La branche PROMOTION reste valide** : une voisine du même groupe de PID a sa
propre `WindowsAudioSource`, sur un fil qui n'a jamais échoué.

❌ **Le cas MAJORITAIRE — une application, une fenêtre, aucune voisine — reste
SANS REMÈDE.** Décision de l'utilisateur : **corriger l'affirmation, léguer le
remède**. Le commentaire de `REPIT_REARMEMENT_AUDIO` porte désormais la
réfutation, vérifiée contre le code par la revue.

⚠️ **Corollaire trouvé par la revue transverse** : `REARMEMENTS_MAX` **ne peut
pas mordre dans ce même cas** — voir §7, transverse n°6.

---

## 5. Recette ③ ① — le critère ④ de D6 à palier long

**Deux exécutions retenues, quatre déplacements de focus.** Règle de sélection
énoncée avant tout décompte : l'ensemble des déplacements joués par le pilote
dans les deux exécutions versées. ⚠️ **Deux exécutions ANTÉRIEURES ont été
écartées** avant de fixer le protocole — ce sont des exclusions, déclarées comme
telles.

| Exécution | Sessions établies | Déplacement 1 | Déplacement 2 |
| --- | --- | --- | --- |
| `focus-1` | **3** | `w-4` : 921 600 vs 230 400 → **promotion** | `w-6` : 921 600 vs 589 824 → **promotion** |
| `focus-2` | **3** | `w-4` : 921 600 vs 589 824 → **promotion** | `w-6` : 589 824 vs 589 824 → **pas de promotion au relevé** |

**3 promotions sur 4 déplacements.** Palier de **60 s** aux quatre déplacements,
contre `DELAI_REMONTEE = 20 s` — trois fois la temporisation, là où D6 mesurait à
25 s (25 % de marge). Échelle observée **posée** avant chaque palier (le FAIT,
jamais une durée). Chaque promotion est jugée sur une **taille relevée**
(`getStats()`, `frameWidth × frameHeight`), jamais sur un compte de lignes.

⚠️ **Le seul non-promu N'EST PAS une égalité stable, et le rapport de tâche
l'affirmait d'abord** : `agent-critere-3-focus-2-plat.log` porte, **0,83 s après
le relevé**, un `taille d'encodage changée … width=1280 height=720` pour `w-6` —
soit **≈ 61,0 s** après l'imposition du focus, **juste au-delà du palier de 60 s
appliqué**. **C'est littéralement le phénomène que le palier de 60 s existait
pour éliminer.** L'attribution n'est pas tranchable sur les pièces (la ligne
précède de 0,15 s les premières traces de démontage du pilote) : ce n'est ni une
promotion établie, ni un contre-exemple net.

⚠️ **Cette campagne NE SE COMPARE PAS à D6** : trois variables changent d'un coup
(budget, effectif, palier). ⚠️ **`BUDGET_BPS = 8 000 000`, pas la valeur livrée
(12 M)** — à 12 M et trois fenêtres, la part par fenêtre reste au-dessus du seuil
`BPP_MIN`, donc **aucune pression, donc aucune promotion possible par
construction**. L'observation la plus intéressante est que **le bras 8 Mb/s de D6
rapportait DÉJÀ 3/4** au même budget, à effectif bien plus grand et palier 2,4×
plus court.

---

## 6. Recette ③ ② — l'A/B sur `set_desired_bitrate` : joué, et il n'établit rien

Le leg n°4 de D6 — « le point ouvert le plus important » — a enfin son bras
désarmé : `PART_SONDAGE=0` (variable de banc, jamais une configuration livrée,
convention de `PLEIN_ECRAN` : `=0` désarme, une présence n'active pas).
**Le désarmement est vérifié sur pièces** : la trace `objectif de sondage DESARME
(PART_SONDAGE=0)` apparaît exactement une fois par session dans les deux
exécutions désarmées.

| Exécution | Bras | Fenêtres mesurées | Mb/s cumulés | `packetsLost` |
| --- | --- | --- | --- | --- |
| `ab-arme-1` | ARMÉ | 3 | **11,439** | 0 |
| `ab-arme-2` | ARMÉ | 3 | **6,247** | 0 |
| `ab-desarme-1` | DÉSARMÉ | 3 | **7,546** | 0 |
| `ab-desarme-2` | DÉSARMÉ | 3 | **6,813** | 0 |

Moyenne ARMÉ **8,843 Mb/s**, DÉSARMÉ **7,180 Mb/s** — **écart +23,2 %, dans le
sens attendu.**

⚠️ **L'écart INTRA-bras est plus grand que l'écart INTER-bras** : 11,439 contre
6,247 sur le même bras armé, soit **+83,1 %**. **Le bruit dépasse le signal.**

**Verdict** : l'A/B montre une différence **directionnelle** cohérente avec un
effet de `set_desired_bitrate` sur le trafic émis, **mais cette différence n'est
pas distinguable du bruit de mesure** avec deux exécutions par bras. **Il
n'établit donc PAS que l'appel a un effet observable**, et **à plus forte raison
pas qu'il est nécessaire** — la prémisse qui le disait « le plus important »
reste réfutée par D6 elle-même (pont ≥ 1,44 Gb/s, `packetsLost = 0`).

**Le leg reste dû.** Ce qu'il faut est **plus d'exécutions**, pas un autre
montage — et de préférence à plus de trois fenêtres, donc après le legs n°4.

⚠️ **Trois fenêtres mesurées aux quatre exécutions**, pas huit (§9). La base de
comparaison est identique aux quatre, ce qui rend l'A/B légitime **sur un
effectif de 3**, jamais sur celui du brief.

---

## 7. La revue transverse de fin de branche — six défauts

Elle a trouvé **cinq** défauts en D7 et **trois** Critiques en D8. Elle en trouve
**six** ici, et **tous ont la même forme** : chacun est correct des deux côtés
pris séparément, et franchit une **frontière de tâche**. **Une revue par tâche ne
peut structurellement pas les voir.**

### n°1 — `capteur/fenetre/commandes.rs` décrivait un mécanisme retiré ✅ CORRIGÉ

Le commentaire du bras « fenêtre endormie » affirmait encore : « `resize` change
désormais le mode de la sortie virtuelle », et en tirait une **conséquence
assumée** — un plein écran demandé pendant le sommeil serait **perdu**. La
**tâche 3** avait retiré le mécanisme ; la **tâche 9** a édité ce fichier
**cinquante lignes plus haut** sans le voir. `redimensionnement.rs` et
`sortie.rs`, eux, avaient bien été annotés par la tâche 3.

**Ce commentaire a donc dit successivement deux choses fausses** : d'abord « sans
effet » (réfuté par D8), puis « change le mode » (périmé par D9).

### n°2 — `client/src/main.ts` justifiait le correctif HiDPI par une conséquence déjà supprimée ✅ CORRIGÉ

La tâche 5 a écrit : « sans ce facteur, CHAQUE connexion de CHAQUE fenêtre
déclencherait un changement de mode ». **La tâche 3, un commit plus tôt dans la
même branche, avait retiré le changement de mode.** La raison écrite dans le code
était donc fausse **au moment même où elle était écrite**.

**Le correctif lui-même reste juste** — l'annonce de viewport et le `Resize` de
routine doivent parler la même unité —, mais **sa vraie raison est autre** : le
viewport annoncé décide la **taille de la sortie virtuelle créée**, et sans dpr un
client HiDPI recevait une sortie plus petite que sa surface d'affichage réelle.
Le commentaire dit désormais cela.

⚠️ **Le verdict (c) de la recette ① hérite du cadrage périmé** : il conclut
« le défaut était réel et est corrigé », ce qui reste vrai de l'asymétrie
d'unité, **pas de la conséquence produit annoncée**, qui n'existait plus.

### n°3 — `capteur/audio.rs` portait encore l'affirmation réfutée ✅ CORRIGÉ

Le test `un_groupe_entierement_inapte_reste_muet` commentait : « le réarmement
après répit est le **seul remède** ». Réfuté par la tâche 15, corrigé dans
`sommeil.rs`, **pas ici**. C'est le **naufrage du « 487 », sixième occurrence** —
corriger une affirmation là où on l'a montrée plutôt que là où elle vit.

### n°4 — la taille de sortie n'est bornée par personne, et D9 la fait doubler ⛔ LEGS

`superviseur::boucle::creer_sortie` passe le viewport annoncé au pilote **tel
quel**. La tâche 5 fait désormais annoncer ce viewport en **pixels
périphériques** : à `devicePixelRatio = 2`, une fenêtre de 1280×720 CSS demande
une sortie de **2560×1440**, soit **quatre fois** les pixels à capturer et
encoder. Dans le même sous-bloc, **la seule fonction du dépôt qui sache poser un
plafond** — `borner_a_la_taille_max` (1920×1080) — **perd son dernier appelant**
avec le changement de mode.

**Ce n'est pas une régression** (la création n'a jamais été bornée), **c'est une
lacune que D9 rend plus mordante**. D6 avait relevé le décodeur du navigateur
saturé dès huit fenêtres de 1280×720. **Aucun client HiDPI réel n'a été mesuré.**
Documenté auprès de la fonction et dans `main.ts` ; **non corrigé**.

### n°5 — le leg 2 est fermé sur UN registre, pas sur les deux ⛔ LEGS

La tâche 10 a donné une génération monotone aux inscriptions du registre de
sommeil (`capteur/sommeil/registre.rs`). **`capteur/serveur.rs::oublier` porte la
même course F5** sur le registre d'attentes de connexion média, avec un `remove`
**inconditionnel** — et son propre commentaire la décrit déjà (« course jugée
négligeable, pas inexistante »). **Le brief de la tâche 10 ne nommait que
`sommeil`.** Documenté auprès de la fonction ; **non corrigé**. Le remède serait
le même patron.

### n°6 — `REARMEMENTS_MAX` ne mord pas dans le cas majoritaire ⛔ LEGS

`sommeil/porteurs.rs` remet le compteur de réarmements à zéro dès qu'une session
est **décidée** porteuse (`if actif { rearmements.remove(…) }`). Or `actif` est
une **décision d'arbitrage**, pas la preuve qu'un son sorte — et la tâche 15 a
établi que la réélection ne restaure rien. Pour une fenêtre **seule de son groupe
de PID**, la sortie de répit la rend **automatiquement** porteuse : le compteur
repart de zéro à chaque cycle.

**Le garde-fou du §5.1 point 4 de la conception — « pour qu'un périphérique
définitivement mort ne tourne pas sans fin » — ne s'applique donc en pratique
qu'aux groupes de PID à PLUSIEURS fenêtres**, c'est-à-dire exactement là où la
branche promotion couvre déjà le besoin.

**Non corrigé, délibérément** : la sanction n'a aucune conséquence observée
aujourd'hui (le cycle ne tourne pas sans fin pour autant, l'enfant ne resignalant
`AudioMort` qu'après un rattachement). Le remède juste est de refermer le cycle
sur une **preuve** de son, et cette preuve viendra avec le legs n°1.

### Ce que la revue transverse a AUSSI vérifié, sans rien trouver

- **Le bras catch-all de `capteur/pont_media.rs`**, payé quatre fois (D5
  `Sommeil`, D6 `Part`, D7 `Audio`, D8 `PleinEcran`) : `AudioMort` étant un
  `VersCapteur` et non un `DepuisCapteur`, il n'est **structurellement pas** sur
  ce chemin. **Cinquième fois évitée.**
- **La transposition de `sommeil/registre.rs`** : diff du bloc déplacé, seule
  différence = les visibilités `pub(super)`. Les purges de `oublier` (dont
  `inaptes` et `rearmements`, ajoutées par la tâche 8) ont bien survécu à
  l'extraction de la tâche 10.
- **`scripts/run-agent.sh`** : `PLEIN_ECRAN_MODE_SORTIE` retirée,
  `PART_SONDAGE` ajoutée **dans la tâche qui la crée** (le piège payé en D1, D2
  et D6).
- **Les annotations de retrait** dans `superviseur/table.rs`,
  `boucle/placement_periodique.rs`, `table/tests_retention.rs`,
  `windows_source/sortie.rs`, `windows_source/redimensionnement.rs` : toutes
  correctes et à jour.

---

## 8. Le leg 10 : décidable, et la réponse contredit le rapport qui l'a mesurée

D8 demandait « pourquoi si peu de `Resize` ? » et laissait la question
**indécidable**, faute de champ `session` sur la trace `contrôle reçu`
(correction I8 de D8). **La tâche 5 a posé ce champ ; la tâche 14 l'a mesuré.**

Inventaire réel, `agent-critere-1-1.log` :

| Session | `Visibility` reçus | `Resize` reçus |
| --- | --- | --- |
| `w-2` | 3 | **0** |
| `w-3` | 3 | 3 |
| `w-5` | **1** | **0** |
| `w-7` | 13 | 1 |

**Deux sessions sur quatre, canal de contrôle DÉMONTRÉ VIVANT (les `Visibility`
arrivent), n'émettent aucun `Resize`.**

⚠️ **Le canal vivant ÉCARTE l'hypothèse « canal mort » pour ces deux sessions ;
il ne DÉSIGNE PAS le client.** C'est exactement la nuance que la correction C3 de
D8 avait payée en réfutant l'argument inverse. **Le maillon fautif reste non
identifié.**

⚠️ **Le rapport de la tâche 14 déclare ce leg clos en sens inverse, et il n'a pas
été corrigé** (ronde de correction interrompue de façon assumée par le
propriétaire du dépôt ; arbre propre, aucun commit). ~~Le contredire est le
premier travail de qui le relira.~~

❌ **CETTE DERNIÈRE PHRASE EST FAUSSE (7 août 2026, tâche 18, D10) : il n'y aura
personne à qui « le relire ».** Le rapport vivait dans l'espace de travail
éphémère de D9 (`.superpowers/sdd/`), gitignoré et jamais commité — **vérifié
par la commande** : `git log --all --diff-filter=A --name-only --
'*task-14*'` ne rend aucun résultat pour ce chantier, sur aucune branche, dans
aucun *stash*. **Il a disparu avec la session qui l'a écrit.** Ce paragraphe,
et celui de `CLAUDE.md` qui le reprend, sont désormais la seule trace de la
contradiction — pas un renvoi vers une pièce qu'on pourrait encore consulter.

---

## 9. 🔴 Le plafond à trois fenêtres — C1 de D8, observée en train de mordre

**C'est le fait produit le plus lourd de la branche, et il n'était pas cherché.**

`superviseur/boucle.rs` exige une correspondance exacte avec la taille demandée
(1280×720, à `placement::TOLERANCE_PX = 4` près) et **rend la sortie au pilote**
sinon. Or, sur cette VM, **les sorties naissent à 3840×2160** — la dernière taille
laissée au registre par une mesure antérieure.

Résultat, aux **six** exécutions de la recette ③ **sans exception** : **trois**
sessions établies (`enfant lancé` = 3, `fenêtre attachée au capteur` = 3), et
toutes les tentatives au-delà échouent sur
`sortie créée mais introuvable dans la topologie DXGI … apparues=["\\.\DISPLAY8 3840x2160"]` :

| Exécution | `ERROR` de ce type |
| --- | --- |
| `focus-1` | **20** |
| `focus-2` | **16** |
| `ab-desarme-2` | **12** |

**`DISPLAY8` est à 3840×2160 dans les SIX exécutions, sans exception.**

⚠️ **« Inatteignable » vaut pour le PRODUIT, qui n'écrit plus au registre depuis
la tâche 3. Cela ne vaut pas pour ce qui y a DÉJÀ été écrit, et rien ne nettoie
derrière.** Le remède opérationnel reste la sonde elle-même
(`MULTIFENETRE_MODE_SORTIE=1280x720`, **dans un lancement à elle seule** —
l'aiguillage retourne après la première sonde reconnue).

⚠️ **La portée reste INCONNUE** : « le mode registre est par GUID » contre « une
seule écriture empoisonne toutes les sorties futures » n'est **toujours pas
tranché**. D9 a établi l'attribution par GUID de la **pollution**, pas la portée
du **blocage**.

✅ **Le superviseur, lui, encaisse ce défaut sans jamais perdre une session
saine** : les trois sessions établies tiennent pendant que les tentatives
suivantes échouent en boucle.

⚠️ **Conséquence de méthode** : **toutes les mesures de capacité de D9 portent
sur trois fenêtres**, pas huit ni dix. Elles ne se comparent à **aucune** campagne
de D4 à D6 sur un seul chiffre absolu.

---

## 10. Ce que le code livre, et l'état du plafond de 500 lignes

**Relevé par la commande le 6 août 2026, APRÈS les dernières éditions de la
ronde** (y compris celles de la revue transverse — une table relevée en début de
ronde est fausse à la fin de la même ronde, erreur que D8 a commise en croyant
bien faire).

**Le tableau de dette a toujours DEUX lignes, et l'une d'elles a maigri** :
`encode.rs` **1536** (inchangé), `windows_source.rs` **628** (~~638~~).
**Aucun autre fichier de code source ne dépasse 500 lignes.**

| Étage | Fichier | Nature |
| --- | --- | --- |
| la règle d'inaptitude | `agent/src/capteur/audio.rs` (**228**) | **pur, aucun `cfg`** — champ `inapte`, 4 tests neufs |
| le registre | `capteur/sommeil.rs` (**269**) + `sommeil/registre.rs` (**331**) | répit, borne, génération monotone |
| le message | `capteur/protocole.rs` (**381**) | `VersCapteur::AudioMort`, poussé, non répondu par `Fait` |
| la détection locale | `transport/tick.rs` (**343**), branche **a1sexies** | verrou `audio_mort_signale`, remis à zéro par `rattachement_survenu` |
| la télémétrie | `windows_source/telemetrie.rs` (**72**) | **pur**, par session ; lue par `capteur/fenetre/trace.rs` (**43**) |
| le rejeu du `Resize` | `client/src/resize.ts` (**45**) | **pur, sans DOM**, 5 tests |
| la sonde P | `diagnostics/multifenetre/mode_sortie.rs` (**383**) + 5 enfants | `eliminatoire.rs` 307, `voisines.rs` 240, `temoin.rs` 200, `combinaisons.rs` 157, `persistance.rs` 107 |

**Deux marges étroites, à connaître avant de toucher à ces fichiers :**

- ⚠️ **`agent/src/superviseur/table.rs` : 494 lignes, marge 6** — la plus serrée
  du dépôt après `encode/arret.rs` (500). Elle était de 7 à la fin de D8, et une
  **seule ligne de commentaire** de D9 a suffi à la réduire. Point de chute :
  `superviseur/table/tests_retention.rs` (326).
- ⚠️ **`agent/src/transport/tick/tests.rs` : 489 lignes, marge 11** — **neuve**,
  qu'aucun tableau ne signalait.

**Deux extractions faites sur exigence de revue, après une compression que le
dépôt interdit** : `sommeil.rs` (franchi 500 → ramené à **499 par compression**
→ **269** par extraction de `registre.rs`) et `capteur/fenetre.rs` (508 → 496 par
compression → **485** par extraction de `fenetre/trace.rs`). **Deux fois dans le
même sous-bloc.**

**Vérifications de fin de branche** : `cargo test -p agent` → **440 passed, 0
failed** (422 en début de branche, **+18**) ; `cargo check --target
x86_64-pc-windows-gnu` → **sortie 0, 11 avertissements**, tous `dead_code`, dont
**deux délibérés et justifiés dans le code** ; `npx vitest run` (client) →
**107 passed**.

---

## 11. Ce que D9 n'établit PAS

Reprend le §11 de la conception, et l'étend de ce que l'exécution a ajouté.

**Écrit d'avance, et confirmé :**

- **La latence de bout en bout**, qu'aucun sous-bloc du chantier D n'a mesurée.
- **Les trois couches inconnues** : le plafond de 8 encodeurs, celui de 4
  processus, et le mécanisme de l'abandon du mutex DXGI.
- **La course du leg 2 n'a pas été provoquée** — le correctif se prouve par tests
  d'hôte ; la VM n'établit que la non-régression du rattachement.
- **Le répit et la borne de réarmement audio ne sont pas calibrés** — comme
  `BPP_MIN`, `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`, `HYSTERESIS`,
  `REPIT_APRES_ECHEC` et `TAILLE_MAX_SORTIE`.
- **Aucun jugement visuel ni d'écoute** n'a été porté sur aucune constante.
- **Le chemin d'extinction propre du superviseur**, jamais exercé depuis D1.
- **Rien au-delà des rangs joués, et rien d'un client réel** : le montage reste
  un Chrome sans interface, à décodage logiciel, sur l'hôte qui porte la VM.
- **La visibilité et le focus restent IMPOSÉS par le pilote de recette**, page par
  page — limite héritée de D5, la plus lourde du montage, qu'aucun sous-bloc n'a
  levée.

**Ce que l'exécution y ajoute :**

- **Aucun taux, nulle part** : deux exécutions par critère au mieux, une pour
  plusieurs.
- **Le MÉCANISME de la non-persistance du changement de mode** : séparé en deux
  régimes, **pas expliqué**, un confondeur covariant avec leur frontière.
- **La PORTÉE du blocage par pollution de registre** : par GUID, ou global ? Non
  tranchée — et **rien dans le produit ne nettoie le registre**.
- **`survit = true` n'a JAMAIS été observé** : le contrôle de persistance n'a pas
  été vu capable de l'autre valeur.
- **Le leg 1 n'a jamais été exercé de bout en bout sur la VM** : aucun
  `AudioMort`, aucun `réarmement programmé`, aucun `abandon définitif` dans un
  seul journal.
- **Le rejeu du `Resize` différé n'a pas été observé en conditions réelles**, et
  le mécanisme d'échec de la tentative forcée n'est pas diagnostiqué.
- **Le maillon fautif du leg 10 reste non identifié.**
- **Rien au-delà de TROIS fenêtres**, à cause du §9 — donc rien qui se compare aux
  campagnes de D4 à D6.
- **Le critère ③ focus a tourné à `BUDGET_BPS = 8 M`, pas à la valeur livrée
  (12 M).**
- **L'A/B de `set_desired_bitrate` n'établit rien** : le bruit dépasse le signal.
- **Le coût du HiDPI n'est pas mesuré** : `deviceScaleFactor = 2` n'a été exercé
  que sur la symétrie d'unité.
- **Le cinquième déclencheur de mort de capture audio n'a pas été essayé.**
- ~~Neuf constats de revue sont PARQUÉS sur la tâche 14 et vivent toujours dans
  son rapport versé.~~ ❌ **FAUX (tâche 18, D10) : ce rapport a disparu — voir
  le §13, legs n°10, pour le sort réel des neuf constats (trois traités,
  six perdus).**

---

## 12. Pièges neufs — à connaître avant de toucher à ce terrain

- ⚠️ **Une variable de disruption peut viser la mauvaise couche entière, et cela
  se lit comme une panne du produit.** Quatre déclencheurs, neuf exécutions, zéro
  erreur : ce n'était pas le remède qui ne marchait pas, c'était le levier qui
  n'était pas relié. **Vérifier À QUOI un mécanisme est lié avant de choisir
  comment le casser.**
- ⚠️ **Un critère peut être insatisfiable pour DEUX raisons, et trouver la
  première fait manquer la seconde.**
- ⚠️ **Un compteur de fenêtres côté PILOTE compte des popups, pas des sessions.**
  Annoncé 8 puis 6 ; il y en avait **3**, aux six exécutions. La source de vérité
  est `enfant lancé`/`fenêtre attachée au capteur` dans `agent.log`.
- ⚠️ **Une promotion peut arriver 0,83 s APRÈS la fin de la mesure** — le
  phénomène même que le palier de 60 s existait pour éliminer. **Chercher
  activement l'événement juste après la fenêtre.**
- ⚠️ **Une sonde qui demande à une sortie la taille qu'elle a déjà ne peut pas
  échouer** (rejoué ici, trouvé par l'implémenteur, run **étiqueté** et
  conservé). Exclure structurellement la taille courante, juger sur le
  **mouvement**.
- ⚠️ **`survit = true` peut être rendu par une sortie qui a DISPARU** (sentinelle
  `(0,0) == (0,0)`), et `mouvement_observe = true` par un tour vide. **Un verdict
  positif doit exiger que la chose mesurée existe encore.**
- ⚠️ **Un plafond de fenêtres peut venir d'un état laissé par une mesure
  antérieure, pas du produit.** `grep 'sortie créée mais introuvable'` avant de
  conclure à un plafond.
- ⚠️ **Extraire pour rester sous 500 lignes, ce n'est pas COMPRESSER** — payé
  **deux fois** dans ce seul sous-bloc.
- ⚠️ **Une exécution peut être conforme à la LETTRE d'un brief et manquer son
  OBJET** : la première version du leg 2 frappait la génération **au lancement du
  processus**, quand la course est à l'**attache**. **Le défaut était dans la
  conception.**
- ⚠️ **Corriger un faux en produit un autre** — rejoué au moins trois fois dans ce
  sous-bloc, sur les documents de tâche, pas sur le code. Et **corriger dans un
  ADDENDUM laisse le corps faux** : le rapport de la tâche 16 a dû être repris
  une seconde fois, ses 27 occurrences classées une par une.
- ⚠️ **Le naufrage du « 487 », sixième occurrence** (transverse n°3). La règle
  n'est pas « recompter » : **« corrigé à sa place » est une affirmation de
  COMPLÉTUDE, et une affirmation de complétude se vérifie en énumérant les places
  AVANT de l'écrire** — `grep -n '<le nombre>' CLAUDE.md`.

---

## 13. Les legs — dix points, dont trois neufs

Repris intégralement dans `CLAUDE.md`, section « Sous-bloc D9 ».

1. ⛔ **Reconstruire la capture audio après sa mort** — le remède réel du leg 1.
   Point de chute : `demarrage/audio.rs`, `transport/piste_audio.rs`,
   `windows_audio.rs`.
2. ⛔ **Le leg 2 sur le SECOND registre** (`capteur/serveur.rs::oublier`).
3. ⛔ **L'A/B sur `set_desired_bitrate`** — joué, n'établit rien ; il faut plus
   d'exécutions, pas un autre montage.
4. 🔴 **Nettoyer la pollution de registre, ou s'en rendre immunisé** — elle bloque
   le produit à trois fenêtres aujourd'hui.
5. 🔴 **Borner la taille de sortie demandée** (transverse n°4).
6. ⛔ **`REARMEMENTS_MAX` ne mord pas dans le cas majoritaire** (transverse n°6) —
   se referme avec le legs n°1.
7. ⛔ **Le maillon fautif du leg 10 reste non identifié**, et ~~le rapport de la
   tâche 14 conclut l'inverse sans avoir été corrigé~~ — ❌ **ce rapport a
   disparu (tâche 18, D10) : voir le §8 pour la correction complète.**
   L'instrumentation `video.clientWidth`/`clientHeight` posée par la tâche 18
   dans `client/src/main.ts` vise ce maillon, mais **désigne une hypothèse
   parmi d'autres** — le canal de contrôle en reste une (§8). Non mesurée sur
   la VM : différée à une session ultérieure.
8. ⛔ **Le cinquième déclencheur de mort de capture audio, jamais essayé** : tuer
   le `chrome.exe` cible du process loopback.
9. ⛔ **`agent/src/survie_verdict.rs` est posé à la racine du crate** contre deux
   précédents du dépôt.
10. ⛔ ~~Neuf constats de revue PARQUÉS sur la tâche 14, toujours dans son
    rapport versé.~~

    ✅ **REQUALIFIÉ (7 août 2026, tâche 18, D10) : le rapport a disparu**
    (espace de travail éphémère, gitignoré, jamais commité — vérifié par
    `git log --all --diff-filter=A --name-only -- '*task-14*'`, aucun
    résultat pour ce chantier). **Trois des neuf constats survivent**, extraits
    vers ce document et `CLAUDE.md` avant la disparition du rapport source —
    **TRAITÉS** :
    - l'A/B rouge/vert non propre du critère ①c (§ 3 ci-dessus) ;
    - la pièce du critère (a) qui ne couvre qu'une page sur treize (§ 3) ;
    - « le shell réémet `fenetre-ouverte` pour une fenêtre déjà ouverte » —
      **REQUALIFIÉ** : ce n'est pas un défaut d'instrument mais un
      **comportement du produit, mécanisme non élucidé**.
    **Les six autres sont PERDUS**, sans pièce nulle part dans ce dépôt.
    Inventer leur contenu serait pire que de le dire.
