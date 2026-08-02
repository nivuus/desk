# Sous-bloc D4 — capture mutualisée : résultats de la recette

**Date** : 2 août 2026
**Conception** : `docs/superpowers/specs/2026-08-02-multifenetres-capture-mutualisee-design.md`
**Plan** : `docs/superpowers/plans/2026-08-02-multifenetres-capture-mutualisee.md`
**Sous-bloc précédent** : D3, `2026-08-02-multifenetres-plafond-concurrence-resultats.md`
**Branche** : `chantier-multifenetres-d4`, recette jouée sur `f2b3fbe`
(soit `770ff01` plus le seul correctif décrit au §3.1)
**Journaux** : `docs/superpowers/plans/journaux-multifenetres-d4/` — **tous en
UTF-8**, séquences ANSI de `tracing` déjà retirées (les fichiers se lisent et se
`grep`ent à plat, sans `sed`).

---

## 0. Le verdict

**Aucun des trois critères de réception n'est tenu, et la cause est unique :
le canal entre le capteur et ses enfants ne délivre pas sa réponse d'attache.
Aucune session WebRTC ne s'établit, sur aucun rang.**

Ce n'est pas un résultat partiel qu'on pourrait arrondir. Sur les deux
exécutions de recette jouées sur le binaire de la branche, le compteur de pages
navigateur qui **diffusent réellement** (images décodées en croissance) vaut
**0** à tous les rangs, de 1 à 8 fenêtres.

| Critère | Verdict | Le chiffre |
| --- | --- | --- |
| 1 — dépasser quatre fenêtres **diffusant**, et nommer le plafond suivant | **NON TENU** | **0 fenêtre diffuse**, à tous les rangs de 1 à 8 |
| 2 — tuer le capteur ne tue aucune session | **NON TENU** | les **4** enfants meurent en `ExitStatus(1)` à l'instant même de la mise à mort |
| 3 — la cadence relevée avant et après | **NON MESURÉ** | sans « après », un « avant » seul n'a pas de référent — il n'a pas été joué |

**Ce que la recette établit tout de même, et qui est le fait le plus utile de ce
document :** un **unique processus capteur** a tenu **8 duplications DXGI et
8 encodeurs matériels NVENC** simultanément, sur 8 sorties virtuelles distinctes,
**zéro `WARN`, zéro `ERROR`**. La thèse centrale de D4 — mutualiser la capture
retire le plafond de quatre **processus** établi par D3 — est donc **soutenue au
niveau de la capture**, et **réfutée par rien**. Elle n'est pas *démontrée en
conditions de produit* pour autant, puisque rien n'a été diffusé.

⚠️ **Deux exécutions de recette, une par critère : aucun taux, nulle part.**
Aucune des mesures de ce document ne porte de fréquence, de variance ni
d'intervalle de confiance.

---

## 1. Ce qui a été exécuté, et ce qui a été écarté

### Exécuté

1. **Contrôle d'état initial** depuis un processus neuf (`MULTIFENETRE_DXGI=1`) :
   une seule sortie, `\\.\DISPLAY1`, 2400×1080
   (`topologie-initiale.log`).
2. **Une exécution de fumée à 2 fenêtres** sur le code de la branche tel que
   livré — c'est elle qui a révélé le défaut du §3.1
   (`defaut-canal-1-code-initial.log`).
3. **Un correctif**, signalé avant d'être appliqué, puis appliqué et committé
   (`f2b3fbe`), puis **une seconde exécution de fumée** qui montre que le défaut
   n'est pas levé pour autant
   (`defaut-canal-2-apres-correctif-flush.log`).
4. **Trois exécutions de DIAGNOSTIC** sur un binaire **délibérément modifié**
   (modifications non committées, revenues depuis), pour isoler la cause
   (`defaut-canal-3`, `-4`, `-5`).
5. **La montée du critère 1**, de 1 à 9 fenêtres, sur le binaire de la branche
   (`critere1-montee-agent.log`, `critere1-montee-pilote.log`).
6. **L'épreuve du critère 2**, mise à mort du capteur par PID relevé, à
   4 fenêtres (`critere2-capteur-agent.log`, `critere2-capteur-pilote.log`).
7. **Purge et contrôle de topologie final** depuis un processus neuf
   (`purge-finale.log`, `topologie-finale.log`).

### Écarté, explicitement

- **Le « avant » du critère 3** (recompilation sur `7d7e254`, dernier commit où
  l'enfant capture lui-même). **Non joué.** Sans « après » mesurable, un
  « avant » seul ne se compare à rien : il aurait produit un chiffre sans
  référent, ce que ce dépôt appelle précisément un énoncé qui dépasse sa mesure.
  **Il n'existe donc aucun relevé de cadence d'avant la mutualisation dans ce
  document.**
- **La correction du défaut du §3.2.** Elle exige de refondre la couche de
  transport du canal (voir §3.4) : ce n'est plus un correctif, c'est une
  reconception, et un agent de recette qui mesurerait sa propre reconception
  produirait un document que personne ne pourrait lire comme une recette.
  **Signalée, non entreprise.**
- **Toute capture d'écran CDP pendant une mesure**, par contrainte de protocole
  héritée de D1 (elle provoque un `Resize`, donc un `SHOW`, donc une session et
  une sortie de plus).
- **La latence bout en bout**, hors périmètre du plan.

---

## 2. Le relevé du critère 1

### 2.1 Ce qui a été observé

Montée de 1 à 9 Bloc-notes, ouverts **un par un après le superviseur** (le cas
produit, pas l'énumération initiale), chaque rang attendu **sur le fait**
— l'apparition d'une page navigateur — et non sur une durée.

| Rang | Pages navigateur | Fenêtres qui **diffusent** | Issue |
| --- | --- | --- | --- |
| 1 | 1 | **0** | page ouverte |
| 2 | 2 | **0** | page ouverte |
| 3 | 3 | **0** | page ouverte |
| 4 | 4 | **0** | page ouverte |
| 5 | 5 | **0** | page ouverte |
| 6 | 6 | **0** | page ouverte |
| 7 | 7 | **0** | page ouverte |
| 8 | 8 | **0** | page ouverte |
| **9** | **8** | **0** | **refusée** |

`critere1-montee-pilote.log` :

```
[  132.3s 2026-08-02T18:52:39.461Z] RANG 8 : pages=8 DIFFUSENT=0 ouverte=true
[  224.0s 2026-08-02T18:54:11.111Z] RANG 9 : pages=8 DIFFUSENT=0 ouverte=false
[  224.0s 2026-08-02T18:54:11.111Z] >>> ARRÊT DE LA MONTÉE au rang 9 : aucune page de plus.
```

**« DIFFUSENT »** est le nombre de pages dont `framesDecoded` a **augmenté**
entre deux relevés `getStats` espacés de 5 s. Il vaut 0 partout. Le relevé brut
le dit sans ambiguïté — `"etat":"new"` est l'état ICE, et `"images":null`
signifie qu'aucune statistique `inbound-rtp` vidéo n'existe :

```
STATS (rang 8 — relevé 2) {"w:w-2":{"etat":"new","images":null,...
```

### 2.2 Le rang qui arrête la montée est nommé — et ce n'est pas un `HRESULT`

Le critère demandait « l'appel exact et son `HRESULT` ». **Il n'y en a pas**, et
c'est le relevé lui-même qui l'établit : la montée s'arrête sur un **refus du
produit**, pas sur un refus du système. La page-shell affiche, au rang 9
(`critere1-montee-pilote.log`) :

```
statut="« Sans titre - Bloc-notes » n'a pas pu s'ouvrir : plus aucune sortie
virtuelle disponible."
```

C'est le libellé de `Effet::AnnoncerRefus` émis par `Table::detecter` quand
`self.entrees.len() >= self.capacite`, avec `CAPACITE = 8`
(`agent/src/superviseur/boucle.rs:58`). **Le superviseur a refusé la neuvième
fenêtre parce qu'on le lui a demandé**, à la valeur choisie par la tâche 8 pour
coïncider avec le plafond d'encodeurs connu.

Conséquence à énoncer nettement : **cette recette n'a approché aucun plafond du
système.** Ni le plafond d'encodeurs de 8, ni le plafond de sorties virtuelles
de 10, ni quoi que ce soit d'autre n'a été mis en défaut. Aucun `HRESULT` de
refus n'apparaît dans le journal — `grep -c "ERROR\|WARN"` rend **0** sur
`critere1-montee-agent.log`.

### 2.3 Le fait solide : 8 duplications et 8 encodeurs dans UN processus

C'est ce que la montée établit réellement, et c'est le cœur de la thèse de D4.
Comptes exacts sur `critere1-montee-agent.log` :

| Ligne comptée | Compte |
| --- | --- |
| `capteur lancé` | **1** |
| `sortie virtuelle créée` | 8 |
| `enfant lancé` | 8 |
| `source distante servie par le capteur` (côté enfant) | 8 |
| `duplication de sortie établie` | **8** |
| `encodeur matériel retenu` | **8** |
| `WARN` / `ERROR` | **0** |

Le capteur est unique et identifié :

```
2026-08-02T18:50:48.030244Z  INFO agent::superviseur::lanceur: capteur lancé pid=18212
2026-08-02T18:50:48.035660Z  INFO agent::capteur: capteur démarré tube="\\\\.\\pipe\\agent-capteur"
```

Les huit duplications portent huit **noms de sortie distincts** — comparés par
ensemble de noms, jamais par cardinal :

```
\\.\DISPLAY5  \\.\DISPLAY6  \\.\DISPLAY7  \\.\DISPLAY8
\\.\DISPLAY9  \\.\DISPLAY10 \\.\DISPLAY11 \\.\DISPLAY12
```

et les huit enfants n'en ouvrent aucune, chacun journalisant
`source distante servie par le capteur (mode multi-fenêtres)`. La première
duplication est établie à `18:50:52.289418Z`, la huitième à `18:52:27.103991Z`.

**Portée exacte de ce fait, à ne pas élargir** : ce sont des **constructions**
réussies. Aucune image n'a été comptée sur ces huit voies (le compteur
`cadence du capteur` n'apparaît pas — voir §3.2, il est en aval du point de
blocage), et les huit encodeurs n'ont donc **jamais été alimentés ensemble**
dans cette exécution. C'est exactement la réserve que portait déjà la mesure ②
du 31 juillet 2026, et elle n'est pas levée ici.

---

## 3. Ce qui a échoué, et pourquoi

### 3.1 Un premier défaut, signalé puis corrigé : la réponse d'attache n'était pas vidée

`agent/src/capteur/fenetre.rs` écrivait la réponse `Attachee` dans un
`BufWriter` **sans la vider**, là où toutes les autres écritures du fichier
étaient déjà suivies d'un `flush` (commandes, images, `Etat`). La réponse
restait donc dans le tampon jusqu'à la première écriture qui le vide,
c'est-à-dire **jusqu'à la première image**. Or Desktop Duplication ne rend une
image qu'au changement du bureau, et un Bloc-notes immobile n'en produit aucune.

Symptôme relevé sur `defaut-canal-1-code-initial.log` : le capteur journalise
`fenêtre attachée au capteur` pour les deux fenêtres, l'enfant ne journalise
**jamais** `attaché au capteur`, et les deux pages restent en
`iceConnectionState = "new"`.

**Corrigé** au commit `f2b3fbe`, une ligne plus son commentaire. Le correctif
est juste et nécessaire — mais **il ne suffit pas**, et c'est le §3.2.

### 3.2 Le défaut bloquant : l'écriture du capteur n'aboutit pas

Sur le binaire corrigé (`defaut-canal-2-apres-correctif-flush.log`), le blocage
**se déplace en amont** : le capteur ne journalise même plus
`fenêtre attachée au capteur`. Sa dernière ligne par fenêtre est

```
2026-08-02T18:38:25.059801Z  INFO agent::encode: convertisseur BGRA→NV12 (Video Processor MFT) configuré converter_provides_samples=true
```

puis plus rien. La trace `fenêtre attachée au capteur` est posée **juste après**
le `flush` ajouté : elle manque donc parce que **ce `flush` ne rend pas**.

**L'expérience différentielle qui l'établit.** Même binaire, même séquence, une
seule variable changée — le fil qui lit les commandes de l'enfant
(`lire_les_commandes`, lancé par `accueillir` dans `agent/src/capteur/serveur.rs`)
est **empêché d'entrer en lecture bloquante**. Résultat
(`defaut-canal-3-sans-fil-lecteur.log`) :

```
2026-08-02T18:42:25.350820Z  INFO agent::capteur::fenetre: fenêtre attachée au capteur session=w-2 sortie=\\.\DISPLAY5 largeur=1280 hauteur=720
2026-08-02T18:42:25.350854Z  INFO agent::capteur::tube: attaché au capteur session=w-2 sortie=\\.\DISPLAY5 largeur=1280 hauteur=720
```

— l'attache se termine des **deux** côtés, en 34 µs, et l'ICE des deux pages
passe à `"etat":"connected"` avec un RTT relevé de 0,015 à 0,082 s.

Un second tirage (`defaut-canal-4-sans-fil-lecteur-tx-fuite.log`) va plus loin
et montre que **le chemin média fonctionne** une fois l'obstacle retiré : le
navigateur décode de vraies images H.264, `1280×720`, `octets: 131518` reçus, et
les deux compteurs de cadence apparaissent enfin et **s'apparient** :

```
2026-08-02T18:45:50.340538Z  INFO agent::capteur::fenetre: cadence du capteur session=w-2 images=7 cadence="0.7"
2026-08-02T18:45:43.396984Z  INFO agent::transport::cadence_video: cadence de la piste vidéo (côté enfant) session=w-2 unites=7 cadence="0.5"
```

**Le fait établi, formulé exactement** : *l'écriture du capteur sur le tube
n'aboutit pas tant que son fil de lecture de commandes a une lecture bloquante
pendante sur la même instance de tube ; en l'absence de cette lecture pendante,
l'attache se termine, l'ICE s'établit et des images H.264 arrivent au
navigateur.* C'est un fait **différentiel**, sur un binaire par ailleurs
identique.

### 3.3 Ce que ce diagnostic n'établit PAS — la part d'hypothèse

L'explication qui vient à l'esprit est la sérialisation des entrées/sorties sur
un objet fichier **synchrone** sous Windows (les deux extrémités sont ouvertes
sans `FILE_FLAG_OVERLAPPED`, et `try_clone` duplique le descripteur sans créer
d'objet fichier neuf). **Cette explication n'est PAS établie, et une observation
la contrarie** : côté **enfant**, l'écriture d'une commande **aboutit** alors que
son propre fil `repartir_les_trames` est en lecture bloquante sur son objet
fichier. On le lit à ce que `commander` échoue sur son *délai de réponse*, dont
le libellé n'est attaché qu'au `recv_timeout` :

```
2026-08-02T18:45:53.459644Z  WARN agent::transport::adaptation: l'encodeur refuse le réglage du débit à chaud erreur=aucune réponse du capteur à Debit { bps: 12000000 }
```

Atteindre ce message suppose que l'écriture et son `flush` sont passés. **Il y a
donc une asymétrie entre les deux extrémités que ce document ne sait pas
expliquer.** Le mécanisme reste **inconnu** ; seul l'effet est mesuré.

Une troisième tentative de diagnostic (`defaut-canal-5-fil-lecteur-retarde.log`,
fil lecteur endormi 25 s) s'est révélée **confondue** et n'est versée que pour
mémoire : un lecteur endormi ne répond à aucune commande, donc `commander`
expire à 10 s et affame `Session::run`, ce qui casse la session pour une raison
étrangère à la question posée. **Elle n'étaye rien.**

### 3.4 Ce que le remède demande — et pourquoi il n'a pas été fait ici

Deux voies se dessinent, et **aucune n'est un correctif** :

- **entrées/sorties recouvrantes** (`FILE_FLAG_OVERLAPPED` aux deux extrémités,
  `ConnectNamedPipe` compris, plus une enveloppe `Read`/`Write` sur `OVERLAPPED`) ;
- **une connexion par sens** (l'enfant ouvre deux tubes, un par direction), ce
  qui réintroduit exactement l'appariement d'état que le §3.2 de la conception
  voulait éviter.

L'une comme l'autre est de la conception, dans du code `#[cfg(windows)]` que
l'hôte ne sait que **typer**, sur la couche même que le sous-bloc a bâtie en
huit tâches. **Cela revient au sous-bloc, pas à sa recette.**

### 3.5 Le relevé du critère 2

Épreuve jouée à **4 fenêtres**, capteur tué **par PID relevé**
(`Stop-Process -Id 28328`, jamais `pkill -f`).

**La moitié « supervision » tient.** Bornes temporelles explicites :

| Événement | Horodatage |
| --- | --- |
| mise à mort demandée (horloge du pilote, UTC) | `2026-08-02T18:57:12.756Z` |
| `capteur terminé pid=28328` | `2026-08-02T18:57:13.294396Z` |
| `capteur lancé pid=19640` | `2026-08-02T18:57:13.295794Z` |
| `capteur mort, relancé pid_mort=28328 pid_neuf=19640` | `2026-08-02T18:57:13.295805Z` |

Soit une relance en **≈ 0,54 s** après la demande de mise à mort, et **1,5 ms**
après le constat de la mort. Les quatre sorties virtuelles sont **réemployées**
(`DISPLAY5`–`DISPLAY8`, 4 sorties créées pour 8 lancements d'enfant au total) :
la rétention acquise en D3 fonctionne.

**La moitié qui fait le critère ne tient pas.** Les quatre enfants meurent à
l'instant même :

```
2026-08-02T18:57:13.294243Z  INFO agent::superviseur::lanceur: enfant terminé pid=27712 code=ExitStatus(ExitStatus(1))
2026-08-02T18:57:13.294330Z  INFO agent::superviseur::lanceur: enfant terminé pid=25952 code=ExitStatus(ExitStatus(1))
2026-08-02T18:57:13.294336Z  INFO agent::superviseur::lanceur: enfant terminé pid=23932 code=ExitStatus(ExitStatus(1))
2026-08-02T18:57:13.294358Z  INFO agent::superviseur::lanceur: enfant terminé pid=28332 code=ExitStatus(ExitStatus(1))
2026-08-02T18:57:13.294380Z  WARN agent::superviseur::enfants: enfant mort de lui-même session=w-10 pid=Some(27712)
```

et quatre enfants **neufs** sont lancés, sur des sessions **neuves**
(`w-2, w-4, w-6, w-10` → `w-11, w-12, w-13, w-14`). Le pilote le confirme du
côté navigateur : les quatre pages d'application portent après coup
`?session=w-11 … w-14`.

⚠️ **Le compteur `clôture de session amorcée` vaut 0 sur toute la fenêtre de
l'épreuve — et ce zéro est VIDE DE SENS ici.** Aucune session n'était établie ;
il n'y avait rien à clore. Lire ce 0 comme une réussite du critère serait
exactement l'erreur d'énoncé que ce dépôt documente.

⚠️ **Ce relevé ne réfute pas non plus la fenêtre de reprise de
`SourceDistante`.** Les enfants étaient bloqués dans `lire_trame` à l'attache
initiale, donc **jamais entrés** dans la boucle de transport : le chemin
`FenetreCanal` + `rattacher` n'a **pas été atteint**, ni exercé, ni éprouvé.
Le critère 2 échoue **en amont** de ce qu'il visait à mesurer.

### 3.6 Le critère 3 n'a pas été mesuré

Aucun palier ne diffusait ; il n'y avait donc pas de cadence de produit à
relever. Les seuls chiffres de cadence de cette recette
(`cadence du capteur … 0.7` / `cadence de la piste vidéo … 0.5`) proviennent
d'un **binaire de diagnostic modifié**, sur un Bloc-notes **immobile** — et
Desktop Duplication ne rend une image qu'au changement du bureau, ce qui suffit
à expliquer un chiffre inférieur à 1 i/s sans rien dire du coût du trajet IPC.
**Ces chiffres ne sont pas une mesure du critère 3 et ne doivent pas être cités
comme telle.**

Le « avant » sur `7d7e254` n'a pas été joué (§1).

---

## 4. Ce que cette recette N'établit PAS

- **Aucun taux, nulle part.** Une exécution par critère. Ni fréquence d'échec,
  ni variabilité.
- **Rien de la latence, rien de la cadence en conditions de produit**, rien de
  la durée (la plus longue exécution utile fait ≈ 224 s).
- **Aucun plafond du système n'a été approché** : ni les 8 encodeurs, ni les
  10 sorties virtuelles. La montée s'arrête sur `CAPACITE`, une constante du
  produit.
- **Les 8 duplications et 8 encodeurs sont des CONSTRUCTIONS**, jamais alimentées
  ensemble : aucune image comptée sur ces huit voies, aucune unité H.264
  décodée ni regardée.
- **Le mécanisme du défaut du §3.2 est inconnu**, et l'hypothèse la plus
  naturelle est contrariée par une observation (§3.3).
- **Le chemin de reprise de `SourceDistante` n'a jamais été atteint** : ni
  validé, ni invalidé.
- **Le comportement du capteur au-delà de 8 fenêtres est inconnu**, et le
  comportement d'un capteur qui meurt alors que des sessions **diffusent**
  réellement l'est aussi — c'est précisément ce que le critère 2 devait
  mesurer.
- **Rien du redimensionnement, du recouvrement, du déplacement de fenêtre**,
  ni de l'audio, ni de l'injection clavier.
- **Une seule application** (Bloc-notes), **immobile**. Aucune charge
  d'encodage réelle.
- **Rien du comportement quand Apollo consomme le même vivier de sorties.**
- **La fraîcheur du binaire** est adossée à `build-agent-reference.log`
  (`Finished release profile [optimized]`) et à un arbre `git status` propre
  sous `agent/` au moment de la compilation — **cohérent, non prouvé** par un
  horodatage versé.

---

## 5. Pièges rencontrés

- **Un `BufWriter` transforme une poignée de main en pari sur le trafic.** Le
  défaut du §3.1 n'existe que parce que la réponse d'attache voyageait dans le
  même tampon que les images : elle ne partait qu'à condition qu'il y ait des
  images. **Toute réponse de protocole doit être vidée à l'écriture**, sans
  dépendre de ce qui suit.
- **Une fenêtre immobile ne produit RIEN.** Desktop Duplication n'émet qu'au
  changement du bureau — piège déjà documenté pour les bancs à mires, et qui
  frappe ici le chemin de production. **Une recette qui veut des images doit
  animer sa source** ; ce document ne l'a pas fait, et c'est une faiblesse de
  son protocole autant qu'un révélateur du défaut.
- **Le premier symptôme désignait le mauvais coupable.** Le correctif du §3.1
  est juste et n'a rien débloqué : le blocage s'est simplement *déplacé en
  amont*, et le journal a **cessé** d'afficher une ligne qu'il affichait avant.
  **Une trace qui disparaît après un correctif est une information, pas une
  régression de journalisation.**
- **Un diagnostic qui change deux choses n'établit rien** : le tirage
  `defaut-canal-5` (fil lecteur endormi) a rendu les commandes sans réponse en
  plus de retarder la lecture, ce qui a cassé la session pour une raison
  étrangère à la question. Versé, mais exclu du raisonnement.
- **La VM s'est mise en veille prolongée EN PLEINE MESURE**, à
  `2026-08-02T18:05:41Z` (`qemu-system-x86_64: terminating on signal 15`), une
  exécution perdue. Trois processus `agent` ont **survécu** au redémarrage et
  ont dû être tués avant la mesure suivante — le piège documenté, rencontré tel
  quel. Deux extinctions relevées sur la journée : `10:04:03` et `18:05:42` UTC.
- **Le journal du pilote n'est pas de l'UTF-8 sans précaution** : les lignes que
  `run-agent.sh` renvoie de PowerShell portent des octets de contrôle isolés
  (`Op\x02ration r\x02ussie`). Les fichiers versés ont été assainis ; c'est le
  défaut « à deux réglages » déjà connu, ici sur le chemin de l'**hôte** et non
  de la VM.
- **Un Bloc-notes fait avancer le compteur de sessions de deux.** Les sessions
  vont `w-2, w-4, w-6 …` : une seconde fenêtre éligible et fugace est détectée
  par lancement. Sans conséquence ici (les pages ouvertes, elles, sont bien au
  nombre attendu), **mais compter les lancements plutôt que les fenêtres
  induirait en erreur** — la leçon de Paint, sous une autre forme.

---

## 6. État de la VM à la fin

Contrôle depuis un **processus neuf**, comparé par **ensemble de noms** :

| Moment | Ensemble des sorties attachées |
| --- | --- |
| avant la recette (`topologie-initiale.log`) | `{ \\.\DISPLAY1 }` |
| après la recette (`topologie-finale.log`) | `{ \\.\DISPLAY1 }` |

Identique, nom pour nom. Deux purges autonomes ont été nécessaires en cours de
route (`retirees=8` puis `retirees=4`, `purge-finale.log`), les processus ayant
été tués net : **la libération des sorties n'a donc pas été exercée par le
chemin nominal**, et cette restauration prouve seulement qu'aucune sortie ne
subsiste — pas que le chemin de libération fonctionne.

Aucun processus `agent` ne subsiste. VM en cours d'exécution à la clôture.

---

## 7. Ce qu'il reste à régler

1. **Faire aboutir l'écriture du capteur** (§3.2, §3.4). **C'est la seule chose
   qui bloque, et elle bloque tout.** Tant qu'elle n'est pas réglée, D4 ne peut
   pas être reçu, et aucun des trois critères ne peut être rejoué.
2. **Rejouer les trois critères** ensuite, intégralement — aucun n'a de relevé
   exploitable aujourd'hui.
3. **Animer la source** dans le protocole de recette. Un Bloc-notes immobile ne
   produit aucune image : même une fois le canal réparé, un relevé de cadence
   sur une source figée ne mesurerait rien.
4. **Trancher la valeur de `CAPACITE`** sur une mesure, et non sur la
   coïncidence avec un plafond d'encodeurs connu d'ailleurs. Le 8 actuel n'a
   été confronté à rien.
5. **Reporté tel quel** : couche du plafond de 4 processus toujours inconnue,
   mécanisme de l'abandon du mutex DXGI toujours inexpliqué, mise en sommeil des
   fenêtres masquées toujours conjecturale, partage de la capacité réseau entre
   N flux (D6) toujours dû, chemin d'extinction propre du superviseur toujours
   jamais exercé.

---

## 8. Contrôle de la dette de taille de fichier

Relevé par la commande de `CLAUDE.md`, le 2 août 2026 en fin de recette.

| Fichier | Lignes | Écart au tableau de `CLAUDE.md` |
| --- | --- | --- |
| `agent/src/encode.rs` | 1536 | conforme |
| `agent/src/windows_source.rs` | 648 | conforme |
| `agent/src/wasapi.rs` | 543 | conforme |

**Aucun autre fichier de code source ne dépasse 500 lignes.** Le seul fichier
touché par cette tâche, `agent/src/capteur/fenetre.rs`, passe de 169 à
**182 lignes** : marge très large.

Marges étroites relevées **par la commande, pas recopiées** :

| Fichier | Lignes | Marge |
| --- | --- | --- |
| `agent/src/encode/arret.rs` | 500 | **0** |
| `agent/src/capture.rs` | 496 | 4 |
| `agent/src/superviseur/boucle.rs` | **491** | 9 — *la conception annonçait 485* |
| `agent/src/superviseur/table.rs` | 489 | 11 |
| `agent/src/capteur/distante.rs` | **487** | **13** — fichier NEUF de D4, à surveiller |
| `agent/src/demarrage.rs` | **472** | 28 — *la conception annonçait 468* |

⚠️ **`capteur/distante.rs` à 487 lignes est une marge étroite qu'aucun document
ne signalait encore.** C'est le fichier qui portera le remède du §3.4 s'il passe
par la reprise : **toute addition substantielle y appelle une extraction.**

---

## 9. Vérification finale

- `cargo test` : **321 passés, 0 échec**.
- `cargo check --target x86_64-pc-windows-gnu` : sortie 0, **9 avertissements
  `dead_code`**, tous préexistants (aucun sur le fichier modifié).
- `scripts/build-agent.sh` avec `.env` sourcé : `Finished release profile
  [optimized]`, 9 avertissements, 0 erreur (`build-agent-reference.log`).
- Topologie DXGI restaurée nom pour nom (§6).
- Aucun processus `agent` résiduel sur la VM.
- Journaux versés : **17 fichiers** sous `journaux-multifenetres-d4/` —
  13 journaux et 4 pièces d'instrument (`instrument/`), **tous en UTF-8**.
