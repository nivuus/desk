# Sous-bloc G4 — résultats : la surveillance, et le critère que sa propre spec ne pouvait pas rendre rouge

> ✅ **LA RECETTE A EU LIEU.** L'en-tête de la première rédaction de ce document
> disait *« la recette qui N'A PAS EU LIEU »* — la VM était prise par le
> sous-bloc F4. Elle a été rendue, et **les quatre critères sont mesurés**,
> plus deux relevés hors critères. Le §7 énumère toujours ce qui n'est pas
> établi, et il a **rétréci sans disparaître**.

Plan : `2026-08-21-gestion-apps-g4.md` (`d25176d`).
Conception : `../specs/2026-08-19-gestion-apps-design.md`, §G4.
Journaux : `journaux-gestion-apps-g4/`.

---

## 1. Familles de lecture des journaux — **DEUX**, remesurées APRÈS la recette

| Famille | État relevé | Ce qu'il faut faire |
| --- | --- | --- |
| tous les journaux de recette (`c1-*`, `c24-*`, `c3a-*`, `c3b-*`, `c2bis-*`, `c5-*`, `c6-*`, `agent-*-plat`) | UTF-8, **CRLF**, **aucune séquence ANSI**, **aucun octet NUL** | **rien** — les CR ne gênent aucun `grep` |
| les journaux d'hôte (`s1-*`, `verify-all-*`, `rouge*`, `t4-*`) | UTF-8/ASCII, LF | **rien** |

Relevé par `file -b`, `grep -lP`, un balayage `tr -dc '\000'` et `grep -lU $'\r'` :
voir `journaux-gestion-apps-g4/familles-de-lecture.txt`.

🔵 **AUCUN journal ne porte de séquence ANSI, et ce n'est pas de la chance** :
les journaux de sonde sont écrits par un `StreamWriter` UTF-8 côté VM, et les
copies d'`agent.log` sont **mises à plat au `sed` avant d'être versées**. Les
CRLF viennent de la VM et sont inoffensifs.

⚠️ **`familles-de-lecture.txt` S'EST POLLUÉ LUI-MÊME à sa première rédaction** :
ses commandes de contrôle passées à `echo` entre guillemets **doubles** ont vu
leurs échappements interprétés, et le fichier a reçu un **vrai octet NUL** — il
se classait alors « data », c'est-à-dire exactement ce qu'il servait à
détecter. C'est le piège du sous-bloc S3, repayé. Réécrit par heredoc **cité**,
et le contrôle rend désormais **0**.

## 2. Ce que G4 livre

| Étage | Fichier | Lignes | Nature |
| --- | --- | --- | --- |
| les quatre états | `agent/src/apps/surveillance/mode.rs` | **146** | **PUR**, 4 tests |
| l'anti-rebond | `agent/src/apps/surveillance/rebond.rs` | **207** | **PUR**, horloge en paramètre, 6 tests, **aucun ne dort** |
| l'injection | `agent/src/apps/surveillance/faute.rs` | **233** | **PUR** + un budget **global au processus**, 6 tests |
| les compteurs | `agent/src/apps/surveillance/partage.rs` | **163** | **SANS `cfg`**, 4 tests |
| une racine | `agent/src/apps/surveillance/racine.rs` | **325** | `#[cfg(windows)]`, **aucun test possible** |
| le fil | `agent/src/apps/surveillance/fil.rs` | **221** | `#[cfg(windows)]`, **aucun test possible** |
| le parent | `agent/src/apps/surveillance.rs` | **106** | **SANS `cfg`** — `demarrer`, `TAMPON_NOTIFICATIONS` |
| l'extraction | `agent/src/apps/boucle/memoire.rs` | **293** | **VERBATIM**, jouée AVANT l'addition |
| le câblage | `agent/src/apps/boucle.rs` | **360** | le sondage, le déclencheur, le mode, **E5**, **D7** |
| le câblage | `agent/src/apps.rs` | **307** | le mode, l'`info!`, la troisième poignée |
| le script | `scripts/run-agent.sh` | **163** | **deux lignes, tâche DÉDIÉE** |

**Tailles relevées par la commande APRÈS la dernière édition.** Porte du
sous-bloc : **450**. **Aucun fichier ne l'atteint** ; le plus gros est
`boucle.rs` à 360.

🔴 **L'EXTRACTION A GAGNÉ EXACTEMENT CE QU'ELLE PROMETTAIT.** `boucle.rs` valait
**385** ; sans l'extraction il vaudrait **385 + 207 = 592**, donc il aurait
franchi **450 ET 500**. La marge a été rendue **avant** d'être consommée — la
forme forte de D9 (tâche 6) et de D10 (tâches 1 à 3), jamais la forme faible
« on franchit puis on rattrape », que ce dépôt a payée cinq fois dont deux par
une compression qu'il interdit.

---

## 3. Les mesures d'hôte, chacune **annoncée avant d'être prise**

| Commande | Annoncé | Mesuré |
| --- | --- | --- |
| `cargo test -p agent` | **944** | **944 passed, 0 failed** |
| `cargo test -p proto` | 109 | **109** |
| `cd proto && npx vitest run` | 296 | **296** |
| `cd client && npx vitest run` | 466 | **466** |
| `plateforme` sqlite / postgres | 564 / 564 | **564 / 564** |
| `cargo check --target x86_64-pc-windows-gnu` | sortie 0, tous `dead_code` | **sortie 0, 23 avertissements, 0 hors famille** |
| `./scripts/verify-all.sh` | sortie 0 | **sortie 0 — « Les 10 étapes sont passées »** |

**944 = 924 (départ) + 4 + 6 + 6 + 4.** Journal :
`journaux-gestion-apps-g4/verify-all-cloture-hote.log`.

⚠️ **Le départ est 924 et non les 916 du plan**, et c'est **attribué, pas
supposé** : F4 a commité `93873c6` entre la mesure du plan et la mienne, et ce
commit ajoute **+8** fonctions de test dans `agent/` (`git show | grep -c
'#\[test\]'`). *Un compte n'est attribuable qu'assorti de son arbre.*

⚠️ **`verify-all.sh` a rendu 1 à sa première exécution**, sur
`plateforme : npm run test:postgres`, **110 échecs**. Cause relevée dans les
journaux du conteneur : `the database system is in recovery mode` — l'instance
Postgres se resynchronisait. **Ce n'est pas G4** : `git diff --name-only
d25176d..HEAD -- plateforme/ client/ proto/ …` rend **AUCUNE ligne**. Rejouée
après reprise : **sortie 0**. Les deux exécutions sont rapportées ; taire la
première serait taire une mesure prise.

⚠️ **Dix-huit en-têtes `==>` à l'écran pour DIX appels `etape` dans le script.**
Les deux comptes sont relevés (`grep -c`), et **c'est DIX que ce document
retient** — les huit autres viennent de l'intérieur de `design:verifier`.

### `cargo clippy --workspace` : **quatre avertissements NE SONT PAS de la famille `dead_code`**

C'est un **fait relevé**, et il contredit une formule que ce dépôt emploie à
chaque clôture. Les quatre :

| Avertissement | Fichier | Dernier commit du fichier |
| --- | --- | --- |
| `unused imports: inscrire and retirer` | `agent/src/capteur/sommeil.rs:46` | `0baa783` (P2) |
| `this map_or can be simplified` | `agent/src/capteur/vivier.rs:225` | `4efad2c` (D6) |
| `manual implementation of .is_multiple_of()` | `agent/src/mire.rs:48` | `aa5a342` |
| `redundant closure` | `agent/src/transport/piste_audio.rs:330` | `a6f0143` (D11) |

🔴 **AUCUN N'EST DE G4, et c'est PROUVÉ et non affirmé** : les quatre fichiers
sont absents de `git diff --name-only d25176d..HEAD`. **Non corrigés** — hors
périmètre. Ils prolongent le constat que la clôture du chantier E avait fait sur
un `unused_variables` : *« tous `dead_code` » n'est plus vrai du dépôt.*

⚠️ **Le compte total de clippy (476) n'est PAS un chiffre-juge** : il dérive avec
la fraîcheur du build. **C'est la NATURE qui est vérifiée.**

---

## 4. Les décisions que la mesure ou la lecture ont tranchées

### 🔴 `DELAI_ANTI_REBOND_MAX` vaut **4 s**, et non les 5 s de la spécification

**Dérivé, pas recopié.** Le pire cas du critère ① est la somme de quatre termes,
dont **trois sont mesurés** :

```
DELAI_ANTI_REBOND_MAX
+ granularité du sondage    (200 ms, RELEVÉ dans apps/boucle.rs)
+ coût d'une réconciliation (≈ 70 ms au repos, MESURÉ)
+ 10 ms par icône neuve     (MESURÉ)
```

À **5 s** : `5 280 ms` — **au-dessus** de ce que ① exige. À **4 s** : `4 280 ms`,
**720 ms** de marge, soit **soixante-douze icônes neuves**. Un test d'hôte
vérifie cette dérivation.

🔵 **DÉRIVÉE N'EST PAS CALIBRÉE, et les deux ne sont pas la même chose** :
personne n'a jugé que quatre secondes « se sentent bien ».
⚠️ **Le pire cas reste OUVERT à un endroit, nommé et non borné** : si *k*
applications apparaissent d'un coup, le critère ① tombe dès **k > 72**.

### 🔴 E5 — un commentaire nommait exactement le défaut que son code produisait

`apps/boucle.rs` portait : *« le drapeau se baisse APRÈS la réconciliation, pas
avant : le fil d'installation attend `reconciliee`, et le lever trop tôt lui
ferait lire un compte pris avant que l'installeur n'ait fini d'écrire. »*

Le chemin **nominal** était correct. **Le chemin de course ne l'était pas** : une
réconciliation périodique **déjà en cours** quand l'installeur sortait faisait
déclarer `reconciliee` **pour un tour commencé AVANT** — un `sans-effet` **faux**,
c'est-à-dire précisément ce que le commentaire disait vouloir empêcher.

**Pourquoi c'est de G4** : la fenêtre valait ≈ **0,2 %** des sorties d'installeur,
et **G4 l'élargit d'un ordre de grandeur** puisque tout son objet est de rendre
les réconciliations plus fréquentes pendant une installation.

🔴 **SA SEULE PREUVE EST UN ARGUMENT DE FLOT DE CONTRÔLE.** `boucle.rs` est
`#[cfg(windows)]` ; aucun test d'hôte ne l'atteint, et la recette de G4 ne lance
aucune installation. **C'est la situation exacte que le défaut F1 de D7 a payée.**
Le correctif est appliqué parce qu'il est **strictement plus sûr** ; **la mesure
est LÉGUÉE, et déclarée manquante.**

### 🔴 D7 — la trace mentait sur son nom, **troisième fois dans ④**

`reenrolement observe` : **aucun réenrôlement n'a lieu.** Mesuré sur le journal
d'un agent au repos — **onze** lignes pour **douze** réconciliations. C'est le
**rafraîchissement de jeton** du battement qui fait bouger la `watch`, et
`PERIODE_BATTEMENT` vaut `PERIODE_RECONCILIATION` **par coïncidence, non par
dérivation**. Après `retenus` (G1) et `icones_echouees` (G2), **c'est le
troisième compteur de ④ à mentir sur son nom.**

🔴 **LE COMPORTEMENT N'EST PAS CHANGÉ, ET LA RAISON EST L'INVERSE DE CE QU'ON
CROIRAIT** : la boucle *pourrait* distinguer les deux, mais ce serait **retirer
une réparation réelle** — la promesse « un `Catalogue` perdu ne laisse pas la
plateforme divergente sans terme » **n'a aucune autre implémentation** que cet
envoi complet périodique. ⛔ **Legs** : le mesurer exige de peser la trame sur le
fil ; le seul chiffre disponible est celui de G1 (**56 145 octets** pour **154**
applications, **sans** `icone` ni `source_max`), et **celui d'aujourd'hui n'est
mesuré par rien**.

### 🔴 M4 cesse d'être une hypothèse — **aucune fonctionnalité de crate ajoutée**

`ReadDirectoryChangesW`, `GetOverlappedResult`, `CancelIoEx`, `CreateFileW` et
`CreateEventW` étaient toutes déjà derrière des fonctionnalités activées.
**Établi par la compilation**, pas par la lecture des bindings : G1 avait annoncé
`Win32_UI_Shell` seule pour `ShellExecuteExW` et **la compilation l'avait
réfuté**.

⚠️ **Deux affirmations de mon propre premier jet, réfutées par la compilation** :
`windows_core::Error::from_win32` **n'existe pas** en windows-rs 0.62, et
`GetLastError` y est `unsafe`.

---

## 5. Ce que j'ai fait de faux, et qui est corrigé plutôt que tu

1. 🔴 **J'AI PUBLIÉ UN COMPTE FAUX DANS UN MESSAGE DE COMMIT.** `f2476d1`
   annonce « boucle.rs 334 » et en tire « sans l'extraction, 385 + 181 = 566 ».
   **Le fichier vaut 360**, et le calcul juste est **385 + 207 = 592**. J'ai
   mesuré, **puis** ajouté vingt-six lignes de commentaire, et publié la mesure
   d'avant. **C'est le piège que le plan m'ordonnait d'éviter, dans ses propres
   termes** — *« une table mesurée en début de ronde est fausse à la fin de la
   même ronde »*, erreur de D8 — commis dans la ronde qui le prescrit. Le
   message ne peut pas être amendé ; la correction vit ici et dans `f9a08b8`.
   ⚠️ *La conclusion tient, et elle tient PLUS FORT que ce que j'avais écrit.*
2. 🔴 **L'EN-TÊTE DE `memoire.rs` A ANNONCÉ « QUATRE » `pub(super)` ; LE DIFF EN
   MONTRE CINQ.** Un compte écrit de mémoire au lieu d'être lu dans la sortie de
   la commande — **dans le fichier même dont l'en-tête promet la complétude**.
   Corrigé, avec son aveu.
3. 🔴 **UNE ROUGE N'A PAS ROUGI AU PREMIER JET, ET CE N'ÉTAIT PAS LE TEST.**
   `echeance` porte **deux** gardes (`self.premiere?` **et** `self.derniere?`) ;
   n'en lever qu'un laisse le second court-circuiter au repos. Les deux mutés,
   elle rougit. **Une rouge qui ne rougit pas est un échec de la rouge, jamais un
   succès du produit** — et le premier jet est **conservé** dans le journal.
4. 🔴 **`APPS=0` NE DÉSARMAIT PAS LA SURVEILLANCE** dans mon premier câblage :
   `surveillance::demarrer` était appelée **avant** le garde. `APPS=0` aurait
   coupé la découverte et l'installation en laissant tourner un fil, quatre
   handles et **256 Kio de pool non paginé** au service de compteurs que **plus
   personne ne sonde**. Corrigé, et **déclaré**.
5. ⚠️ **Un journal porte une ligne parasite**, `command not found: premiere` :
   des accents graves dans un libellé passé à `echo` sous zsh **exécutent une
   commande**. Piège déjà documenté par ce dépôt, repayé ici. **Non nettoyé** —
   nettoyer une pièce après coup est ce que ce dépôt refuse.
6. ⚠️ **Le journal `rouges-3-partage.log` porte une entrée numérotée 9 qui
   N'EST PAS UNE ROUGE** : un garde-fou de mon harnais laissé dans la boucle,
   qui devait ne **pas** trouver son ancre et l'a trouvée. **Annotée, et son
   numéro n'est pas réattribué** — une renumérotation tardive est le geste par
   lequel une référence survit à ce qu'elle désigne.
7. ✅ **Mon premier compte d'avertissements `cargo check` était 23 pour 22** :
   `grep -c '^warning'` compte **aussi la ligne de résumé**. Piège de G1, repayé,
   et attrapé avant publication.

---

## 6. Les gardes vus rouges — **treize mutations, une à la fois**

Harnais complet à chaque fois : copie nommée + `sha256`, mutation **par numéro
de ligne**, **preuve que le `diff` est non vide**, lecture de **quelle assertion
tombe**, restauration **depuis la copie** (jamais `git checkout --`), `sha256`
identique.

| # | Ce qu'elle mute | Verdict |
| --- | --- | --- |
| 1 | `desarme` → `is_some()` | `left: Desarmee` / `right: SansRebond` |
| 2 | l'échéance devient immédiate | ✅ |
| 3 | la seconde notification ne repousse plus | ✅ |
| 4 | la **borne haute** retirée | ✅ — *le test qui compte* |
| 5 | `consommer` ne remet pas `premiere` à `None` | ✅ |
| 6 | une échéance existe **au repos** | ⚠️ **verte au 1ᵉʳ jet**, rouge au second |
| 7 | budget **relu à chaque appel** (la panne de D10) | ✅ — le **premier** `consommer` rend déjà `false` |
| 8 | le compteur se consomme à la lecture | ✅ |
| 10 | un débordement ne déclenche plus | ✅ — `left: 0` / `right: 1` |
| 11 | l'arrêt ne se propage plus aux clones | ✅ |

*(9 n'est pas une rouge — voir §5.6.)*

🔴 **AUCUNE ROUGE N'EST JOUABLE sur les tâches 6, 7, 8 et 9** : tout ce qu'elles
changent vit dans du `#[cfg(windows)]`, hors de portée de tout test d'hôte.
**Dit plutôt que tu.**

---

## 6bis. 🔴 LA PORTE S1 — la rafale NE FAIT PAS DÉBORDER

**Sept exécutions** : 1 du premier instrument, 3 du deuxième, 3 du troisième.

| N | fichiers/s | créés | manquants | erreurs | débordements |
| --- | --- | --- | --- | --- | --- |
| 1 000 | 3 378 | 1 001 | −1 | 0 | **0** |
| 5 000 | 3 209 | 5 001 | −1 | 0 | **0** |
| 20 000 | 2 944 | 20 001 | −1 | 0 | **0** |
| 60 000 | 2 850 | 60 001 | −1 | 0 | **0** |

*(le +1 est la création du répertoire ; « manquants = −1 » dit donc que le
guetteur n'a RIEN perdu — un compte tronqué ressemble exactement à un
débordement, et les distinguer était l'objet du champ.)*

🔴 **POURQUOI, ET C'EST ARITHMÉTIQUE** : un débordement exige plus de ~1 260
événements **entre deux réarmements**. À 2 850 fichiers/s ils arrivent toutes
les ~350 µs, et le lecteur réarme bien plus vite. **Le plafond mesuré est celui
du SYSTÈME DE FICHIERS** (2 850 à 3 378/s, quasi constant de 1 000 à 60 000),
**pas celui du tampon.** La machine ne PEUT PAS produire le régime.

**Corpus 220 avant et après, aux sept exécutions.** L'instrument ne détruit pas
ce qu'il mesure : des `.tmp` que `lecture::lnk_sous` ignore **par
construction**, dans un sous-répertoire NEUF du menu Démarrer.

🔴 **DEUX INSTRUMENTS FAUX AVANT LE BON, LES DEUX JOURNAUX VERSÉS :**

1. `s1-v1-instrument-defectueux.log` — le compteur était un
   `Register-ObjectEvent -Action`. Sur 1 000 fichiers posés à 3 289/s il n'a
   rendu que **DIX** événements, à **3,6 par seconde** : **il mesurait la
   cadence de la pompe d'événements de PowerShell, pas le tampon.** Un « ne
   déborde pas » lu là n'aurait rien voulu dire. Le compteur vit désormais dans
   une classe **C#** abonnée directement au watcher.
2. `s1-v2-N-non-transmis.log` — 🔴 **`G4_RAFALE_N` n'atteignait jamais le
   processus.** Une tâche planifiée démarre dans un environnement NEUF, et
   `schtasks /run` ne transporte rien. **Trois exécutions demandées à 1 000,
   5 000 et 20 000 ont TOUTES tourné à 1 000**, et seul l'en-tête du journal l'a
   dit. **C'est le piège de `run-agent.sh`, payé cinq fois par ce dépôt, rejoué
   par mon propre instrument.** N passe désormais par un fichier.

---

## 6ter. LA RECETTE — les quatre critères, **deux exécutions chacun**

**Vingt exécutions de sonde, onze lancements d'agent.** Aucun taux n'est
revendiqué : deux exécutions établissent la reproductibilité, pas une fréquence.

🔵 **LE CONTRÔLE QUI VAUT A ÉTÉ JOUÉ À CHAQUE LANCEMENT** : `APPS_SURVEILLANCE`
et `APPS_FAUTE` ont été relevées **dans le `C:\dev\run-agent.ps1` GÉNÉRÉ**, pas
dans le tracé du code — le piège de D1, D2 et D7. Et la trace
`mode de surveillance retenu mode=…` a confirmé la valeur **arrivée**.
⚠️ **Ce qui discrimine reste le COMPTE**, jamais la trace : au bras `=0`,
**aucune** ligne `racine surveillée` n'apparaît — le fil n'a pas démarré.

🔵 **ET LE BINAIRE EST IDENTIFIÉ PAR UNE CHAÎNE, JAMAIS PAR SA TAILLE.**
`strings agent.exe` rend **1** pour `mode de surveillance retenu`, `notifications
perdues` et `APPS_SURVEILLANCE`, et **0** pour un témoin négatif — le contrôle
peut donc échouer. *Le contrôle par la taille est mort : F4 a mesuré que son vert
restauré pesait exactement autant que son rouge.*

### ① Un raccourci créé apparaît en moins de 5 s — **TENU**

| Bras | exéc. 1 | exéc. 2 | déclencheur | notifications |
| --- | --- | --- | --- | --- |
| **VERT** (variable absente) | **960 ms** | **999 ms** | `notification` | 2, 5 |
| **ROUGE** (`APPS_SURVEILLANCE=0`) | **29 997 ms** | **29 966 ms** | `periode` | **0** |

**31× plus rapide, et le rouge dépasse les cinq secondes d'un facteur six.**

🔵 **LA MESURE CORROBORE LA DÉRIVATION DE D6, ET CE N'ÉTAIT PAS CHERCHÉ** :
`750 ms` (anti-rebond) `+ ≤200 ms` (granularité du sondage) `+ ~70 ms`
(réconciliation) = **960 à 1 020 ms**. Les deux mesures tombent dedans.

⚠️ **Le rouge n'est rouge que parce que le témoin est créé JUSTE APRÈS une ligne
`catalogue reconcilie`** : un raccourci créé une seconde avant une
réconciliation périodique serait passé. Le protocole attend cette
synchronisation, et le journal l'écrit (`attente_synchro_ms`).

⚠️ **La ROUGE que la spécification nomme — « le binaire de G1 » — N'A PAS ÉTÉ
JOUÉE, et ne pouvait pas l'être** : `PLATEFORME_VERSION` vaut 4, un agent v1/v2
boucle sans terme sans pouvoir lire le refus. `APPS_SURVEILLANCE=0` est
**meilleure** : même binaire, même corpus, même machine, une seule variable.

### ② Un débordement est détecté et journalisé — 🔴 **NON MESURABLE**

**Rafale de 20 000 fichiers rejouée sur le produit, deux exécutions** :

| | exéc. 1 | exéc. 2 |
| --- | --- | --- |
| notifications reçues | **96 742** | **96 328** |
| `debordements` | **0** | **0** |
| lignes `notifications perdues` | **0** | **0** |

🔴 **LE VERDICT EST ÉCRIT TEL QUEL, ET IL ÉTAIT PRÉVU** : S1 l'annonçait, et le
produit le confirme sur **~96 000 notifications réelles**. **Le tampon n'a PAS
été rétréci pour faire passer le critère** — ce serait régler le produit sur son
test, et `TAMPON_NOTIFICATIONS` porte cette interdiction dans son commentaire.

### ②bis L'injection — **le chemin de code EST exercé** (1 exécution)

`APPS_FAUTE=debordement:3` : la ligne `notifications perdues` sort **exactement
trois fois** (`debordements=1`, `2`, `3` — budget **global au processus**,
épuisé une seule fois), et la ligne `catalogue reconcilie` porte
`debordements=3`.

🔵 **ET ELLE ÉTABLIT LA PROPRIÉTÉ QUI FERME LE CHEMIN DE PERTE** : le
débordement porte `declencheur="notification"` et `notifications=2 debordements=2`
— **les deux compteurs montent ensemble**, donc un débordement DÉCLENCHE la
réconciliation qui répare. C'est exactement ce que `signaler_debordement`
promet.

🔴 **CE QUE L'INJECTION N'ÉTABLIT PAS, ET C'EST ÉCRIT MOT POUR MOT** : elle
établit que le **REMÈDE** fonctionne, **jamais qu'une CAUSE existe**. Rien ici ne
dit qu'un débordement réel se produira jamais sur cette machine — et S1 comme la
rafale disent le contraire.

### ③ Un débordement ne perd aucune application — **TENU**, et **le montage de la spec est NON DISCRIMINANT**

**Montage A, celui de la spécification** (rafale + témoin pendant, ≥ 2 périodes) :

| Bras | exéc. 1 | exéc. 2 |
| --- | --- | --- |
| VERT (variable absente) | `cles=157` | `cles=157` |
| **ROUGE (`APPS_SURVEILLANCE=seule`)** | **`cles=157`** | **`cles=157`** |

🔴 **LA ROUGE EST VERTE, ET C'ÉTAIT ÉCRIT AVANT DE LA JOUER** (divergence E4).
Mon raisonnement tenait en trois faits : une réconciliation relit le disque
**entier** quel que soit son déclencheur ; un débordement est **lui-même** une
complétion, donc un déclencheur ; et le premier tour est `complet` par
construction. **La mesure le confirme** : en mode `seule`, le témoin apparaît par
`declencheur="notification"`.

**Ce qui achète l'absence de perte n'est donc pas la réconciliation périodique :
c'est le fait que toute réconciliation relise tout.**

**Montage B — `APPS_FAUTE=muette`, le seul qui PEUT être rouge** :

| Bras | réconciliations pendant 90 s | `cles` finales | déclencheur |
| --- | --- | --- | --- |
| VERT (période armée + `muette:1000`) | **3** | **157** | `periode` |
| **ROUGE (`seule` + `muette:1000`)** | **0** | **156** | `demarrage` |

**Deux exécutions par bras. `notifications=0` des deux côtés** — les complétions
sont bien **avalées**, donc l'injection fait ce qu'elle annonce. Et au ROUGE
**l'application n'apparaît JAMAIS**, quatre-vingt-dix secondes durant.

🔵 **C'EST LA MESURE QUI JUSTIFIE D1**, et elle n'existait pas avant ce
sous-bloc : la réconciliation périodique achète **une surveillance qui cesse de
délivrer SANS ERREUR**, et rien d'autre.

### ④ L'anti-rebond réduit le nombre de réconciliations — **TENU**

Même binaire, même corpus, même machine, même rafale (20 000 fichiers ≈ 6,2 s),
fenêtre **bornée par deux horodatages écrits** :

| Bras | exéc. 1 | exéc. 2 |
| --- | --- | --- |
| **VERT** (variable absente) | **4** | **4** |
| **ROUGE** (`APPS_SURVEILLANCE=sans-rebond`) | **60** | **58** |

**15× moins.** ⚠️ **Et ce ROUGE ne mesure PAS « anti-rebond contre RIEN »** : le
sondage a une granularité de 200 ms, qui est déjà un anti-rebond faible. Il
mesure **« anti-rebond contre 200 ms »**, et c'était écrit d'avance.

### ⑤ Le rétablissement d'une surveillance perdue — **TENU** (1 exécution, hors critères)

`APPS_FAUTE=perte:1` : **une** ligne `racine de surveillance PERDUE`, **une**
ligne `RÉTABLIE` — des **transitions**, jamais une par tentative — et une
notification postérieure déclenche encore (`declencheur="notification"`).

🔵 **ET LA CHRONOLOGIE EST LE RELEVÉ LE PLUS FORT DE TOUTE LA RECETTE, PAR
ACCIDENT DE MINUTAGE :**

```
09:23:02.5367927Z  témoin créé
09:23:02.542895Z   racine de surveillance PERDUE   (6 ms plus tard : la faute
                                                    injectée a consommé la
                                                    complétion DU TÉMOIN)
09:23:03.542659Z   racine de surveillance RÉTABLIE (1 s, repli exponentiel)
puis               cles=157  declencheur="periode"
```

**Une notification a été réellement perdue, et l'application est apparue quand
même — par la réconciliation périodique.** C'est la garantie de D1 observée sur
le chemin réel, dans un montage qui ne la cherchait pas.

### ⑥ Le témoin de repos — **TENU**, et il POUVAIT échouer (2 exécutions)

Agent armé, disque au repos, **240 s** au total :

| | exéc. 1 (120 s) | exéc. 2 (240 s cumulées) |
| --- | --- | --- |
| réconciliations | 1 `demarrage` + **4** `periode` | 1 + **8** `periode` |
| **notifications** | **0** | **0** |
| débordements | 0 | 0 |

🔵 **UNE PAR PÉRIODE, EXACTEMENT, ET ZÉRO NOTIFICATION.** Les quatre racines de
cette VM **ne sont pas bruyantes au repos** : la multiplication du travail par
`PERIODE_RECONCILIATION / DELAI_ANTI_REBOND_MAX` = **7,5** que D3 redoutait **ne
se produit pas**. ⚠️ **Le contrôle pouvait échouer** — une seule écriture de fond
aurait fait bouger `notifications`.

⚠️ **Cela mesure le repos de CETTE VM, pas celui d'un poste de travail utilisé.**

### Contrôle de sortie

**Corpus des quatre racines : 220 avant, 220 après**, à **chacune** des vingt
exécutions de sonde. `G4 Temoin.lnk`, `…\Programs\g4-rafale\` et
`C:\dev\g4-preparation\` retirés ; tâche planifiée `g4-sonde` supprimée.
**La VM a survécu** (même domaine, sans interruption) et `/dev/null` est resté un
`character special file`.

**Ce que G4 LAISSE sur la VM, et pourquoi** : les cinq `.ps1` de sonde et leurs
journaux dans `C:\dev\` — ils rendent la recette **rejouable sans rien
remonter**, comme les deux témoins de G2 sur le Bureau. Ils ne sont pas des
`.lnk` et **le catalogue ne peut pas les voir**.

---

## 7. 🔴 CE QUE G4 N'ÉTABLIT PAS — la liste a rétréci, elle n'a pas disparu

- 🔴 **AUCUN TAUX, NULLE PART.** Deux exécutions par bras au mieux ; **une** pour
  ②bis et ⑤ ; **sept** pour S1, dont **quatre** seulement dimensionnantes.
- 🔴 **QU'UN DÉBORDEMENT RÉEL SE PRODUISE JAMAIS SUR CETTE MACHINE.** S1 (7
  exécutions, jusqu'à 60 000 fichiers) et la rafale sur le produit (2 exécutions,
  ~96 000 notifications) rendent **zéro**. **Tout ce qui est mesuré du chemin de
  débordement l'est SOUS INJECTION**, et l'injection établit que le remède
  fonctionne, jamais qu'une cause existe.
- 🔴 **QUE LE CRITÈRE ③ SOIT DISCRIMINANT PAR LE MONTAGE DE SA SPÉCIFICATION.**
  Il ne l'est pas — mesuré, 2 exécutions par bras. Le montage B l'est, mais il
  **fabrique** la panne au lieu de l'observer.
- 🔴 **LE CORRECTIF E5 N'A TOUJOURS POUR PREUVE QU'UN ARGUMENT DE FLOT DE
  CONTRÔLE.** La recette ne lance **aucune installation** : le chemin
  `reconcilier` / `reconciliee` n'a pas été exercé une seule fois. **Légué, et
  déclaré manquant** — c'est la situation exacte que le défaut F1 de D7 a payée.
- 🔴 **AUCUNE CAUSE NATURELLE DE PERTE DE HANDLE N'A ÉTÉ OBSERVÉE**, ni ici ni
  ailleurs dans ce dépôt. ⑤ est entièrement sous injection.
- **`Veille::arreter` n'a toujours AUCUN APPELANT DE PRODUCTION** : le fil de
  surveillance n'a **jamais été arrêté proprement** de toute la recette — chaque
  bras se termine par un `Stop-Process -Force`. L'agent n'a aucun chemin
  d'extinction propre, ce que `CLAUDE.md` écrit depuis D1.
- **Une racine ABSENTE au démarrage n'est jamais surveillée** : le cas n'a pas
  été construit. **Latence, jamais perte.**
- **Aucune constante calibrée** : `TAMPON_NOTIFICATIONS`, `DELAI_ANTI_REBOND`,
  `DELAI_ANTI_REBOND_MAX` (**dérivée** — la mesure de ① la corrobore, elle ne la
  calibre pas), `PAS_ATTENTE_MS`, `PERIODE_RECONCILIATION` que G4 ne change pas.
  **Aucun jugement d'usage n'a été porté** sur le délai que l'utilisateur
  ressent.
- **Le témoin de repos mesure CETTE VM**, pas un poste de travail utilisé.
- **Le coût de l'envoi complet périodique sur le fil n'est mesuré par rien** —
  et la recette l'a vu partir toutes les trente secondes, sans le peser.
- **Rien de la latence de bout en bout** : G4 mesure jusqu'à une **ligne de
  journal**, jamais jusqu'à une page. Aucun sous-bloc du chantier D ni de ④ ne
  l'a jamais mesurée.
- **Un seul corpus (220 raccourcis, 156 clés), une seule VM, une seule
  application témoin.** Rien de la charge, rien de plusieurs VMs.
- **Aucun navigateur, aucune page de hub** : les quatre critères se jugent dans
  le journal de l'agent, comme D11 le prescrit.

## 8. Divergences relevées par l'exécution, en plus des neuf du plan

- **E10 — les comptes de départ du plan avaient dérivé avant que je les
  lise** : agent **924** et non 916, du fait de F4. Attribué par
  `git show | grep -c '#[test]'`.
- **E11 — l'extraction « verbatim » exige CINQ `pub(super)`.** Le plan admet
  « l'en-tête de module et les `use` » ; il en faut une de plus, et elle est
  **structurelle** : en Rust un item privé d'un module **enfant** n'est **pas**
  visible de son **parent**, et une extraction rigoureusement verbatim **ne
  compilerait pas**.
- **E12 — les tâches 8 et 9 ne peuvent pas être deux commits.** La tâche 8
  change la signature de `boucle::tourner` ; sans son appelant, elle **ne
  compile pas**. Un commit qui ne compile pas est un piège que ce dépôt a payé
  en D4.
- **E13 — `cargo clippy --workspace` porte quatre avertissements hors famille
  `dead_code`**, tous préexistants (§3).
- **E14 — `grep -n 'G4' CLAUDE.md` rend NEUF occurrences dont UNE seule
  concerne ce sous-bloc** : les huit autres nomment les gardes **G1..G7 du
  sous-projet design system**, un homonyme complet. **Une substitution globale
  les aurait abîmées.**
- **E15 — `CLAUDE.md` publie QUATRE lignes de dette pour DEUX réelles**
  (`encode.rs` 1536, `windows_source.rs` 630). `proto/src/plateforme/tests.rs`
  y figure à 561 et vaut moins ; `proto/ts/plateforme.test.ts` à 512 de même.
  **Consigné, non corrigé** — l'index appartient à la tâche de clôture, qui n'a
  pas eu lieu.

---

## 9. Ce qui reste à faire, et à qui

**G4 est complet** : produit, porte, recette, revue transverse, index.

**À G5** (qui n'a pas de plan) : le manifeste PWA par application, les
`file_handlers` alimentés par les vraies associations, les types installeur du
hub. **G4 ne l'approche pas.**

**Legs de G4 :**

1. 🔴 **Le correctif E5 n'a pour preuve qu'un argument de flot de contrôle**, et
   la recette n'a lancé aucune installation. **Le fermer demande une recette qui
   installe réellement pendant qu'une réconciliation périodique court** —
   c'est-à-dire une recette de G3 rejouée sous G4.
2. 🔴 **Aucune cause naturelle de débordement ni de perte de handle n'est
   connue.** Tout le chemin est éprouvé **sous injection**. S1 établit même que
   cette machine **ne peut pas** produire le régime : 2 850 fichiers/s contre
   les ~1 260 événements par microseconde qu'il faudrait.
3. ⛔ **Le coût de l'envoi complet périodique sur le fil n'est mesuré par rien.**
   Le seul chiffre disponible est celui de G1 — 56 145 octets pour 154
   applications, **sans** `icone` ni `source_max`. Celui d'aujourd'hui porte 156
   applications **avec** les deux.
4. ⛔ **`Veille::arreter` attend un chemin d'extinction propre** qui n'existe
   nulle part dans l'agent depuis D1.
5. ⛔ **Une racine absente au démarrage n'est jamais surveillée.**
6. ⛔ **Quatre avertissements `clippy` hors famille `dead_code`**, préexistants,
   non corrigés — la formule « tous `dead_code` » n'est plus vraie du dépôt.
7. ⛔ **`plateforme/src/http/routes-installation.ts` est à 500, MARGE NULLE** —
   legs de G3, hors périmètre de G4.
8. ⛔ **La divergence `403 vm-etrangere` / `404 vm-inconnue`** reste **signalée
   et non tranchée** : décision de sécurité, elle appartient au propriétaire du
   dépôt. **G4 ne la rencontre pas.**
