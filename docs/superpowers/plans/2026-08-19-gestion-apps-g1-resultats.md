# Sous-bloc G1 — le catalogue naît, et on peut lancer ce qu'il contient : résultats

**Date de la recette :** 20 août 2026.
**Plan :** `docs/superpowers/plans/2026-08-19-gestion-apps-g1.md` (commit `d11c3c1`).
**Spécification :** `docs/superpowers/specs/2026-08-19-gestion-apps-design.md` (commit `dded5b5`).
**Journaux :** `docs/superpowers/plans/journaux-gestion-apps/` — **UTF-8, séquences
ANSI déjà retirées** (tous les `*-plat.log`) : ils se `grep`ent à plat, sans `sed`.

**Binaire mesuré :** `agent.exe`, **10 035 712 octets**, rebâti après
`cargo clean --release -p proto -p agent` (20 fichiers, 34,1 MiB retirés) — le
binaire d'avant G1 pesait **9 914 368** octets et a été conservé sur la VM sous
`C:\dev\agent-v1-avant-g1.exe` pour servir de témoin v1.
**Plateforme :** lancée au commit `0bb1d87`, sur `192.168.3.1:8090`.

⚠️ **AUCUN TAUX N'EST REVENDIQUÉ NULLE PART.** Chaque énoncé porte son nombre
d'exécutions.

---

## 1. Le verdict, en quatre faits qui ne se simplifient dans aucun sens

**① LES SIX CRITÈRES SONT TENUS.** Les trois chiffres fondateurs de la spec
sont reproduits **à l'unité** par le produit, le catalogue suit les créations et
les retraits sans redémarrage, et un lancement ouvre la bonne fenêtre **par le
raccourci**, répertoire de travail compris.

**② LE CORPUS DE LA SPEC EST CONFIRMÉ PAR `IShellLinkW`, ET C'ÉTAIT LA QUESTION
OUVERTE (divergence E16).** Les 218 / 167 / 154 avaient été mesurés par
`WScript.Shell` ; `agent/src/apps/lecture.rs` est `#[cfg(windows)]` et n'avait
jamais tourné. **Les deux voies s'accordent sur les quatre chiffres
observables** — 218 lus, 51 écartés donc 167 retenus, 154 clés, et **7**
`cible-vide`. Aucun écart à écrire.

**③ 🔴 UN DÉFAUT DE PRODUIT NEUF, DE FRONTIÈRE, QUI MORD EN PERMANENCE :** le
**pont fichiers** hérite de `AGENT_VM`/`AGENT_SECRET` et s'enrôle sous la même
identité que son père le superviseur. Le registre de G1 (« le dernier
enrôlement gagne ») les fait s'évincer mutuellement, sans terme, à ~1,5 Hz.

**④ 🔴 LA GARANTIE « UN REFUS DE VERSION NE SE RÉESSAIE PAS » EST RÉFUTÉE PAR
LA MESURE**, et c'est le mode de panne que le protocole nomme lui-même comme
« le plus coûteux à diagnostiquer ».

**Ces quatre faits ne se compensent pas.** ③ et ④ sont des constats, pas des
correctifs : leur sort appartient au propriétaire du dépôt.

---

## 2. Les six critères

| # | Critère | Verdict | Exéc. |
| --- | --- | --- | --- |
| ① | 154 applications pour 218 raccourcis | **TENU** | **2** |
| ② | Un raccourci neuf apparaît sans redémarrer l'agent | **TENU** | **2** |
| ③ | Un raccourci retiré quitte le catalogue **sans que sa ligne disparaisse** | **TENU**, par le renvoi complet, **pas** par le chemin incrémental | **2** |
| ④ | Les raccourcis sans cible sont exclus **ET** journalisés | **TENU** — **7**, le compte annoncé d'avance | **2** |
| ⑤ | Un lancement ouvre la bonne fenêtre, **par le raccourci** | **TENU** sur ses deux assertions | **2** (agent) + **5** tentatives HTTP |
| ⑥ | Le lancement honore le répertoire de travail | **TENU** | **3** lancements |

### ① — 154 pour 218

Première réconciliation, **deux exécutions**, journaux
`exec2-…-plat.log` et le run mono-fenêtre :

```
catalogue reconcilie total=218 retenus=154 cles=154 apparues=154 modifiees=0 disparues=0 duree_ms=84
catalogue reconcilie total=218 retenus=154 cles=154 apparues=154 modifiees=0 disparues=0 duree_ms=92
```

`GET /applications?vm=…` rend **154** aux deux exécutions.

⚠️ **Le champ `retenus` de la trace ne compte PAS les 167 raccourcis retenus** :
il vaut `lancables.len()`, une table indexée par **clé**, donc toujours égal à
`cles`. Les 167 se dérivent — `218 − 51 écartés` — et ne sont émis nulle part.
**Relevé, pas corrigé.**

### ④ — sept, et le compte est BORNÉ à la première réconciliation

Comme l'exigent la décision D13 et la divergence E12 — jamais un total de
fichier. Écartés des lignes 1 à 58, avant la première ligne
`catalogue reconcilie`, **aux deux exécutions, chiffres identiques** :

| motif | compte |
| --- | --- |
| `cible-vide` | **7** |
| `extension` | 41 |
| `cible-absente` | 3 |
| **total** | **51** |

`218 − 51 = 167`. **Les trois chiffres de la spec, par `IShellLinkW`.**

### ② et ③ — le catalogue suit

② : création d'un `.lnk` pendant que l'agent tourne →
`total=219 retenus=155 cles=155 apparues=1`, `GET` rend **155**, et
`apparue_a = 1787202136748` est postérieur de **34 015 ms** au démarrage de
l'agent (`1787202102733`).

③ : retrait → **les deux contrôles**, comme le plan l'exige. `GET` rend **154**
et ne liste plus la ligne ; le `SELECT` direct la montre **toujours présente**,
`disparue_a` non nul.

🔴 **Réserve qui compte : aux DEUX exécutions, ce n'est pas le chemin
incrémental qui a posé `disparue_a`.** L'agent a bien annoncé `disparues=1`,
et la plateforme ne l'a pas appliqué ; c'est le **renvoi complet du
réenrôlement** (décision D3) qui a réparé, à chaque fois. La perte est
imputable au churn du §3 — un message montant mis en file pendant que le socket
est tombé est PERDU, et la divergence E2 le déclare.

✅ **D3 est donc EXERCÉE et TENUE dans son rôle de filet** : « c'est ce renvoi
complet qui rend la perte d'un message montant sans conséquence ». C'est la
première fois que ce filet est éprouvé sur le chemin réel.
❌ **Et le chemin incrémental reste NON DÉMONTRÉ**, faute d'un socket stable.

### ⑤ — lancé par le raccourci

Côté agent, **4 lancements, 4 fois la même forme** :

```
ordre de lancement reçu demande=4bcf9174-… cle=c12b7f3d…
raccourci lancé chemin="C:\ProgramData\…\Accessories\Notepad.lnk"
lancement demande="4bcf9174-…" cle="c12b7f3d…" issue=Raccourci
```

`Get-Process notepad` passe de **3 à 5** pour deux lancements.

Côté HTTP, **5 tentatives** : `{"issue":"raccourci"}` en **200** trois fois
(sur `g1-repertoire`), `504 {"refus":"delai"}` deux fois (sur `Notepad`) —
**et les deux expirations avaient LANCÉ**. La réponse `Lancee` se perd dans le
churn du §3.

### ⑥ — le répertoire de travail

Un `.lnk` vers `cmd.exe /c cd > sortie.txt`, `WorkingDirectory` posé à
`C:\dev\g1-rep`. Trois lancements par la route :

```
EXISTE_DANS_REPERTOIRE_DE_TRAVAIL contenu=C:\dev\g1-rep
ailleurs_C_dev: False   ailleurs_Desktop: False   ailleurs_System32: False
```

Le fichier atterrit **là**, et porte **ce** chemin.

---

## 3. 🔴 Le défaut de produit que la recette a trouvé : le pont s'évince avec son père

Journal `exec1-superviseur-churn-enrolement-plat.log` : **94** `agent enrôlé` et
**93** fermetures `CloseFrame { code: Policy, reason: "remplace" }` en **64 s**,
pour **zéro** `enfant lancé`.

**Attribution, sur pièces.** Trois `agent.exe` relevés par `Win32_Process` :
11312 (superviseur), 19004 et 19668, **tous deux enfants de 11312**. Les deux
lancements sont tracés à `04:54:37.305` — `capteur lancé pid=19004` et
`pont fichiers lancé pid=19668` — et la **première** fermeture `remplace` tombe
**93 ms plus tard**.

- Le **capteur** est hors de cause : il retourne AVANT l'enrôlement (E3).
- Le **pont** est placé APRÈS l'enrôlement **délibérément** — `main.rs` l'écrit :
  « il ouvre sa PROPRE `PeerConnection` vers la page-shell, donc il présente un
  jeton, exactement comme un enfant ».
- `lancer_pont` (`agent/src/superviseur/lanceur/pont.rs:48-61`) retire
  `SUPERVISEUR`, `CAPTEUR`, `TEST_FILE` et `WINDOW_TITLE`, **mais pas
  `AGENT_VM` ni `AGENT_SECRET`** — vérifié : `grep -rn env_remove agent/src/`
  ne rend **aucune** occurrence de ces deux noms.

Le pont s'enrôle donc sous le **même `vm_id`** que son père, et la décision D8
de G1 — « une VM peut apparaître deux fois : le dernier enrôlement gagne,
l'ancien socket est fermé avec `FERMETURE_POLITIQUE` » — les fait s'évincer
mutuellement, **sans terme**.

**Ce que cela coûte, mesuré :**

- le catalogue complet (154 entrées) est **renvoyé à chaque cycle**, puisque
  chaque réenrôlement repose `complet = true` ;
- les messages montants **incrémentaux se perdent** — c'est la réserve de ③ ;
- les réponses `Lancee` se perdent aussi — c'est le `504` de ⑤ ;
- **le pont ne survit pas non plus** : il meurt sur « aucune offre SDP pour le
  pont fichiers » faute de page-shell, et `surveillance_pont` le relance toutes
  les 500 ms. C'est ce qui entretient le cycle ;
- **deux boucles de découverte tournent au lieu d'une** — `apps::brancher` est
  appelée avant l'aiguillage `PONT`, donc le pont réconcilie lui aussi. Chaque
  trace `raccourci ecarte` apparaît **deux fois**, une par processus, et le
  travail COM/Shell est fait deux fois toutes les 30 s.

**Défaut de FRONTIÈRE entre trois chantiers** — l'enrôlement de P3, le pont de
F1, le registre de G1 — **correct de chaque côté pris séparément**. C'est
exactement la classe de défaut que la revue transverse existe pour trouver, et
c'est la **mesure** qui l'a trouvé.

⚠️ **Il n'est PAS corrigé**, et le remède n'est pas évident : donner au pont sa
propre identité, le faire ne pas s'enrôler, ou faire tolérer au registre
plusieurs sockets par VM sont trois décisions différentes.

---

## 4. 🔴 Le refus de version ne peut pas être lu, et la VM boucle

**Step 3 du plan, la « rouge de D10, gratuite », UNE exécution**, journal
`step3-version-v1-contre-v2-plat.log`.

Un agent **v1** contre une plateforme **v2**. Le plan attendait
`la plateforme REFUSE la version du canal /agent : aucune reprise`. Relevé :

| ligne cherchée | compte |
| --- | --- |
| `la plateforme REFUSE la version` | **0** |
| `message de la plateforme illisible (version divergente ?)` | **10** |
| `reprise du canal /agent` | **10** |

verbatim :

```
WARN agent::plateforme: message de la plateforme illisible (version divergente ?)
  url="ws://192.168.3.1:8090/agent" erreur=version de plateforme non supportée : 2
  texte="{\"type\":\"refus\",\"v\":2,\"motif\":\"version\"}"
INFO agent::plateforme: reprise du canal /agent … tentative=8 delai_ms=30000
```

**La cause est structurelle, lue dans le code :** `verifie_version`
(`proto/src/plateforme.rs`) est un `deserialize_with` posé sur le champ `v` de
**tout** message, **le refus compris**, et la plateforme émet son refus avec
**sa** version. Un agent de version N ne peut donc **jamais lire** le refus
d'une plateforme de version M ≠ N : il tombe dans la branche « illisible », qui
est **reprenable**. Le bras `MotifCanal::Version` de `sur_refus` n'est
atteignable que si les deux bouts s'accordent déjà sur `v` — **c'est-à-dire
jamais dans le seul cas pour lequel il existe.**

⚠️ **Le fait de D10 tient : la plateforme REFUSE bien.** C'est sa
**conséquence** qui est fausse. Et les deux commentaires qui la promettaient
nommaient eux-mêmes le mode de panne obtenu :

- « sans quoi une incompatibilité de version se déguiserait en **boucle de
  reconnexion infinie**, qui est le mode de panne le plus coûteux à
  diagnostiquer » ;
- « la VM **se tait sans boucler**, ce qui est exactement le comportement
  voulu — un silence franc plutôt qu'une reconnexion infinie ».

**Les trois places portent désormais le constat** (revue transverse, §6).
**Constat, pas correctif** : le remède demande de décider comment lire un
message dont la version diverge sans le désérialiser entièrement, et c'est une
décision de protocole.

✅ **Ce que la mesure confirme sans réserve :** l'obligation de déployer agent
et plateforme **au même commit** est inchangée, et même renforcée — c'est la
seule parade qui existe aujourd'hui.

---

## 5. Les rouges jouées, avec leur forme

| Rouge | Où | Forme obtenue |
| --- | --- | --- |
| **①** — la clé par la cible seule | test d'hôte, mutation de `cle()` | `le_corpus_reel_rend_218_lus_167_retenus_154_cles_…` échoue : **`left: 112, right: 154`**, « applications distinctes sur cette VM ». Un second test tombe aussi (`la_cle_replie_la_casse_…`). Source restaurée, `git status` vide |
| **⑤ et ⑥** — lancer par la cible reconstruite | **sur la VM**, binaire muté et redéployé | La route rend **`{"issue":"cible"}` 3 fois sur 3** au lieu de `raccourci`, et `sortie.txt` **n'atterrit NULLE PART** — ni dans le répertoire de travail, ni dans `C:\dev`, ni sur le Bureau, ni dans `System32`. Journal `rouge-6-…-plat.log`, qui porte `lancé par la CIBLE, pas par le raccourci`. Source restaurée et binaire rebâti à l'identique |
| **③** — supprimer la ligne | déjà jouée en T14 sur l'hôte | rejouée ici sur le chemin réel par la **lecture des deux contrôles** : `GET` et `SELECT` direct |
| **`Resolve` jamais appelée** | contrôle sur `agent/src/**/*.rs` | forme corrigée (blanchiment des commentaires) : **0** sur l'arbre réel, **1** dès qu'un appel est injecté. La forme naïve rendait `2 / 3` et comptait les commentaires **qui disent qu'on ne l'appelle pas** |
| **Lacune de nommage** | mutation de `rename_all` | `IssueLancement` en `snake_case` : **75 passed, 0 failed** — le contrôle NE PEUT PAS rougir. Même mutation sur l'enum portant `BattementRecu` : `conformite_aux_vecteurs_partages` **ÉCHOUE**. Lacune désormais **inscrite** dans le code |

**Une rouge attendue qui n'a PAS eu la forme prévue : celle de D10** — voir §4.

---

## 6. La revue transverse — huit défauts, seize places

Barème des sous-blocs antérieurs : 5 en D7, 3 en D8, 6 en D9, douze en D10,
sept en D11, huit en P1, dix en P2 (sur vingt-trois places), cinq en S1, neuf
dans le chantier E, douze en P3, douze en S2, onze en F1, huit en P4.

**Les six premiers franchissent une frontière de tâche ou de chantier**, et
sont corrects de chaque côté pris séparément.

| # | Place(s) | Ce qui est devenu faux |
| --- | --- | --- |
| 1 | `0003-agents.sql` l.2, 43-44, 49-51 (**3 places**) | « `application` … RESTE VIDE », « qui **empruntera** le canal », « PAS un oubli si **aucun code ne l'écrit** ». ④ les a prises au mot |
| 2 | `agents/canal.ts` l.1 | l'en-tête annonçait un canal « enrôlement, battement, jeton frais » |
| 3 | `agents/canal.ts` l.9-10 | « ni garde, ni **registre d'appartenance** » : vrai du registre du RELAIS, mais le canal en tient désormais **un autre**, le sien |
| 4 | `agent/src/plateforme.rs` l.1-2 | « celui-ci porte une IDENTITÉ » — il porte aussi un catalogue et des ordres |
| 5 | `agent/src/plateforme.rs`, doc de `Canal` | « Le lâcher arrête le battement de cœur » — il arrête aussi la **découverte** |
| 6 | 🔴 `proto/src/plateforme.rs` (**2 places**) + `agent/src/plateforme.rs` doc de `sur_refus` (**1**) | **la MESURE réfute le commentaire** — voir §4 |
| 7 | spec `2026-08-19-gestion-apps-design.md` (**4 places**) | §3.3 complété par E7 et E8 ; §5 critères ④ et ⑤ (E12, E13) ; §6 portée et hub (E1, E14). **Annoté, pas réécrit** : relevé daté |
| 8 | plan `2026-08-19-plateforme-p3.md` (**2 places**) | « ses **neuf** étapes » pour **dix** (relevé par la commande) ; le legs « ④ empruntera le canal », **CLOS** |

**Step 3 du plan, vérifié PAR LA COMMANDE et non supposé :** aucun des **47**
fichiers touchés par les commits `(g1)` n'est sous `agent/src/capteur/`,
`agent/src/superviseur/`, `src/`, `web/` ni `client/` — **E18 et D12 tiennent**.
Le bras catch-all `Ok(autre)` de `capteur/pont_media.rs` est intact, et aucun
message de G1 ne passe par ce canal.

---

## 7. Les relevés annexes

| Relevé | Valeur | Nombre d'exécutions |
| --- | --- | --- |
| **Durée d'une réconciliation complète** | **84 ms** et **92 ms** à froid après enrôlement ; **2 032 ms** au tout premier tour d'un processus (initialisation COM) ; **49 à 63 ms** à chaud | 2 + 1 + plusieurs |
| **Taille d'un `Catalogue` complet sur le fil** | **56 145 octets** de charge utile TCP montante, dont **160** pour le handshake HTTP, soit **55 985 octets** pour `enroler` + `Catalogue` complet + masquage WebSocket. **MESURÉ** par `tshark` sur `internalBridge`, pas calculé | 1 |
| — à opposer à | **~46 Kio CALCULÉS** par la décision D3 | — |
| **Écart `IShellLinkW` / `WScript.Shell`** | **AUCUN** sur les quatre chiffres observables : 218, 167, 154, 7 | 2 |
| **`.lnk` de zéro octet** | catalogue **complet** (`total=220 cles=155`), et une trace le nomme : `raccourci ecarte chemin=…\g1-vide.lnk motif="cible-vide"` | 1 |
| **`APPS=0`** | `scripts/run-agent.sh` écrit bien `$env:APPS = '0'` (ligne 12 du `run-agent.ps1` généré) ; l'agent journalise `decouverte d'applications DESARMEE (APPS=0)` ; **0** `catalogue reconcilie` et **0** `raccourci ecarte` sur **63 s**, soit plus de deux périodes | 1 |

🔴 **`APPS=0` est la vérification de la tâche 12, elle ne pouvait se faire
qu'ici, et elle n'avait JAMAIS été faite** — l'agent des tâches 1-12 l'avait
déclarée non faite faute de VM. **Elle est faite.**

### Le corpus versé — pourquoi il n'a PAS été remplacé (step 5)

Le plan prescrit de remplacer `agent/testdata/gapps-corpus-vm.json` par « le
relevé de l'agent ». **Ce n'est pas fait, et c'est déclaré plutôt que
dissimulé :** l'agent n'a **aucun mode de vidage de corpus** — il journalise des
comptes, jamais la liste des 218 entrées avec leurs cinq champs. Produire ce
relevé demanderait d'ajouter un mode au produit, ce qui n'est pas une tâche de
recette.

**Ce que la recette apporte à la place, et qui répond à l'objet d'E16 :** la
**confrontation** des deux voies sur tous les chiffres observables, et elles
**s'accordent exactement**. Aucun chiffre n'a bougé, donc rien n'était à écrire
comme écart.

⚠️ **Ce qui reste vrai, et qui est la limite de cette réponse :** les champs
**par entrée** du corpus restent ceux de `WScript.Shell`. Seuls les **agrégats**
sont confirmés par `IShellLinkW`. Un désaccord sur un raccourci **individuel**
qui se compenserait dans les totaux ne serait pas vu.

---

## 8. Un piège d'outillage neuf, et il mord la clôture elle-même

🔴 **`scripts/verify-all.sh` n'est PAS hermétique : il hérite de
l'environnement de l'appelant, et `.env` le fait échouer.**

Relevé, **une exécution de chaque** :

| Environnement | `src/signaling/server.test.ts` |
| --- | --- |
| sans `TURN_URL` / `TURN_SECRET` | **12 passed** |
| avec (c'est-à-dire après `source .env`) | **6 failed, 6 passed** |

Les six assertions attendent un premier message et reçoivent `ice-config`.
**Aucun fichier de signaling n'a été touché par G1** (vérifié par la commande) :
c'est une **sensibilité préexistante**, pas une régression.

⚠️ **Elle mord précisément qui suit la procédure du dépôt** : tout travail sur
la VM **exige** `set -a && source .env && set +a`, et lancer `verify-all.sh`
dans ce même shell rend un arbre rouge **pour la mauvaise raison**. La première
exécution de la clôture, lancée depuis un shell propre, est sortie à **0** ; la
seconde, depuis un shell où `.env` avait été sourcé, à **1**. **Le même arbre,
au même commit.**

---

## 9. Ce que G1 n'établit PAS

- **Aucun taux, nulle part.** Deux exécutions par critère au mieux, une seule
  pour plusieurs relevés annexes et pour la rouge de version.
- **Le chemin INCRÉMENTAL de `disparues` n'est pas démontré** : aux deux
  exécutions, c'est le renvoi complet qui a réparé.
- **Le défaut d'enrôlement du §3 n'est pas corrigé**, et la recette a donc été
  conduite **sous** ce défaut. Toutes les mesures de catalogue portent cette
  condition.
- **Aucun mode mono-processus long ne existe sans navigateur** : le mono-fenêtre
  meurt sur « le signaling s'est fermé avant l'offre », le mode `PONT=1` sur
  « aucune offre SDP pour le pont fichiers ». Le mode `SUPERVISEUR` est le seul
  qui tienne, et c'est celui qui porte le churn.
- **Le remède du §4 n'est pas tranché**, ni celui du §3.
- **Une seule VM, un seul catalogue** de 218 raccourcis. Rien de la charge,
  rien de plusieurs VMs enrôlées, rien d'un catalogue plus grand.
- **Rien de l'isolation entre utilisateurs** (D9) : `vm.utilisateur_id` est NULL,
  et la ligne `vm non attribuee, acces accorde sans isolation …` a bien été
  observée au journal de la plateforme, à chaque requête.
- **Aucune icône, aucun téléversement, aucune surveillance, aucune PWA, aucune
  page de hub** — G2 à G5.
- **Aucune constante calibrée** : `PERIODE_RECONCILIATION`,
  `DELAI_LANCEMENT_MS`, `FILE_EMISSION`. Elles rejoignent la liste déjà longue
  du dépôt.
- **Rien d'un antivirus** : l'état de celui de la VM n'a **pas** été relevé, ni
  par la spec, ni ici.
- **Aucun audit de sécurité** de l'authentification introduite par D9.
- **La divergence `403 vm-etrangere` / `404 vm-inconnue` n'est PAS tranchée** —
  voir §10.

---

## 10. 🔴 La divergence de sécurité, VIVANTE dans le produit, et NON TRANCHÉE

**Le même service rend aujourd'hui deux réponses différentes selon la route,
pour la même situation.**

| Route | Chantier | Réponse à une VM qu'on n'a pas le droit de voir |
| --- | --- | --- |
| `plateforme/src/http/routes-applications.ts` | **G1** (décision D9) | `403 { refus: 'vm-etrangere' }`, **distinct** de `vm-inconnue` |
| `plateforme/src/http/routes-vm.ts` | **P4** | `404 { refus: 'vm-inconnue' }`, **indistinguable** |

Le `403` de G1 est un **oracle d'énumération** : un utilisateur apprend par
tâtonnement quelles VMs existent. Le `404` de P4 suit `routes-auth.ts` et
`agents/enrolement.ts`, qui refusent tous deux de distinguer « inconnu » de
« faux ».

**Les deux chantiers ne peuvent pas avoir raison en même temps.** Chacun a
écrit la divergence en tête de son propre fichier ; **aucun ne l'a tranchée**,
et G1 est implémenté tel qu'il est écrit.

⚠️ **Unifier est une DÉCISION, pas une correction, et elle appartient au
propriétaire du dépôt.** Elle est signalée ici plutôt que prise en douce.

---

## 11. Les legs de G1

1. 🔴 **Le pont hérite de l'identité d'enrôlement de son père** (§3). Trois
   remèdes possibles, aucun tranché. **C'est le legs le plus lourd** : il mord
   en permanence, en configuration livrée.
2. 🔴 **Un refus de version ne peut pas être lu par le pair qui en a besoin**
   (§4). Décision de protocole.
3. 🔴 **La divergence `403`/`404`** (§10). Décision du propriétaire du dépôt.
4. ⛔ **Le chemin incrémental de `disparues` n'est pas démontré** : il le sera
   quand le legs n°1 sera fermé.
5. ⛔ **`verify-all.sh` n'est pas hermétique** (§8) — et il échoue précisément
   pour qui suit la procédure de la VM.
6. ⛔ **Le corpus reste celui de `WScript.Shell` par entrée** (§7). Le fermer
   demande un mode de vidage dans l'agent.
7. ⛔ **Le champ `retenus` de la trace de réconciliation ne compte pas ce que
   son nom dit** (§2 ①) : il vaut `cles`. Les 167 ne sont émis nulle part.
8. ⛔ **Deux boucles de découverte tournent** quand le superviseur est actif
   (§3) — conséquence du legs n°1.
9. ⛔ **La lacune de nommage d'`IssueLancement`** est inscrite mais pas fermée :
   la première variante écrite en deux mots la refermera d'elle-même.

---

## 12. Le contrôle qu'aucune preuve ne vit hors de git

```
git ls-files docs/superpowers/plans/journaux-gestion-apps | wc -l   → 10
git ls-files agent/testdata/gapps-corpus-vm.json                    → suivi
```

**Les dix journaux sont versés**, et le corpus est suivi par git. La spec §11
dit que les sondes de ④ vivent dans `C:\dev\` sur la VM, « c'est-à-dire nulle
part de durable » — c'est pourquoi tout ce qui fonde ce document est ici.

⚠️ **Ce qui ne vit PAS dans git, et c'est délibéré :** le secret d'enrôlement de
la VM, le mot de passe du compte de recette et le secret de signature des
jetons. Ils ont vécu dans le répertoire de travail temporaire de la session et
n'apparaissent dans **aucune** pièce versée.

### Renvoi vers chaque journal

| Journal | Ce qu'il porte |
| --- | --- |
| `build-agent-g1.log` | la compilation du binaire mesuré |
| `step3-version-v1-contre-v2-plat.log` | §4 — la rouge de version, réfutée dans sa conséquence |
| `exec1-superviseur-churn-enrolement-plat.log` | §3 — 94 enrôlements, 93 évictions |
| `exec2-superviseur-criteres-1-2-3-5-plat.log` | ①, ②, ③, ④, ⑤ |
| `exec3-superviseur-criteres-5-6-annexes-plat.log` | ⑤ et ⑥, les quatre `issue=Raccourci` |
| `exec4-superviseur-lnk-vide-et-repertoire-plat.log` | le `.lnk` de zéro octet, `total=220` |
| `rouge-6-lancement-par-la-cible-plat.log` | la rouge de ⑤ et ⑥, jouée sur la VM |
| `annexe-apps0-desarmement-plat.log` | `APPS=0`, la vérification de la tâche 12 |
| `annexe-taille-catalogue-sur-le-fil.txt` | les 56 145 octets, relevés par `tshark` |
| `temoin-cloture-verify-all.log` | la clôture, et le piège d'environnement du §8 |
