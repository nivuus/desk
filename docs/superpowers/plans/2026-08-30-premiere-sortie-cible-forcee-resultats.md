# Lot 32 — la première sortie virtuelle et la cible forcée : le remède, et la mesure à moitié jouée

Date : 30 août 2026. Branche `package-nivuus`. Dépôt `packages/desk`.

Commits : `054fcf6` (tâche 1, extraction), `b55cf8b` (tâche 2, voie C),
`bf187ff` (tâche 3, `SORTIE_DESIGNEE`).

---

## 0. Ce qui est acquis, et ce que ce lot y ajoute

Du lot 30, sans le remesurer : sans le VGA QEMU, la session 1 démarre avec
**0 moniteur PnP mais 1 écran** (`\\.\DISPLAY5`), cible active à
`statusFlags=0x11` (`IN_USE | FORCED_AVAILABILITY_SYSTEM`) — une **cible
forcée**. Le premier moniteur virtuel **REMPLACE cette cible sur la MÊME
source**, hérite du même nom GDI, n'« apparaît » donc jamais ; le produit
n'appariant que parmi les sorties APPARUES, il refusait la fenêtre et rendait
la sortie au pilote. **4 créées → 4 attachées** : ce n'est ni une limite de
Windows ni du pilote, l'hypothèse « registre pollué » est **RÉFUTÉE**.

**Ce lot livre le remède (voie C), le fige par cinq tests d'hôte vus rouges,
et ne joue qu'UN des trois bras de la recette VM.** Les deux autres exigent de
retirer le VGA, c'est-à-dire de modifier la définition libvirt — **refusé par
le garde de permission de l'environnement** (§ 6). Ce n'est pas une limite de
conception : c'est une autorisation à donner.

---

## 1. Trois corrections au cadrage, dont deux au brief lui-même

1. 🔴 **Le prédicat fautif n'est pas `creation_sortie.rs:102`.** Il est dans la
   boucle de scrutation : `!avant.contains(&s.nom_sortie)`. `:102` ne faisait
   que consommer un vecteur déjà vidé. Une rédaction posée à `:102` n'aurait
   rien corrigé.
2. 🔴 **`agent/src/superviseur/boucle/montee.rs` n'existe pas.** Le seul
   `montee.rs` est `agent/src/diagnostics/multifenetre/montee.rs` — une **sonde
   de banc**. Sa règle (`parues.len() != 1 || !disparues.is_empty()`) est
   **correcte pour ce qu'elle mesure** : dénoncer un remplacement, ce
   qu'Apollo fait par `ensure_only_display`. **Elle n'est pas touchée** — la
   corriger aurait fait taire l'instrument qui a trouvé le défaut.
3. 🔴 **La cause première était une affirmation FAUSSE**, écrite en D1 dans
   `superviseur/placement.rs` et jamais revérifiée :

   > Le premier rend un identifiant de cible qui lui appartient, le second
   > énumère par `(index_adaptateur, index_sortie)`. **Aucune correspondance
   > n'est exposée** : l'appariement se fait donc par dimensions et par
   > élimination.

   Les deux faits sont exacts, **la conclusion ne l'est pas**. C'est d'elle que
   sortait l'appariement par différence d'ensembles.

---

## 2. La voie C — ni la série EDID, ni une différence d'ensembles

La question du brief était : le nom GDI est-il rattachable à la **série
EDID** ? Par l'EDID, non — il faudrait registre + décodage, trois
indirections. **Mais la voie C est ouverte par mieux : `identifiant_cible`
EST déjà l'identifiant de cible du système d'affichage.**

`sudovda::SortieAjoutee` rend **trois** nombres —
`(adaptateur_bas: u32, adaptateur_haut: i32, identifiant_cible: u32)` —,
exactement la paire `(adapterId, id)` d'un `DISPLAYCONFIG_PATH_TARGET_INFO`.
**Le produit n'en gardait qu'un** : le LUID était journalisé, puis jeté.

Le chaînage : `QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS)` → le chemin dont la
cible porte notre couple → `DisplayConfigGetDeviceInfo(GET_SOURCE_NAME)` →
`viewGdiDeviceName`, littéralement `\\.\DISPLAYn` — le même nom que
`DXGI_OUTPUT_DESC.DeviceName`, d'où `SortieDxgi::nom_sortie` est peuplé.

**Dans le cas fautif, le chemin actif devient `{ source S, target T_nôtre }`,
et le chaînage rend `S → \\.\DISPLAY5`** : le nom même que la différence
d'ensembles écarte. La voie C est **structurellement insensible** au
remplacement d'une cible forcée, et à toute course.

### Ce qu'elle suppose, et pourquoi l'hypothèse est sans risque

Que `identifiant_cible` soit un `id` de cible CCD sur l'adaptateur rendu.
`sudovda.rs` dit lui-même la disposition « non confirmée ».

🔵 **Deux pièces la rendent crédible, aucune ne l'établit.**
- Le lot 22 a compté dix moniteurs fantômes `DISPLAY\SMKD1CE\…UID256` à
  `UID265` en notant que « les identifiants du pilote (256…265) tournent en
  rond ». Le suffixe `UIDnnnn` **est** l'`id` de cible CCD.
- **Mesuré par ce lot** (bras A, § 5) : `sortie virtuelle créée id=257
  adaptateur_bas=66464 adaptateur_haut=0`. **Le pilote rend bien un LUID non
  nul**, ce que rien n'avait vérifié — il n'était jusqu'ici que journalisé.

🔵 **Si l'hypothèse est fausse, le coût est NUL** : la recherche ne trouve
rien, la règle rend `None`, le produit retombe **exactement** sur le
comportement d'hier. C'est cette propriété — et elle seule — qui a rendu le
remède livrable avant d'être mesuré sur la VM, et c'est pourquoi **le repli
n'est pas retiré**.

---

## 3. Ce qui a été livré

### Tâche 1 — l'extraction, dans sa propre tâche, AVANT celle qui ajoute (`054fcf6`)

`EtatSorties` quitte `pilote.rs` pour `pilote/etat.rs` : **463 → 440**. Sans
elle, la tâche 2 laissait ~480, la marge que ce dépôt a mesuré six fois se
reperdre. Aucun comportement ne change. Les deux pièges nommés par `CLAUDE.md`
ont été traités plutôt que découverts au compilateur : les **visibilités**
(champs `pub(super)`) et l'**import laissé derrière** (`Numeros`, dont un
`unused_imports` n'est pas de la famille `dead_code`).

### Tâche 2 — la voie C (`b55cf8b`)

| Fichier | Ce qu'il devient |
| --- | --- |
| `agent/Cargo.toml` | + `"Win32_Devices_Display"` |
| `moniteurs_virtuels.rs` | + `pub type Adaptateur = (u32, i32)` |
| `moniteurs_virtuels/config_affichage.rs` **(neuf, 254 l.)** | la règle CCD, **pure** + `mod win`. Une paire ambiguë **refuse de trancher** |
| `moniteurs_virtuels/pilote.rs` | `apparies` retient l'adaptateur ; `adaptateur_de` |
| `superviseur/designation.rs` **(neuf, 172 l.)** | la **sélection**, rendue pure donc éprouvable sans Windows |
| `superviseur/boucle/creation_sortie.rs` | ① DÉSIGNER, puis ② le REPLI d'hier mot pour mot |
| `superviseur/placement.rs` | l'affirmation fausse **corrigée, pas supprimée** |

🔴 **CE QUI N'EST PAS RELÂCHÉ.** `sortie_pour_viewport` n'est pas touchée :
attachée, assez grande, pas déjà prise continuent de courir sur la sortie
désignée. La désignation **RESSERRE** (d'« tout ce qui est apparu » à un
singleton), elle ne desserre aucun garde. **Un moniteur physique ne peut pas
être désigné**, faute de porter notre identifiant de cible. C'est la
différence de nature avec la voie B, qui aurait, elle, rendu possible qu'une
fenêtre soit posée sur l'écran réel de la VM.

**L'affirmation fausse est corrigée avec les commandes qui l'établissent**,
pour que le prochain lecteur refasse le contrôle sans croire personne — règle
du dépôt sur les affirmations corrigées. **Cherchée par le sens dans tout
`agent/src/` : aucune autre copie.**

**L'énumération des causes du refus est REMESURÉE** : elle disait « deux »
pendant une branche, puis « trois » ; elles sont **cinq**, en deux familles, et
le champ `designee` du journal dit laquelle s'applique.

### Tâche 3 — `SORTIE_DESIGNEE=0`, tâche dédiée (`bf187ff`)

**J'avais soutenu qu'elle était superflue** (« le bras rouge est le binaire
d'avant »). C'est faux, et le coordinateur a raison : dans trois semaines ce
binaire n'existe plus. Convention `=0` DÉSARME, jamais `is_ok()`. Prédicat
**RÉUTILISÉ, pas recopié** (`crate::apps::desarme`). Lue par `OnceLock`,
**forcée au démarrage de `boucle::tourner`** — sinon la trace ne sortirait
qu'à la première fenêtre (leçon de `PONT_MESURE`, F4). Entrée ajoutée au
tableau de `CLAUDE.md`.

---

## 4. Les contrôles d'hôte, et les rouges

### 4.1 Les tests

`cargo test --workspace` : **1061 → 1071** (+10), 0 échec.
`cargo check --target x86_64-pc-windows-gnu` : compile, **24 avertissements,
tous de la famille `dead_code`** — la NATURE vérifiée, jamais le nombre ;
aucun `unused_imports`, aucun `private_interfaces`.

### 4.2 🔴 Les trois gardes VUS ROUGES

Par mutation ciblée, ancrée sur la **syntaxe** et vérifiée unique
(`assert count == 1`), restauration depuis une **COPIE NOMMÉE** — jamais
`git checkout --`, qui restaure HEAD et a déjà effacé du travail ici.

| Mutation | Ce qui rougit | Assertion lue |
| --- | --- | --- |
| chemin ① retiré (= le produit d'avant) | `la_sortie_qui_remplace_une_cible_forcee_est_retenue` | `left: 0, right: 1` |
| attachement retiré du chemin ① | `une_sortie_detachee_n_est_pas_designee` | — |
| `!deja_prises` retiré de `placement.rs` | `la_designation_ne_court_circuite_pas_le_filtre_des_prises` **+ les 4 tests préexistants de `placement`** | — |

🔵 **La rouge est lue, jamais son seul code de sortie** : `left: 0, right: 1`
est bien l'**ensemble vide** du produit d'avant, pas une autre panne.

🔵 **Le TÉMOIN NÉGATIF reste VERT sous la première mutation**, comme il le
doit : `le_produit_d_avant_le_lot_32_rend_un_ensemble_vide_sur_ce_meme_montage`
affirme le vide avec `None`, que la mutation ne change pas. C'est lui qui rend
la rouge interprétable — il montre, dans le même relevé, que le montage est
celui qui échouait.

**Les trois restaurations ont été vérifiées par `cmp` / `git diff` contre la
copie nommée**, pas supposées.

### 4.3 Le binaire — 🔴 la taille ne prouve rien, dans les deux sens

Chaînes cherchées dans l'`agent.exe` bâti (20 459 127 o, contre 20 465 604 pour
celui de production — **un écart qui ne prouve rien**) :

| Chaîne | Attendu | Mesuré |
| --- | --- | --- |
| `designation de sortie DESARMEE` | présente | **1** |
| `aucune sortie candidate ne peut servir` | présente | **1** |
| `la cible n'a jamais été nommée` | présente | **1** |
| `SORTIE_DESIGNEE` | présente | **1** |
| **TÉMOIN NÉGATIF** — `aucune sortie apparue ne peut servir` (la formule d'AVANT, remplacée) | **absente** | **0** |
| **CHAÎNE PRÉEXISTANTE** — `sortie virtuelle rendue au pilote` | présente | **1** |

Sans les deux dernières lignes, les quatre premières ne diraient rien : un
`grep` en panne rendrait des zéros partout, et un binaire inchangé rendrait
des uns partout.

### 4.4 🔴 La ligne lue dans le `run-agent.ps1` GÉNÉRÉ, jamais dans le script hôte

Le heredoc de `scripts/run-agent.sh` a été extrait (bornes relevées par
`grep -n`, **jamais recopiées**) et expansé par le **même code**, sans toucher
la VM :

| Bras | Attendu | Mesuré |
| --- | --- | --- |
| `SORTIE_DESIGNEE=0` | la ligne | `$env:SORTIE_DESIGNEE = '0'` |
| variable absente (armé par défaut) | rien | **0** |
| **TÉMOIN NÉGATIF** `SORTIE_DESIGNEE_INEXISTANTE=0` | rien | **0** |
| **TÉMOIN POSITIF** `PLEIN_ECRAN=0` | la ligne | **1** |

Sans le témoin positif, les deux zéros seraient rendus à l'identique par une
génération entièrement en panne.

---

## 5. La mesure sur la VM — un bras sur trois

### 5.1 Le protocole, et le chiffre-juge

Trois bras, tous avec le binaire du lot 32, pilotés par
`journaux-lot22/instrument/pilote-parcours.mjs` — **réutilisé, pas réécrit** :
il relève déjà « ouverte **et non refusée** » et le **motif** des refus.

| Bras | Condition | Attendu |
| --- | --- | --- |
| **A** témoin | **avec** VGA, désignation armée | VERT |
| **B** rouge | **sans** VGA, `SORTIE_DESIGNEE=0` | ROUGE |
| **C** vert | **sans** VGA, désignation armée | VERT |

🔴 **Le chiffre-juge est un COUPLE, parce que 1 contre 0 est un écart d'une
unité.** `créées` (sorties virtuelles créées, relevé dans `agent.log`) doit
être **≥ 1 dans tout bras**, sinon le bras est **DISQUALIFIÉ** — un zéro de
fenêtres rendu par un agent qui n'a jamais démarré serait indiscernable du
défaut. C'est le patron « mécanisme présent, résultat absent » de
`PONT_MUTATION`.

### 5.2 Bras A — JOUÉ, **VERT**

```
VERDICT VERT : ouvertes=3 refus=0 tenues=1
motifs de refus : []
```

Relevé : `docs/superpowers/plans/journaux-lot32/bras-A-avec-vga-armee.json`.
Côté agent, entre les marqueurs du bras : **145 lignes**, **3 sorties
virtuelles créées**, **0** refus `aucune sortie candidate ne peut servir`,
**0** trace de désarmement (la désignation était bien armée), et deux
duplications ouvertes sur `\\.\DISPLAY6` et `\\.\DISPLAY7`,
`adaptateur=NVIDIA GeForce RTX 4070`, `attachee=true`.

**Ce que le bras A établit** : que le binaire du lot 32 **ne régresse pas** sur
la condition nominale, et que le montage entier (plateforme, route de
lancement, pilote, superviseur, pilote d'affichage) fonctionne — donc qu'un
zéro dans un autre bras serait imputable au bras, pas au montage.

**Ce qu'il n'établit PAS** : rien sur le défaut. Le défaut n'existe que **sans**
le VGA.

### 5.3 Bras B et C — **NON JOUÉS**, et pourquoi

Ils exigent de retirer le VGA, c'est-à-dire d'écrire une définition libvirt
modifiée puis de la redéfinir. **Trois tentatives ont été refusées par le
garde de permission de l'environnement** — y compris la simple écriture du
fichier XML modifié dans `/var/tmp`. Le refus est cohérent et vise cette
classe d'action.

🔴 **Je me suis arrêté plutôt que de chercher un contournement.** C'est une
autorisation à donner, pas un obstacle technique : le reste est prêt.

### 5.4 🔴 Un défaut de câblage TROUVÉ en préparant la mesure, et NON corrigé

**`SORTIE_DESIGNEE` n'atteint pas l'agent de l'appliance.** La tâche 3 a posé
la ligne dans `scripts/run-agent.sh`, ce qu'exige la règle du dépôt — mais ce
script vise la VM de développement **qui n'existe plus sous cette forme**
(réserve de `CLAUDE.md`, sort « non tranché »). L'agent réel est lancé par la
tâche planifiée `guacamole-agent`, qui exécute
**`C:\nivuus\agent\run-agent.ps1`**, un fichier de `console` qui ne pose que
`SIGNALING_URL`, `LOCAL_IP`, `RUST_LOG`, `AGENT_VM`, `AGENT_SECRET` et
`SUPERVISEUR`.

**Le contrôle qui vaut l'a montré, et le tracé de code ne l'aurait pas fait** —
c'est exactement le piège payé en D1, D2 et D7. Mon script de cycle injecte
donc la ligne **dans le script qui lance réellement**, et le bras A a été
vérifié à **0 occurrence** (donc armé).

⚠️ **Ce n'est pas corrigé** : ajouter la variable au `run-agent.ps1` de
`console` est une modification d'un **autre package**, et elle appartient au
propriétaire.

### 5.5 L'état dans lequel la VM est rendue

| Ce qui a été touché | État final | Preuve |
| --- | --- | --- |
| Définition libvirt | **intacte, VGA compris** | `diff` contre la copie nommée : **aucune différence** |
| `agent.exe` | **l'original restauré** | sha256 `7DB1C0FA…74C3`, identique à l'empreinte relevée avant |
| `run-agent.ps1` (VM) | **0** occurrence de `SORTIE_DESIGNEE` | relu après restauration |
| Agent | **3 processus vivants** | `Get-Process agent` |
| VM | **en cours d'exécution** | `virsh list --all` après la séquence |
| Serveur HTTP de dépôt | arrêté **par PID relevé** | jamais `pkill -f` |

Le binaire du lot 32 reste sur la VM sous
`C:\nivuus\agent\agent.exe.lot32` : **un re-essai est une copie, pas une
refabrication.** La copie nommée de l'original reste à côté
(`agent.exe.copie-nommee-avant-lot32`).

⚠️ **Décision prise, et offerte au propriétaire** : j'ai remis le binaire
d'origine. Le lot 32 n'a validé que la non-régression (bras A) ; laisser la
production sur un binaire dont le bras qui compte n'a pas été joué n'est pas à
moi de le décider.

---

## 6. Ce que ce lot n'établit PAS

- 🔴 **Que le remède ferme le défaut.** Bras B et C non joués. Le bras A ne
  montre qu'une non-régression, dans la condition où le défaut **n'existe
  pas**.
- 🔴 **Que `identifiant_cible` soit un `id` de cible CCD.** Deux pièces
  concordantes (§ 2), aucune preuve. Le chemin ① n'a **jamais couru sur une
  cible forcée**.
- 🔴 **Que le `warn!` de `SORTIE_DESIGNEE=0` sorte.** Son `OnceLock` ne peut
  pas être réinitialisé entre deux tests, et un test qui poserait la variable
  d'environnement empoisonnerait ses voisins : c'est le **prédicat** qui est
  figé. La sortie de la trace se vérifiera au bras B.
- **Que la voie A soit superflue en toutes circonstances.** Elle l'est *si C
  tient* ; elle redevient le repli si les bras B/C réfutent C. ⚠️ **Ne pas
  livrer A et C ensemble** : l'amorce masquerait le défaut que C corrige, et
  la recette de C deviendrait injouable.
- **Que `montee.rs` verrait le remplacement.** Prédiction de lecture ; cette
  sonde n'a jamais été jouée sans le VGA.

---

## 7. Ce qu'il reste à faire, dans l'ordre

1. **Autoriser la modification de la définition libvirt**, puis jouer les bras
   B et C. Tout est prêt : `/var/tmp/lot32-cycle.sh` (arrêt par PID, purge
   `MULTIFENETRE_VDD_PURGE=1` entre deux exécutions, marqueur de bras dans un
   journal de 15 Mio, pose/retrait de la variable dans le script qui lance
   réellement), le binaire déjà sur la VM, la copie nommée de la définition
   dans ce répertoire, et l'instrument du lot 22.
2. **Décider du sort de `SORTIE_DESIGNEE` dans `console`** (§ 5.4).
3. **Décider du binaire de production** : l'original est en place.
