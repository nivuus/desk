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

> # ⚠️ CE DOCUMENT PORTE DEUX RECETTES, ET LA SECONDE RENVERSE LA PREMIÈRE
>
> Les §0 à §9 sont la **première recette** (tâche 9, binaire `f2b3fbe`). Ils
> restent versés tels quels : leur §3 porte le **diagnostic du défaut de canal**,
> qui est vrai, qui a permis la correction, et qui est le fait le plus utile de
> ce sous-bloc.
>
> Les §10 et suivants sont la **seconde recette** (tâche 11, binaire `ddf2915`,
> après la réécriture de la couche de transport du canal par la tâche 10). Elle
> a été jouée le même jour, avec les quatre corrections de protocole que la
> première s'était elle-même prescrites.
>
> **Les trois critères y sont tenus.** Chaque affirmation des §0 à §9 que la
> seconde recette réfute est annotée en place — mais **lire les §0 à §9 seuls
> donne une image fausse de l'état du dépôt**.

---

## 0. Le verdict

> ⚠️ **VERDICT DE LA PREMIÈRE RECETTE (tâche 9), RENVERSÉ PAR LA SECONDE.**
> Le §10 porte le verdict à jour : **les trois critères sont tenus**, 8 fenêtres
> diffusent simultanément, tuer le capteur ne tue aucune session, et la cadence
> est relevée des deux côtés du changement.

**Aucun des trois critères de réception n'est tenu, et la cause est unique :
le canal entre le capteur et ses enfants ne délivre pas sa réponse d'attache.
Aucune session WebRTC ne s'établit, sur aucun rang.**

Ce n'est pas un résultat partiel qu'on pourrait arrondir. Sur les deux
exécutions de recette jouées sur le binaire de la branche, le compteur de pages
navigateur qui **diffusent réellement** (images décodées en croissance) vaut
**0** à tous les rangs, de 1 à 8 fenêtres.

| Critère | Verdict **de la première recette** | Le chiffre | Verdict à jour (§10) |
| --- | --- | --- | --- |
| 1 — dépasser quatre fenêtres **diffusant**, et nommer le plafond suivant | **NON TENU** | **0 fenêtre diffuse**, à tous les rangs de 1 à 8 | **TENU** — 8 fenêtres diffusent |
| 2 — tuer le capteur ne tue aucune session | **NON TENU** | les **4** enfants meurent en `ExitStatus(1)` à l'instant même de la mise à mort | **TENU** — 0 enfant terminé |
| 3 — la cadence relevée avant et après | **NON MESURÉ** | sans « après », un « avant » seul n'a pas de référent — il n'a pas été joué | **MESURÉ** — 61,8 → 58,3 i/s |

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

> ⚠️ **La seconde recette CORRIGE cette phrase sur un point précis : un plafond
> du système A été approché, et il s'est manifesté.** Le refus au rang 9 reste
> bien celui de `CAPACITE` — cela ne change pas. Mais **à 8 fenêtres qui
> diffusent réellement**, l'adaptation par la résolution est **refusée
> 18 fois sur 18** avec `MF_E_UNSUPPORTED_D3D_TYPE` à `SetOutputType` de
> l'encodeur H.264, alors qu'à 2 fenêtres elle réussit **3 fois sur 3**. Voir
> §13. Ce que la première recette ne pouvait pas voir : ses huit encodeurs
> n'étaient que des **constructions**, jamais reconfigurés à chaud.

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

> ✅ **Cette réserve est LEVÉE par la seconde recette.** Les huit encodeurs d'un
> **unique** processus ont été alimentés ensemble : **494,4 i/s cumulées**
> (51,6 à 66,2 i/s par fenêtre, huit sessions, `r2-critere1-montee-agent.log`),
> et le navigateur a décodé de vraies images H.264 sur les huit voies. Voir
> §11. La réserve de la mesure ② du 31 juillet — « aucune image soumise sur un
> périphérique D3D11 unique et partagé » — tombe avec elle.

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

> ✅ **CE DÉFAUT EST CORRIGÉ**, par la tâche 10 (commits `716eda9` et
> `ddf2915`) : deux connexions par fenêtre — une par sens —, un fil écrivain
> dédié côté capteur, et une borne de 12 s sur l'attente d'une réponse de
> commande. La seconde recette le démontre corrigé **en conditions de produit**
> (§11). ⚠️ **Le MÉCANISME du défaut reste inconnu** : la correction le rend
> impossible par construction, elle ne l'explique pas. Le §3.3 ci-dessous garde
> donc toute sa valeur, et l'asymétrie qu'il relève n'est **toujours pas**
> expliquée.

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

> ✅ **Le chemin `FenetreCanal` + `rattacher` A ÉTÉ ATTEINT ET EXERCÉ** par la
> seconde recette : quatre sessions qui diffusaient réellement ont vu leur
> capteur mourir, et les quatre se sont rattachées au capteur relancé en
> **538 à 689 ms**, sans qu'aucun enfant ne meure (`delta_enfant_termine = 0`).
> Voir §12.

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

> ✅ **Le « avant » sur `7d7e254` a été joué par la seconde recette**, sur une
> source animée, à 4 fenêtres, avec un binaire recompilé pour l'occasion
> (8 861 184 octets contre 9 035 264 pour HEAD). Le critère 3 est **mesuré des
> deux côtés** : §13.

---

## 4. Ce que cette recette N'établit PAS

> ⚠️ **Cette liste est celle de la PREMIÈRE recette.** La seconde en lève
> plusieurs points (plafond du système approché, images comptées, chemin de
> reprise atteint, cadence de produit, source non immobile) et **en laisse
> plusieurs entiers**. La liste à jour est au **§15**, et c'est elle qu'il faut
> lire ; celle-ci n'est conservée que pour dater ce qui était su quand.

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

> ✅ **Les points 1, 2 et 3 sont FAITS** (tâches 10 et 11) : l'écriture du
> capteur aboutit, les trois critères ont été rejoués intégralement, et la
> source de recette est désormais animée. **Le point 4 est fait à moitié** :
> `CAPACITE = 8` a été confronté à une mesure, et le 8 tient pour la *capture*
> — mais la **reconfiguration à chaud des encodeurs**, elle, ne tient pas à 8
> (§13). La suite à donner à jour est au **§16**.

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

---
---

# SECONDE RECETTE (tâche 11) — les trois critères, sur le binaire de la tâche 10

**Date** : 2 août 2026, en fin de journée
**Binaire mesuré** : `ddf2915`, `C:\dev\target\release\agent.exe`,
**9 035 264 octets**, horodaté `2026-08-02T22:01:42` (heure VM) pour les
critères 1, 2 et 3-après ; **rebâti à l'identique en taille** à
`22:26:37` après l'aller-retour du critère 3 (§13).
**Journaux** : préfixe `r2-` sous `journaux-multifenetres-d4/`, **tous en
UTF-8**, séquences ANSI retirées. **Les journaux de la première recette n'ont
pas été écrasés.**

## 10. Le verdict de la seconde recette

**Les trois critères de réception sont tenus.**

| Critère | Verdict | Le chiffre, **relevé** |
| --- | --- | --- |
| 1 — dépasser quatre fenêtres **diffusant**, et nommer le plafond suivant | **TENU** | **8 fenêtres diffusent simultanément** ; le rang 9 est refusé par `CAPACITE = 8`, une constante du produit |
| 2 — tuer le capteur ne tue aucune session | **TENU** | **0** enfant terminé, **0** clôture de session, **4/4** rattachements en 538 à 689 ms |
| 3 — la cadence relevée avant et après | **MESURÉ** | **61,76 → 58,30 i/s** par fenêtre à N = 4 (moyennes **calculées** sur 4 fenêtres) |

**Ce que la tâche 10 a réparé est réparé.** Le défaut du §3.2 — l'écriture du
capteur qui n'aboutissait pas — ne se manifeste plus : les sessions s'établissent
en quelques secondes, l'ICE passe à `connected`, et de vraies images H.264 sont
décodées par le navigateur sur toutes les voies, à tous les rangs.

⚠️ **Aucun taux, nulle part.** Une exécution par critère, plus une exécution
pour l'épreuve du §13.2. Ce document ne porte aucune fréquence d'échec, aucune
variance, aucun intervalle.

⚠️ **Un défaut neuf est trouvé, et il n'est pas mince** : à 8 fenêtres,
l'adaptation par la résolution est **entièrement inopérante** (§13.2). Il ne
casse aucune session, mais il retire à 8 fenêtres l'outil dont le chantier C
volet 1 avait fait la réponse à la congestion.

### Les quatre corrections de protocole exigées

1. **Source animée** — une fenêtre Chrome `--app` sur `anim-d4.html`, dont un
   `canvas` se redessine à chaque `requestAnimationFrame`. Mesuré **90,0 Hz de
   rAF** sur la VM avant la recette (capture d'écran prise **hors mesure**,
   `instrument/anim-d4.html`). Une fenêtre unique par instance, vérifiée par
   `listefen`. Le Bloc-notes immobile de la première recette est écarté.
2. **Ceinture éprouvée** — §12.2. `commander` rend une **erreur**, il ne
   suspend pas.
3. **Survie de la VM contrôlée après chaque rang** — `SURVIE VM (…)
   virsh="en cours d'exécution" acces_partage=OUI` sur chaque rang des trois
   phases, par un **accès réel** au partage et non par la présence de l'entrée
   de montage. **Aucune hibernation pendant cette recette.**
4. **Le « avant » du critère 3 est joué**, sur `7d7e254` recompilé (§13).

## 11. Critère 1 — huit fenêtres diffusent

`r2-critere1-montee-pilote.log`, `r2-critere1-montee-agent.log`. Fenêtres
ouvertes **une par une après le superviseur**, chaque rang attendu **sur le
fait** (l'apparition d'une page navigateur), jamais sur une durée.

« DIFFUSENT » = nombre de pages dont `framesDecoded` **augmente** entre deux
relevés `getStats` espacés de 5 s.

| Rang | Pages | **Diffusent** | i/s par fenêtre (min–max) | Somme des i/s |
| --- | --- | --- | --- | --- |
| 1 | 1 | **1** | 86,01 | 86,0 |
| 2 | 2 | **2** | 66,13 – 82,30 | 148,4 |
| 3 | 3 | **3** | 83,85 – 84,66 | 252,4 |
| 4 | 4 | **4** | 61,17 – 67,82 | 262,0 |
| 5 | 5 | **5** | 0,79 – 47,93 | 159,2 |
| 6 | 6 | **6** | 13,43 – 42,85 | 206,8 |
| 7 | 7 | **7** | 10,76 – 34,65 | 198,8 |
| **8** | **8** | **8** | 22,90 – 35,85 | 245,4 |
| 9 | 8 | 8 | — | refus |

Les colonnes « i/s » sont **relevées** par le pilote (delta de `framesDecoded`
sur delta d'horloge de la page) ; les sommes sont **calculées**.

```
[   93.3s 2026-08-02T20:12:47.002Z] RANG 5 : pages=5 DIFFUSENT=5 ouverte=true
[  136.4s 2026-08-02T20:13:30.019Z] RANG 8 : pages=8 DIFFUSENT=8 ouverte=true
[  229.7s 2026-08-02T20:15:03.324Z] RANG 9 : pages=8 DIFFUSENT=8 ouverte=false
```

**Le rang 5 est le fait qui compte.** D2 puis D3 avaient établi un plafond de
**quatre processus** concurrents tenant une duplication DXGI, et D3 avait
désigné la capture mutualisée comme la voie pour le contourner. **Elle le
contourne** : la cinquième fenêtre diffuse, et les trois suivantes aussi.

### 11.1 Ce que le capteur unique tient réellement

`r2-critere1-montee-agent.log`, marqueurs relevés au rang 8 :

| Ligne comptée | Compte |
| --- | --- |
| `capteur lancé` | **1** |
| `sortie virtuelle créée` | 8 |
| `enfant lancé` | 8 |
| `fenêtre attachée au capteur` | **8** |
| `clôture de session amorcée` | **0** |
| `ERROR` | **0** |
| `accès à la duplication perdu, réouverture` | 28 |
| `0x887a0026` (mutex abandonné) | 28 |

Les 28 pertes d'accès sont la conséquence connue depuis D1 de chaque création
de sortie virtuelle ; la reprise de D2 les encaisse toutes, **et aucune session
n'en meurt** — dans un seul processus cette fois, ce qui n'avait jamais été
mesuré.

**Les huit encodeurs sont ALIMENTÉS ENSEMBLE**, ce qui n'avait jamais été
obtenu — ni par la mesure ② du 31 juillet (constructions seules, périphérique
unique), ni par la première recette. Dernier relevé de chaque session au palier
à 8 fenêtres :

```
cadence du capteur session=w-8  images=635 cadence="63.5"
cadence du capteur session=w-4  images=517 cadence="51.6"
cadence du capteur session=w-14 images=652 cadence="65.2"
cadence du capteur session=w-10 images=603 cadence="60.2"
cadence du capteur session=w-6  images=663 cadence="66.2"
cadence du capteur session=w-2  images=658 cadence="65.8"
cadence du capteur session=w-12 images=615 cadence="61.4"
cadence du capteur session=w-16 images=606 cadence="60.5"
```

soit **494,4 i/s cumulées (calculé)** capturées **et encodées** dans un
**unique** processus, sur huit sorties de 1280×720. Le compteur de l'enfant
s'apparie au sien à moins de 0,2 i/s près sur les huit voies
(`cadence de la piste vidéo (côté enfant)` : 51,6 / 60,2 / 60,6 / 61,6 / 63,4 /
65,3 / 65,7 / 66,3) : **le trajet IPC ne perd rien de mesurable**.

Le navigateur, lui, ne décode que **224,9 i/s cumulées (calculé)** au palier
final de 30 s (17,00 à 36,61 i/s par fenêtre). **L'écart n'est pas dans le
canal, il est en aval** : huit sessions demandant chacune ~10 Mb/s sur le même
pont, RTT relevé montant de 2 ms (N = 1) à 104 ms (N = 7). **Cette attribution
est une INFÉRENCE** — aucune mesure de charge du pont n'a été prise.

### 11.2 Le rang qui arrête la montée est un refus du produit

Comme à la première recette, et pour la même raison :

```
statut="« C:\Windows\System32\WindowsPowerShell\v1.0\powershell.EXE » n'a pas
pu s'ouvrir : plus aucune sortie virtuelle disponible."
```

C'est `Effet::AnnoncerRefus` de `Table::detecter` sur `CAPACITE = 8`
(`agent/src/superviseur/boucle.rs:58`). ⚠️ **Le libellé nomme une fenêtre
PowerShell fugace et non la fenêtre Chrome demandée** : plusieurs fenêtres ont
été refusées après le rang 8, et le statut ne porte que la dernière. Le relevé
des fenêtres de la VM au rang bloquant montre la neuvième fenêtre Chrome
**restée sur le bureau physique**, jamais placée :

```
30668	chrome	0x10422	1280x720+130+130	Anim 9
16544	chrome	0x31022a	1280x720+2400+0	Anim 1
26888	chrome	0x17b014e	1280x720+3680+0	Anim 2
…  (Anim 3 à 8 à x = 4960, 6240, 7520, 8800, 10080, 11360)
```

les huit premières étant chacune sur sa sortie virtuelle, tuilées à droite du
bureau physique de 2400 px.

**Le plafond suivant n'est donc TOUJOURS PAS nommé côté capture** : la montée
s'arrête sur une constante du produit, pas sur un refus du système. Le relevé
n'en dit rien, et `CAPACITE` n'a pas été relevée pour cette recette — un agent
de recette ne modifie pas le produit qu'il mesure.

## 12. Critère 2 — tuer le capteur ne tue aucune session

`r2-critere2-capteur-pilote.log`, `r2-critere2-capteur-agent.log`. Quatre
fenêtres **qui diffusent réellement** (41,74 à 60,37 i/s au relevé « avant »),
capteur tué **par PID relevé** (`Stop-Process -Id 34340`, jamais `pkill -f`).

### 12.1 Le relevé

| Événement | Horodatage | Écart |
| --- | --- | --- |
| mise à mort demandée (horloge du **pilote**, hôte) | `2026-08-02T20:18:52.971Z` | — |
| `capteur terminé pid=34340` (horloge de la **VM**) | `20:18:54.096089Z` | — |
| `capteur lancé pid=32156` | `20:18:54.097487Z` | +1,4 ms |
| `capteur mort, relancé pid_mort=34340 pid_neuf=32156` | `20:18:54.097503Z` | +1,4 ms |
| `canal rattaché au capteur` — w-2 | `20:18:54.635840Z` | **+538 ms** |
| — w-4 | `20:18:54.642254Z` | **+545 ms** |
| — w-6 | `20:18:54.654884Z` | **+557 ms** |
| — w-8 | `20:18:54.786155Z` | **+689 ms** |

⚠️ **Les écarts de la colonne de droite sont INTERNES à l'horloge de la VM**
(depuis `capteur lancé`). L'écart entre la demande du pilote et le constat de
mort — 1,125 s — mêle un aller-retour WinRM et un décalage d'horloge entre
l'hôte et la VM **qui n'a pas été mesuré** : ne pas le lire comme une latence.

Bilan de la fenêtre de l'épreuve, comparé entre le relevé « avant » et le relevé
« après » (donc **fenêtré**, jamais un total de fichier) :

| Compteur | Delta |
| --- | --- |
| `clôture de session amorcée` | **0** |
| `enfant terminé` | **0** |
| `enfant lancé` | **0** |
| `canal rattaché au capteur` | **4** |
| `fenêtre attachée au capteur` | **4** |
| `capteur mort, relancé` | 1 |

**Aucun enfant n'est mort, aucune session n'a été close, aucune session neuve
n'a été créée** — les quatre pages navigateur gardent leurs identifiants
`w-2, w-4, w-6, w-8`. C'est exactement l'inverse de la première recette, où les
quatre enfants mouraient en `ExitStatus(1)`.

Et l'image revient : cadence par fenêtre **après** la relance, sur 10 s,
**52,55 / 62,86 / 64,74 / 66,73 i/s** — contre **41,74 / 44,89 / 57,88 /
60,37** avant. ⚠️ **Ne pas lire ces deux séries comme « c'est plus rapide
après »** : ce sont deux fenêtres de 10 s sur un lien partagé, et rien
n'établit que l'écart soit autre chose que du bruit.

### 12.2 La ceinture : `commander` rend une erreur, il ne suspend pas

C'est le point que la tâche 10 laissait explicitement à la mesure — la
documentation de `Canal::commander` (`agent/src/capteur/tube.rs`) le dit mot
pour mot : « **À vérifier explicitement à la recette** : tuer le capteur pendant
que des commandes circulent, et constater que `commander` rend une erreur. »

Des commandes circulaient : le contrôleur d'adaptation appelle `set_bitrate`
**à chaque décision**, soit environ une par seconde et par session. **96,7 ms
après la relance du capteur**, l'enfant qui a écrit sur le tube mort obtient une
erreur, et il la journalise :

```
2026-08-02T20:18:54.194167Z  WARN agent::transport::adaptation:
  l'encodeur refuse le réglage du débit à chaud
  erreur=Le canal de communication est sur le point d’être fermé. (os error 232)
```

`os error 232` est `ERROR_NO_DATA` — « le tube est en cours de fermeture ». La
parade prévue par la conception (le capteur ferme ses handles en mourant, ce qui
fait **échouer** la lecture au lieu de la suspendre) **fonctionne**, et elle est
observée sur le chemin réel.

**Portée exacte, à ne pas élargir** :

- **une seule mise à mort**, sur une seule exécution : **aucun taux** ;
- la commande fautive a été émise **après** la mort du capteur, pas pendant son
  vol. Le cas adverse — une commande écrite, en attente de réponse, à l'instant
  précis de la mort — n'a **pas** été isolé ; on ne peut pas le viser à la
  milliseconde avec une commande par seconde. Ce qui est prouvé, c'est que la
  **lecture sans délai** de `commander` sur un tube dont le serveur est mort
  **rend une erreur** ;
- **la borne de 12 s côté capteur** (`DELAI_REPONSE_FENETRE`, l'autre moitié de
  la ceinture) n'a **pas** été exercée : aucune expiration n'apparaît dans les
  journaux. Elle reste du code jamais couru.

## 13. Critère 3 — ce que coûte le trajet IPC, et un défaut neuf

### 13.1 Le relevé, avant et après

Deux exécutions **à protocole identique** (`PHASE=cadence`, N = 4, même source
animée, même palier de 30 s), sur deux binaires :

| | **avant** — `7d7e254`, l'enfant capture lui-même | **après** — `ddf2915`, source distante |
| --- | --- | --- |
| binaire | 8 861 184 o, `22:22:55` | 9 035 264 o, `22:01:42` |
| i/s par fenêtre (**relevé**) | 60,06 / 62,14 / 62,31 / 62,52 | 57,43 / 58,31 / 58,40 / 59,07 |
| moyenne (**calculé**) | **61,76** | **58,30** |
| somme (**calculé**) | 247,0 | 233,2 |
| débit moyen par fenêtre (**relevé**) | 9 538 kb/s | 9 309 kb/s |

**Écart : −3,46 i/s par fenêtre, soit −5,6 % (calculé).**

⚠️ **Ce qu'il ne faut PAS conclure de ce chiffre.** Une exécution par côté :
**aucun taux, aucune variance**. Les deux exécutions se suivent sur un lien
partagé dont la charge n'est pas contrôlée, et l'écart de 5,6 % est du même
ordre que la dispersion **intra-série** du critère 2 (41,7 à 60,4 i/s entre
quatre fenêtres d'une même exécution). **Ce relevé borne l'ordre de grandeur du
coût ; il ne le mesure pas.** Ce qu'il établit solidement, en revanche, c'est
qu'il n'y a **pas** d'effondrement : la mutualisation ne divise pas la cadence.

Le côté agent conforte cette lecture sur l'exécution « après » : le capteur
produit 82,6 à 85,4 i/s par fenêtre et l'enfant en écrit 82,6 à 85,3 —
**identiques à 0,1 i/s près**. Le trajet capteur → enfant ne perd rien ; ce que
le navigateur ne voit pas se perd en aval, dans le réseau.

⚠️ Le binaire « avant » **n'a pas de compteur `cadence du capteur` ni
`cadence de la piste vidéo`** (ils datent de la tâche 8) : la comparaison des
deux côtés n'existe **que** pour l'après. Le seul chiffre opposable des deux
côtés est celui du navigateur.

### 13.2 Le défaut neuf : à 8 fenêtres, l'adaptation par la résolution ne marche plus

Sur la montée du critère 1, **18 changements de taille d'encodage sont refusés,
et 0 réussi** :

```
2026-08-02T20:13:21.175810Z  WARN agent::transport::adaptation:
  changement de taille d'encodage refusé, barreau conservé
  erreur=le capteur a refusé : configuration du type de sortie de l'encodeur
  H.264 (transform matériel): Le type d’entrée n’est pas pris en charge pour
  le périphérique D3D. (0xC00D6D76) largeur=1024 hauteur=576
```

`0xC00D6D76` est `MF_E_UNSUPPORTED_D3D_TYPE`, sur le `SetOutputType` de
l'encodeur H.264 — **exactement l'appel et le code que la mesure ② du
31 juillet 2026 avait identifiés comme le point de refus du 9ᵉ encodeur**, et
que `CLAUDE.md` documente déjà comme « le libellé parle de l'entrée alors que
l'appel règle la sortie ».

**Le premier refus tombe 0,56 s après l'attache de la HUITIÈME fenêtre** :

```
508: 20:13:20.615584Z  fenêtre attachée au capteur session=w-16 …
516: 20:13:21.175810Z  changement de taille d'encodage refusé …
```

**L'épreuve différentielle** (`r2-epreuve-taille-n2-*.log`) : même binaire, même
code, **2 fenêtres** au lieu de 8, sous une dégradation `netem adsl` (8 Mb/s,
30 ms) posée pour forcer le contrôleur à descendre de barreau. Résultat :

```
20:27:56.736361Z  INFO … taille d'encodage changée largeur=640  hauteur=360
20:28:06.328901Z  INFO … taille d'encodage changée largeur=1024 hauteur=576
20:28:36.343225Z  INFO … taille d'encodage changée largeur=1280 hauteur=720
```

**3 succès, 0 refus**, et le navigateur rapporte bien des trames `640x360`. Le
chemin `set_encode_size` **à travers le capteur fonctionne**.

**Attribution, et sa réserve.** Deux variables diffèrent entre les deux
exécutions : le nombre de fenêtres (8 contre 2) **et** la dégradation `netem`.
La dégradation détermine **si** le contrôleur demande un changement — elle ne
peut pas déterminer si l'encodeur l'accepte, et l'erreur relevée est un refus de
**construction** d'encodeur, pas un refus de débit. Sous cette réserve, la
lecture est : **reconfigurer un encodeur en construit transitoirement un
neuf** ; à 8 encodeurs vivants le neuvième est refusé, exactement comme au
31 juillet. **La couche qui impose ce 8 reste inconnue** — D4 ne l'éclaire pas
davantage que D3.

**Conséquence produit, à ne pas minimiser** : à `CAPACITE = 8` fenêtres, la
seule réponse à la congestion qui reste est le **débit**, la résolution étant
gelée. C'est la moitié du dispositif du chantier C volet 1 qui disparaît au rang
maximal, sans qu'aucune session ne meure et sans que rien d'autre qu'un `WARN`
ne le signale.

## 14. État de la VM à la fin

Contrôle depuis un **processus neuf**, comparé par **ensemble de noms** :

| Moment | Ensemble des sorties attachées | Journal |
| --- | --- | --- |
| avant la seconde recette | `{ \\.\DISPLAY1 }` | `r2-topologie-initiale.log` |
| après le critère 1 | `{ \\.\DISPLAY1 }` | `r2-topologie-apres-critere1.log` |
| après tout, avant purge | `{ \\.\DISPLAY1, \\.\DISPLAY5, \\.\DISPLAY6 }` | `r2-purge-finale.log` (`avant=3`) |
| après purge | `{ \\.\DISPLAY1 }` | `r2-topologie-finale.log` |

Identique au départ, nom pour nom. **Deux sorties ont dû être purgées** en fin
de recette (`purge terminée retirees=2 avant=3 apres=1`), l'agent de la dernière
exécution ayant été tué net : comme en première recette, **cette restauration
prouve qu'aucune sortie ne subsiste, pas que le chemin de libération
fonctionne**. Ce chemin-là a bien fonctionné ailleurs — après le critère 2, la
topologie est revenue à une seule sortie **sans purge**, et le journal porte
4 `sortie virtuelle rendue au pilote`.

Aucun processus `agent` ne subsiste ; aucun processus `chrome` ne subsiste sur
la VM. VM en cours d'exécution à la clôture, **et aucune hibernation pendant
cette recette** (contrôle après chacun des 9 rangs du critère 1 et de chaque
ouverture des autres phases).

## 15. Ce que la seconde recette N'établit PAS

- **Aucun taux, nulle part.** Une exécution par critère. Ni fréquence d'échec,
  ni variance, ni intervalle.
- **Le plafond suivant côté capture n'est toujours pas nommé** : la montée
  s'arrête sur `CAPACITE = 8`, une constante du produit. Ce que la mesure
  établit, c'est que **8 fenêtres tiennent** — pas que 9 ne tiendraient pas.
- **La couche qui impose le plafond de 8 encodeurs reste inconnue** (NVENC,
  pilote, Media Foundation, virtualisation) — inchangé depuis le 31 juillet.
- **Le mécanisme du défaut de canal de la première recette reste inconnu.** La
  tâche 10 le rend impossible par construction ; elle ne l'explique pas, et
  l'asymétrie relevée au §3.3 n'est toujours pas expliquée.
- **La borne de 12 s côté capteur n'a pas été exercée.**
- **Le cas adverse de la ceinture** — une commande en vol à l'instant exact de
  la mort du capteur — n'a pas été isolé.
- **Rien de la latence bout en bout.** Le critère 3 relève des cadences ; la
  latence exige un instrument qui n'existe pas.
- **Rien de la durée** : la plus longue exécution fait ≈ 268 s, et la session
  la plus longue moins de 5 minutes.
- **Aucune unité H.264 n'a été décodée hors du navigateur, ni regardée.** La
  justesse de l'image n'est **pas** contrôlée : le pilote compte des
  `framesDecoded`, il ne vérifie pas que la fenêtre 3 montre l'application 3.
  *(La correspondance a été vue à l'œil sur une capture d'écran de la VM prise
  **avant** la recette, hors mesure ; ce n'est pas un contrôle.)*
- **Aucun redimensionnement de fenêtre, aucun recouvrement, aucun déplacement**,
  aucune injection clavier, rien de l'audio.
- **Une seule application** (Chrome en mode `--app`), et une seule animation.
  Une page `canvas` plein cadre n'est pas une charge d'encodage représentative
  de toutes les applications.
- **La mort d'un enfant pendant que les autres diffusent** n'est pas exercée ;
  ni la fermeture d'une fenêtre en cours de diffusion.
- **Le chemin d'extinction propre du superviseur n'a jamais été exercé** — les
  agents ont été tués par `Stop-Process`, comme dans tous les sous-blocs
  précédents.
- **Rien du comportement quand Apollo consomme le même vivier de sorties.**
- **La fraîcheur du binaire** est adossée à `r2-build-head-ddf2915.log`, à la
  taille (9 035 264 o) et à l'horodatage relevés sur la VM, et à un
  `git status --porcelain agent/ scripts/` **vide**. **Cohérent, non prouvé.**

## 16. La suite à donner

1. **Le refus de reconfiguration d'encodeur à 8 fenêtres** (§13.2). C'est le
   seul défaut ouvert de ce sous-bloc. Trois pistes, aucune mesurée :
   reconfigurer **en place** plutôt qu'en reconstruisant ; abaisser `CAPACITE`
   à 7 pour garder une place libre ; ou détruire l'ancien encodeur **avant**
   d'en construire un neuf — ce dernier point rejoint la question, ouverte
   depuis le 31 juillet, de savoir si **détruire un encodeur libère la place**,
   qui n'a **jamais** été éprouvée.
2. **Nommer le plafond de capture au-delà de 8**, ce qui suppose de relever
   `CAPACITE` : une décision de conception, pas de recette.
3. **Mesurer la latence**, qu'aucun sous-bloc du chantier D n'a mesurée.
4. **Reporté tel quel** : couche du plafond de 4 processus toujours inconnue,
   mécanisme de l'abandon du mutex DXGI toujours inexpliqué, mise en sommeil des
   fenêtres masquées toujours conjecturale, partage de la capacité réseau entre
   N flux (D6) toujours dû, audio par fenêtre (D6) toujours dû, chemin
   d'extinction propre du superviseur toujours jamais exercé.

## 17. Pièges neufs de la seconde recette

- **Un `rsync -a` qui remonte le temps fait qu'un `cargo build` ne bâtit
  RIEN, et le dit comme un succès.** Pour jouer le « avant », les sources de la
  VM ont été remplacées par celles d'un commit antérieur, puis remises à HEAD.
  `sync-agent.sh` préserve les horodatages : au retour, les sources HEAD étaient
  **plus anciennes** que les artefacts du « avant », et cargo a conclu
  `Finished release profile in 0.13s` — **en gardant le binaire du commit
  précédent**. Un `touch` depuis l'hôte n'y change rien (CIFS ne propage pas
  l'horodatage), un `touch` depuis Windows non plus. **Seul
  `cargo clean --release -p agent` a débloqué**, et la preuve est la taille du
  binaire : 8 861 184 → 9 035 264 octets. **Toujours vérifier la taille et
  l'horodatage du binaire après un aller-retour de sources ; une durée de
  compilation de 0,13 s est un aveu.**
- **`build-agent.sh` lancé depuis un `git worktree` s'arrête EN SILENCE après
  « sources synchronisées »** — le worktree n'a pas de `node_modules`, donc
  `scripts/winrm.js` échoue, et son `2>/dev/null` mange la cause exactement
  comme le piège du `.env` non sourcé déjà documenté. Remède : lier
  `node_modules` dans le worktree.
- **Une source animée doit l'être à une cadence connue, et elle se vérifie.**
  La page de recette affiche son propre débit de `requestAnimationFrame` :
  **90,0 Hz** relevé sur la VM. Sans ce chiffre, une cadence de capture basse
  serait indistinguable d'une source lente.
- **Un `--user-data-dir` par fenêtre est obligatoire** : sans lui, la seconde
  invocation de Chrome rejoint la première instance, et la recette compte des
  lancements au lieu de fenêtres — la leçon de Paint, sous une autre forme.
- **Le statut de refus de la page-shell ne porte que le DERNIER refus.** Il
  nommait ici une fenêtre PowerShell fugace, pas la fenêtre Chrome demandée.
  **Croiser avec le relevé des fenêtres de la VM**, qui montre la fenêtre
  restée sur le bureau physique.
- **Copier `agent.log` APRÈS la fin réelle de l'exécution, pas à la fin du
  pilote.** Le pilote copie le journal puis tue son navigateur ; les enfants
  meurent **ensuite**, et les lignes de libération des sorties partent avec le
  journal suivant. Une pièce a été perdue ainsi (la libération nominale après
  le critère 1), rattrapée par un contrôle de topologie.
- **Un compteur de cadence côté enfant qui s'apparie à celui du capteur est le
  bon instrument pour disculper un canal.** C'est lui qui permet de dire que
  l'écart avec le navigateur est **en aval** du canal, et non dedans.

## 18. Contrôle de la dette de taille de fichier — seconde recette

Relevé par la commande de `CLAUDE.md`, le 2 août 2026 en fin de seconde recette.
**Aucun fichier de code source n'a été modifié par cette tâche** : les chiffres
sont donc identiques à ceux du §8.

| Fichier | Lignes | Marge |
| --- | --- | --- |
| `agent/src/encode.rs` | 1536 | dette assumée |
| `agent/src/windows_source.rs` | 648 | dette assumée |
| `agent/src/wasapi.rs` | 543 | dette assumée |
| `agent/src/encode/arret.rs` | 500 | **0** |
| `agent/src/capture.rs` | 496 | 4 |
| `agent/src/superviseur/boucle.rs` | 491 | 9 |
| `agent/src/superviseur/table.rs` | 489 | 11 |
| `agent/src/capteur/distante.rs` | 487 | 13 |
| `agent/src/transport/socket.rs` | 481 | 19 |
| `agent/src/transport/piste_video.rs` | 477 | 23 |
| `agent/src/demarrage.rs` | 472 | 28 |

**Aucun fichier de code source ne dépasse 500 lignes.** Les trois fichiers du
tableau de dette de `CLAUDE.md` sont **conformes**, aux chiffres près qui y sont
inscrits.

⚠️ **Le remède du §16.1 touchera `agent/src/encode.rs` (1536 lignes) ou
`agent/src/capteur/fenetre.rs` (329)** : le premier est en dette gelée, donc
toute addition y appelle une extraction.

## 19. Vérification finale — seconde recette

- **Aucun code de production modifié** : `git status --porcelain agent/ scripts/`
  **vide** avant et après la recette. `cargo test` et
  `cargo check --target x86_64-pc-windows-gnu` n'ont donc pas été rejoués — il
  n'y avait rien de neuf à vérifier.
- `build-agent.sh` : deux compilations versées, `r2-build-avant-7d7e254.log`
  (19 avertissements, binaire 8 861 184 o) et `r2-build-head-ddf2915.log`
  (**9 avertissements**, binaire 9 035 264 o). Le nombre d'avertissements
  distingue les deux états : c'est un contrôle, pas une coïncidence.
- Topologie DXGI restaurée nom pour nom (§14), après purge.
- Aucun processus `agent` ni `chrome` résiduel sur la VM.
- Dégradation `netem` **retirée** : `tc qdisc show dev internalBridge` rend
  `qdisc noqueue`.
- Journaux versés : **16 journaux** à préfixe `r2-` plus **3 pièces
  d'instrument** (`instrument/pilote-recette-d4b.mjs`,
  `instrument/preparer-d4b.ps1`, `instrument/anim-d4.html`), **tous en UTF-8**,
  séquences ANSI retirées. **Les 17 fichiers de la première recette sont
  intacts.**
