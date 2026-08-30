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

## 5. La mesure sur la VM — les TROIS bras joués

Le propriétaire a autorisé le retrait temporaire du VGA. Il est
`primary='yes'` et alimente la console VNC (`127.0.0.1:5900`), seule sortie
hors bande : pendant le retrait, **aucun filet**.

### 5.1 La condition est REPRODUITE — la cible forcée est bien là

Sonde CCD en **session 1** (tâche planifiée `/it` ; elle **imprime sa
session** plutôt que de la supposer), VM sans VGA, agent arrêté :

```
SESSION = 1
BUFFERSIZES = 0  chemins=1 modes=2
CHEMIN 0 : cible_luid=64057/0 cible_id=256 statusFlags=0x11 | source_id=0
moniteurs_pnp_ok = 0
```

🔵 **`statusFlags=0x11`** = `IN_USE | FORCED_AVAILABILITY_SYSTEM`, et **0
moniteur PnP** : c'est exactement l'état du lot 30. **Sans ce relevé, une verte
obtenue un jour où Windows n'aurait pas fabriqué de cible forcée serait
indiscernable d'une bonne.**

### 5.2 Bras C — sans VGA, désignation armée : **VERT**

```
VERDICT VERT : ouvertes=1 refus=0 tenues=1
motifs de refus : []
```

🔴 **Et surtout, QUELLE assertion passe.** Le journal, entre les marqueurs du
bras :

| Mesure | Valeur |
| --- | --- |
| sorties virtuelles créées | **1** |
| **chemin ① DÉSIGNÉE** | **1** |
| chemin ② repli | **0** |
| refus de viewport | **0** |
| trace de désarmement | **0** |

```
sortie virtuelle créée id=257 adaptateur_bas=64057 adaptateur_haut=0
sortie DESIGNEE par son identifiant de cible (chemin ① — la correspondance CCD
  a rendu son nom GDI) id_pilote=257 adaptateur=Some((64057, 0))
  nom_designe="\\.\DISPLAY5"
```

🔴 **`nom_designe="\\.\DISPLAY5"` est le nom que portait la CIBLE FORCÉE.**
Le remplacement sur la même source est donc établi, et la désignation a rendu
ce nom là où la différence d'ensembles rend le vide. **Le chemin ① a couru sur
une cible forcée — ce qu'il n'avait jamais fait.**

🔵 **L'hypothèse centrale est CONFIRMÉE, pas seulement rendue crédible.** La
sonde relancée pendant que la fenêtre était servie :

```
CHEMIN 0 : cible_luid=64057/0 cible_id=257 statusFlags=0x1 | source_id=0
moniteurs_pnp_ok = 1
  DISPLAY\SMKD1CE\1&28A6823A&0&UID257
```

`id` rendu par le pilote = **257** ; `cible_id` CCD = **257** ; UID du moniteur
= **257** ; et `statusFlags` est retombé de `0x11` à `0x1` — la cible forcée a
cédé la place. **`sudovda.rs:96` disait la disposition « non confirmée » : elle
l'est désormais pour ces trois champs.**

⚠️ **Une réserve honnête sur ma sonde, et elle est à MA charge.** Sa colonne
`nom_gdi` rend `(GetDeviceInfo=31)` — `ERROR_GEN_FAILURE`. **Ce n'est pas
l'API qui échoue : c'est le marshalling C# de ma sonde PowerShell.**
L'implémentation Rust du produit, elle, a rendu `\\.\DISPLAY5` sur le même
chemin, au même instant. La sonde établit les identifiants et les
`statusFlags` ; **elle n'établit pas le nom, et le produit s'en charge.**

### 5.3 Bras B — sans VGA, `SORTIE_DESIGNEE=0` : **ROUGE**

```
VERDICT ROUGE : ouvertes=8 refus=10 tenues=0
motifs de refus : ["aucune sortie d'affichage ne peut servir cette fenêtre",
                   "la session n'a pas tenu après 3 tentatives"]
```

🔴 **La rouge rougit pour la BONNE raison** — le motif est mot pour mot celui
du défaut, lu et non déduit d'un code de sortie.

| Mesure | Valeur |
| --- | --- |
| sorties virtuelles **créées** | **8** |
| chemin ① DÉSIGNÉE | **0** |
| chemin ② repli | **0** |
| refus de viewport | **8** |
| trace de désarmement | **1** |

```
ERROR … aucune sortie candidate ne peut servir ce viewport — elle est rendue
  au pilote session=…:w-1 demande="1860x1080" designee="" candidates=[]
```

🔵 **Le couple juge fait son travail** : **8 créées** — le mécanisme tourne,
le bras n'est pas disqualifié — et **0 tenue**. Mécanisme présent, résultat
absent, patron de `PONT_MUTATION`.

### 5.4 🔴 L'INCIDENT — une rouge vacueuse, attrapée par la trace et non par le fichier

**La première tentative du bras B n'a rien mesuré.** Mon script de cycle a
échoué à poser la variable (un `-replace` à travers trois couches de
guillemets), puis, corrigé, l'a posée **à la fin** de
`C:\nivuus\agent\run-agent.ps1` — c'est-à-dire **après** la ligne qui lance
l'agent (`:61`), donc **jamais exécutée**.

**Les deux fois, le fichier contenait la ligne. Les deux fois, la valeur
n'atteignait pas le processus.** Ce qui l'a dit est le contrôle prescrit par
`CLAUDE.md` : **`TRACE DE DESARMEMENT : 0`** dans le journal. Un tracé de code
aurait conclu l'inverse — c'est le piège payé en D1, D2 et D7, **revécu ici
pour la quatrième fois**, et attrapé.

Après pose **avant** le lancement (ligne 35, contre 61 pour l'invocation) :
`TRACE DE DESARMEMENT : 1`, `WARN agent::superviseur::designation`.

🔵 **Effet de bord : le `warn!` est désormais ÉTABLI.** La ronde précédente le
déclarait non vérifiable sur l'hôte ; il a été vu sortir.

### 5.5 Ce que la VM est devenue, et la restauration

| Ce qui a été touché | État final | Preuve |
| --- | --- | --- |
| Définition libvirt | **le VGA est revenu**, restauré **depuis la copie nommée** | `diff` : **2 lignes**, l'`id` de domaine (34→36) et le tap (`vnet34`→`vnet36`), **toutes deux éphémères**, réattribuées par libvirt à chaque démarrage |
| Console VNC | **rétablie** | un port `5900` en écoute |
| `agent.exe` | **l'original**, depuis sa copie nommée | sha256 `7DB1C0FA…74C3` |
| `SORTIE_DESIGNEE` | **retirée** du script qui lance | 0 occurrence relue |
| Tâche `lot32-sonde` | **supprimée** | `schtasks /delete` |
| Agent | **3 processus vivants** | `Get-Process agent` |
| VM | **en exécution, WinRM répond** (~72 s après `virsh start`) | `virsh list --all` + `/dev/tcp/…/5985` |
| Extinctions hôte pendant la séquence | **0** | `journalctl -u libvirtd`, `terminating on signal 15` |

⚠️ **Le binaire de production est celui d'avant**, à dessein : il doit venir du
package (`hooks/activate.py`, `chemin_agent_console`), jamais d'une
fabrication de travail.

🔴 **LE BINAIRE DE TRAVAIL DU LOT 32 A ÉTÉ RETIRÉ DE LA VM** (ainsi que la
sonde et sa sortie). Il n'avait aucun usage futur — le remède se refabrique en
une commande depuis les sources commitées — et un `agent.exe.lot32` posé à côté
d'`agent.exe` est exactement ce qu'on copie par inadvertance. **La copie nommée
de l'ORIGINAL est conservée** : c'est elle qui atteste ce que la production
faisait tourner.

⚠️ **LE DÉPLOIEMENT N'EST PAS ORDONNANCÉ PAR CE LOT, ET C'EST DÉLIBÉRÉ.** Le
lot voisin réécrit l'encodeur au même moment ; déployer ce remède seul
obligerait à redéployer dans quelques heures, et **deux déploiements sont deux
occasions de se tromper pour un seul gain**. Une seule refabrication par
`scripts/build-agent-croise.sh` quand les deux remèdes seront là.

### 5.6 Un défaut de câblage, TROUVÉ et NON corrigé

**`SORTIE_DESIGNEE` n'atteint pas l'agent de l'appliance.** La tâche 3 l'a
posée dans `scripts/run-agent.sh`, ce qu'exige la règle du dépôt — mais ce
script vise la VM de développement qui n'existe plus sous cette forme. L'agent
réel est lancé par `C:\nivuus\agent\run-agent.ps1`, un fichier du package
`console`. La recette l'y a injectée à la main. **Corriger cela touche un autre
package : hors périmètre, la décision appartient au propriétaire.**

---

## 6. Ce que ce lot établit, et ce qu'il n'établit pas

**Établi :**
- 🔴 **le remède ferme le défaut** — même VM, même viewport, même instrument,
  seule la variable de banc diffère : **1 fenêtre tenue contre 0**, **0 refus
  contre 8** ;
- 🔴 **le chemin ① a couru sur une cible forcée** et a rendu le nom que la
  différence d'ensembles ne peut pas voir ;
- 🔴 **l'hypothèse de `sudovda.rs` est confirmée** pour `identifiant_cible` et
  le LUID : id pilote = cible CCD = UID du moniteur = 257 ;
- la condition du lot 30 est **reproduite et mesurée** (`statusFlags=0x11`,
  0 moniteur PnP), donc la verte ne verdit pas pour la mauvaise raison ;
- le `warn!` de désarmement **sort** ;
- **non-régression** avec VGA (bras A).

**NON établi, et il faut le dire :**
- **qu'aucun média ne traverse n'a été vérifié** : l'instrument n'a aucune pile
  WebRTC et n'émet aucune offre SDP. Il éprouve la chaîne jusqu'à la sortie
  virtuelle, pas l'image ;
- **le comportement au-delà d'une fenêtre sans VGA** : le bras C n'en a servi
  qu'UNE. Le lot 30 avait mesuré 4 créées → 4 attachées, mais **par la sonde**,
  pas par le produit ;
- 🔴 **le bras ROUGE de ce remède n'est PAS rejouable sur l'agent réel de
  l'appliance** : `SORTIE_DESIGNEE` n'atteint que l'agent lancé par
  `scripts/run-agent.sh`, pas celui de `C:\nivuus\agent\run-agent.ps1` (§ 5.6).
  La recette a dû l'y injecter à la main. **Legs inscrit au § « Legs ouverts »
  de `CLAUDE.md`**, sous `package-nivuus` ;
- **la voie A reste non éprouvée**, et n'a plus lieu d'être : ⚠️ **ne pas la
  livrer avec C**, l'amorce masquerait le défaut et rendrait cette recette
  injouable ;
- **`montee.rs` verrait-elle le remplacement** : prédiction de lecture, jamais
  jouée ;
- ⚠️ **trois chaînes de trace portaient des suites d'espaces** (mes heredocs
  Python ayant mangé les continuations `\` de Rust). **Corrigées après la
  mesure** ; les relevés ci-dessus montrent la forme espacée. Le `grep` des
  recettes porte sur le préfixe et n'est pas affecté.

---

## 7. Ce qu'il reste à faire

1. **Déployer le remède**, en **une** refabrication commune avec le lot voisin
   (encodeur) — ordonnancé hors de ce lot. Le binaire d'origine est en place.
2. **Décider du sort de `SORTIE_DESIGNEE` dans `console`** (§ 5.6) —
   **inscrit au § « Legs ouverts » de `CLAUDE.md`**, sous le chantier
   `package-nivuus`, parce qu'un legs qui ne vit que dans un relevé daté est
   un legs perdu.
3. **Mesurer plusieurs fenêtres sans VGA** — le bras C n'en a servi qu'une.

---

## 8. Enquête — « une sortie créée que Windows ne nomme jamais » (lot 31)

Le lot 31 a buté sur ce chemin en montant sa recette d'encodeur : sortie créée
à **780×492**, jamais nommée, `designee=""` **et** `candidates=[]`. Trois
questions m'ont été posées dans l'ordre ; voici les réponses **mesurées**.

### 8.0 🔴 Deux pièges de MÉTHODE, payés avant la première conclusion

**① J'ai failli lire MON PROPRE échec en croyant lire celui du lot 31.**
`C:\nivuus\agent.log` porte 52 000 lignes ; sa dernière occurrence de « la
cible n'a jamais été nommée » est datée `16:26`, avec `demande="1860x1080"` et
les sessions `w-6`…`w-8` — **c'est mon bras B**, pas le lot 31. Le lot 31
écrivait dans un journal séparé, `C:\nivuus\lot31\agent-lot31.log`, et
**`agent.log` ne contient AUCUN `780x492`**. C'est le piège de `CLAUDE.md`
(« un agent survivant tient `agent.log` ») sous une forme neuve : **deux
journaux distincts, et le plus récent des deux échecs était le mien.**
⚠️ **Ce qui a tranché est une donnée du relevé lui-même** — le viewport et les
numéros de session —, pas la date.

**② UN MARQUEUR AJOUTÉ PAR `Add-Content` DANS UN JOURNAL TENU PAR UN AGENT
VIVANT EST SILENCIEUSEMENT PERDU.** `Add-Content` a rendu la main sans erreur,
et le marqueur n'existait pas : l'agent tient le fichier par un `StreamWriter`
qui écrit à SA position et recouvre ce qu'un autre processus a ajouté. Mes
bras précédents marchaient parce que l'agent était **arrêté** quand je posais
le marqueur. **Segmenter par HORODATAGE quand l'agent tourne**, jamais par
marqueur. Détecté parce que le relevé rendait 52 282 lignes « du bras » —
c'est-à-dire tout le journal.

### 8.1 ① Régression de la voie C, ou défaut distinct ? — **DÉFAUT DISTINCT**

Trois pièces, dont deux mesurées après coup :

- 🔵 **Le produit d'AVANT la voie C sert 780×492 sans broncher.** Le binaire de
  production est resté celui d'hier — vérifié par ses chaînes : il porte
  `aucune sortie apparue ne peut servir` (l'ancienne) et **pas**
  `aucune sortie candidate` (la neuve). Sur lui, viewport `780x492` :
  **`ouvertes=2 refus=0 tenues=1`**, deux sorties créées, attachées, servies
  sur `DISPLAY6` et `DISPLAY7`.
- 🔵 **L'A/B explicite, sur MON binaire, au même viewport** :

  | Bras | créées | chemin ① | chemin ② | refus | verdict |
  | --- | --- | --- | --- | --- | --- |
  | désignation **armée** | 5 | **5** | 0 | **0** | VERT |
  | `SORTIE_DESIGNEE=0` | 6 | 0 | **6** | **0** | VERT |

  Les deux chemins servent. **La voie C ne fait aucune différence ici**, et le
  bras désarmé — qui *est* le produit d'hier — ne rougit pas davantage.
- 🔵 **Le relevé du lot 31 le disait déjà** : `candidates=[]` signifie que le
  **repli** ne trouvait rien non plus. Et la topologie à l'expiration porte
  `nombre=1` — le **total** des sorties DXGI, attachées ou non —, inchangé
  avant et après la création. **La sortie n'entrait pas du tout dans la
  topologie** : aucune règle d'appariement, ni l'ancienne ni la nouvelle, ne
  pouvait la voir.

**Verdict : la voie C est hors de cause, et ce n'est pas un défaut de `desk`.**

### 8.2 ② Ce qui différait — **Apollo, et son `ensure_only_display`**

> 🔴 **ANNOTATION DU 30 AOÛT 2026 (lot 32C) — CE TITRE EST INCOMPLET, ET LA
> MESURE L'A MONTRÉ.** Imputer l'échec au seul réglage `ensure_only_display`
> était trop étroit : avec `dd_configuration_option = ensure_active` **et
> aucun client connecté**, l'interférence PERSISTE — Apollo `Running` rend
> **7 refus / 0 fenêtre tenue**, Apollo `Stopped` rend **0 refus / 4 tenues**,
> même binaire à deux minutes d'intervalle. La cause suffisante est la **sonde
> d'encodeur** qu'Apollo relance à **chaque changement de topologie**, à la
> cadence de 5 s de `LIMITE_RATTACHEMENT`. Voir
> [`2026-08-30-juge-image-au-navigateur-resultats.md`](2026-08-30-juge-image-au-navigateur-resultats.md)
> § 6.2. **Ce qui suit reste exact sur les FAITS relevés ; c'est la portée de
> la conclusion qui était trop large.**

Ce n'est **pas** la taille (§ 8.1), et ce n'est pas la limite d'attente : une
sortie qui n'apparaît jamais dans la topologie n'apparaîtra pas davantage en
attendant plus longtemps. La différence est **environnementale**, et elle est
nommée dans la configuration de la VM :

```
C:\Program Files\Apollo\config\sunshine.conf
  # Deactivate every other display and stream the virtual one alone.
  dd_configuration_option = ensure_only_display
```

et dans le journal d'Apollo :

```
[19:42:38] Info: config: 'dd_configuration_option' = ensure_only_display
[19:42:48] Info: Creating a temporary virtual display to probe for encoders...
```

🔴 **Apollo crée ses propres sorties SudoVDA et DÉSACTIVE toutes les autres.**
Le lot 31 mesurait précisément Apollo ; sa topologie portait une `\\.\DISPLAY6`
préexistante à **2410×1080** — ni la nôtre (la purge inter-processus du
démarrage a balayé les seize GUID du gabarit et **n'a rien retiré**), ni un
écran physique. C'est la sortie d'Apollo.

🔵 **`montee.rs` connaissait déjà ce comportement et le nomme** : son garde
`disparues`/`parues` a été écrit contre « le remplacement qu'Apollo faisait par
son réglage `ensure_only_display` ». **La sonde de banc voyait ce que le
produit subit.**

⚠️ **Ce que je n'ai PAS reproduit** : je n'ai pas rejoué une session Apollo
active pour observer le retrait en direct. Apollo tournait pendant mes trois
tests verts (`ApolloService Running`, `sunshine` pid 7372) **sans session**, et
n'a rien gêné. **L'explication repose donc sur la configuration, le journal
d'Apollo et la topologie du lot 31 — pas sur une reproduction.** Elle est dite
comme telle.

### 8.3 ③ Le registre — **ce n'est pas mon sillage**

Le registre porte bien de nombreuses clés `…mesureN…` sous
`GraphicsDrivers\Configuration`, héritées de toutes les campagnes. **Mais le
symptôme de cette pollution, établi par le lot 22, est une sortie qui PARAÎT à
la mauvaise taille** — ici elle ne paraît **pas du tout**. Et le produit
d'hier, sur ce même registre, sert `780×492` correctement (§ 8.1). **La
pollution du registre n'explique pas cet échec**, et ma campagne n'est pas en
cause.

⚠️ `2410×1080` figure dans la liste des tailles polluées relevées par le lot 22
— **c'est une coïncidence de valeur, pas une preuve** : ici cette taille est
celle de la sortie d'Apollo, vivante et attachée, pas d'une sortie née à une
mauvaise taille.

### 8.4 Ce que cette enquête N'établit PAS

- **qu'Apollo est bien la cause**, au sens d'une reproduction : je n'ai pas
  rejoué une session Apollo active (§ 8.2). La chaîne d'indices est forte et
  concordante ; ce n'est pas une mesure du mécanisme ;
- **que le produit devrait s'en défendre.** Deux clients qui pilotent le même
  pilote d'affichage avec des politiques opposées est un conflit de
  configuration, pas un défaut de code — **la décision appartient au
  propriétaire** (couper `ensure_only_display`, ou ne pas faire tourner les
  deux ensemble) ;
- **rien sur la limite de 5 s** : elle n'a pas été mise en cause, donc pas
  éprouvée. Si un cas d'attente légitime apparaît un jour, il reste à mesurer.

### 8.5 L'état dans lequel la VM est rendue

| Ce qui a été touché | État final | Preuve |
| --- | --- | --- |
| `agent.exe` | **l'original**, depuis la copie nommée | sha256 `7DB1C0FA…74C3` |
| `run-agent.ps1` | `SORTIE_DESIGNEE` retirée | 0 occurrence relue |
| Définition libvirt | **jamais touchée** — le retrait du VGA n'a pas été reconduit | VGA présent au `dumpxml` |
| Agent | 3 processus vivants | `Get-Process agent` |
| VM | en exécution | `virsh list --all` |

**Aucune sortie virtuelle laissée** : les trois tests se sont terminés par des
sessions servies puis fermées, et le superviseur purge au démarrage
(`superviseur.rs:72`).
