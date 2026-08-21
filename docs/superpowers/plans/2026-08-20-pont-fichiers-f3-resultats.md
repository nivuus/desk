# Sous-bloc F3 — le renommage, la suppression, et la casse en lecture

**21 août 2026.** Plan : `2026-08-20-pont-fichiers-f3.md`.
Conception : `../specs/2026-08-19-pont-fichiers-design.md`.
Journaux et instrument : `journaux-pont-fichiers-f3/`.

Binaire mesuré : **10 658 816 octets**, bâti le 21 août 2026 à 07:24 après
`cargo clean --release -p proto -p agent`. Le binaire d'avant la correction du
§4 pesait **10 656 768** — les deux tailles sont relevées, et **toute la
recette a été rejouée sur le second**.

---

## §0 — Le monde d'entrée, et ce qu'il faut constater sans le refaire

La tâche 1 du plan était un contrôle d'entrée : lequel de deux mondes ?
**F2 est FUSIONNÉ.** Constaté, non refait :

- la file d'écritures indexée par chemin (`attend` / `oublier` / `en_vol`) est
  livrée et `dead_code`, **posée pour F3** ;
- les verbes 7 et 8 sont **réservés** à `Renommer` / `Supprimer` ;
- l'extraction de `pont/projfs/rappels.rs` est **faite**, sous les noms que le
  plan emploie.

---

## §1 — Ce que F3 livre

| Étage | Fichier | Nature |
| --- | --- | --- |
| le canonicalisateur | `client/src/fichiers/noms.ts` (**237**) | **PUR** — `plier()` = `normalize('NFC')` **puis** `toLowerCase()`, l'ordre n'étant pas commutatif. Quatre règles : correspondance exacte d'abord, **un** homonyme → le nom **STOCKÉ**, plusieurs → `ambigu`, aucun → `absent` |
| la mutation locale | `client/src/fichiers/mutation.ts` (**379**) | `move()` détecté **à l'appel** ; destination résolue **avant**, dans les deux branches, parce que `move()` ÉCRASE ; renommage de pure casse par un nom intermédiaire **aléatoire** |
| le repli par copie | `client/src/fichiers/copie.ts` (**151**) | `retirerArbre` descend feuille par feuille et ne retire **que ce qui vient d'être copié** : une entrée apparue entre-temps fait ÉCHOUER le retrait au lieu d'être balayée |
| le contrôle de flux | `client/src/fichiers/flux.ts` (**106**) | `SEUIL_TAMPON = 64 KiB` ; les deux écouteurs sont retirés quel que soit le gagnant ; `close` libère aussi ; re-vérification après souscription (la course « tester puis attendre ») |
| les compteurs | `agent/src/pont/compteurs.rs` (**150**) | **PUR** — le **seul** point où une cause devient un `HRESULT` (26 appelants directs de `hresult` ramenés à **0**) |
| l'ordonnancement | `agent/src/pont/mutation.rs` (**231**) | **PUR** — `Pousser` / `AttendreEcrituresDues` / `AbandonnerEcrituresDues`. **Aucune fusion** : `a`→`b` puis `b`→`c` sont deux gestes dont l'ordre EST le sens |
| la fenêtre de lecture | `agent/src/pont/lecture.rs` (**141**) | **PUR** — `MORCEAUX_EN_VOL = 4` ; la borne porte sur le **total** en vol, jamais sur le lot |
| la décision | `agent/src/pont/notifications.rs` (**341**) | masque 7 → **9** bits ; `Cible::{SansObjet, DansLaRacine, HorsRacine}` |

**La règle que F3 apporte et qu'aucun autre document ne portait** : l'idiome
**fichier temporaire + renommage** (LibreOffice, Word, la plupart des éditeurs)
exige que les écritures **dues sur la source** soient poussées **AVANT** le
renommage, et qu'une **suppression RETIRE** les écritures dues sur ce chemin.
Sans les deux, la sauvegarde est perdue et un fichier supprimé réapparaît. La
file de F2, indexée par chemin, existe exactement pour cela.

---

## §2 — Le fait de plateforme n°1 : ProjFS REFUSE le renommage d'un RÉPERTOIRE

**Mesuré, deux exécutions de recette plus la sonde S1** :

```
c1a  renommer un FICHIER    : ok      source ABSENT   cible PRESENT
c1b  renommer un RÉPERTOIRE : ECHEC:Cette demande n’est pas prise en charge.
```

Et le journal d'agent ne porte **aucune** notification `code=32` (`PRE_RENAME`)
pour ce geste : **ProjFS refuse avant de nous consulter.**

🔴 **Le critère ① b — « renommer un répertoire » — n'est donc PAS livrable, et
ce n'est pas notre fait.** C'est un fait de plateforme qu'aucun document du
dépôt ne portait. Le code du pont sait le faire ; la plateforme ne le lui
demande jamais.

---

## §3 — Le fait de plateforme n°2 : la suppression descend enfant par enfant

Un répertoire NON VIDE supprimé produit **une notification par enfant**
(`code=16` puis `code=2048`, de bas en haut, quatre entrées mesurées), et le
poste local a suivi. **R-F3-3 est levé : la suppression non récursive suffit**,
et c'est ce qui permet à `removeEntry` d'être appelée **sans `recursive`** —
un `recursive: true` effacerait un sous-arbre que la VM n'a jamais demandé
d'effacer.

---

## §4 — 🔴 Le défaut trouvé par la mesure : un code que RIEN ne rendait

`Erreur::Abandonnee` était **définie, comptée, traduite en
`ERROR_OPERATION_ABORTED`** — et **rendue par AUCUN site de production**. La
fermeture du canal soldait les commandes **en vol** en `CanalFerme`, pendant
que sa propre ligne de journal, une ligne au-dessus, disait « les commandes en
vol sont abandonnées » : **le code et sa trace se contredisaient.**

**Ce n'est pas une lecture, c'est une mesure.** Le critère ④ exige que les douze
codes soient observés au moins une fois ; la coupure posée sur une commande
**réellement en vol** rendait `canal-ferme=1` et `abandonnee=0` là où le tableau
promettait l'inverse.

| | avant | après |
| --- | --- | --- |
| coupure sur commande en vol | `canal-ferme=1`, `abandonnee=0` | **`abandonnee=1`**, `canal-ferme=0` |
| `delai-depasse` du même relevé | 1 | **0** |

⚠️ **Le second zéro est ce qui rend la mesure concluante** : il établit que la
commande a été coupée **EN VOL** au lieu d'expirer à `DELAI_LISTER`. Sans lui,
un `abandonnee=1` pourrait venir d'un autre chemin.

Les deux instants restent distincts et portent deux codes distincts : en vol →
`ERROR_OPERATION_ABORTED` ; arrivée après → `ERROR_IO_DEVICE`.

---

## §5 — Les quatre critères, avec leur nombre d'exécutions

🔴 **Aucun taux n'est revendiqué nulle part.** Deux exécutions par critère au
mieux ; les rouges sont **une** exécution chacune.

| # | Critère | Verdict | Exéc. |
| --- | --- | --- | --- |
| ① a | renommer un **fichier** | **TENU** — `renomme.txt` porte le sha256 `8c9605738f6e92bf` de `projete.txt`, qui est ABSENT | **2** |
| ① b | renommer un **répertoire** | ⛔ **NON LIVRABLE** — ProjFS refuse avant de nous consulter (§2) | **2** + S1 |
| ② | supprimer un fichier, puis un répertoire non vide | **TENU** — `Casse.txt` ABSENT, `a-effacer/**` ABSENT, témoin `gros-lecture.bin` intact | **2** |
| ③ | couper le canal sur une commande en vol | **TENU** — `close_notify DTLS`, `abandonnee=1`, aucun gel, `dues` retombé à 0 | **2** (dont 1 qui a manqué sa fenêtre, versée) |
| ④ | les douze codes observés | **8 sur 12 sur le binaire livré** — voir le tableau ci-dessous | **6** |

### Le tableau du critère ④, code par code

| Code | Observé | Par quoi |
| --- | --- | --- |
| `introuvable` | ✅ 17 | geste réel |
| `acces-refuse` | ✅ 1 | **injection** |
| `disque-plein` | ✅ 1 | **injection** |
| `non-supporte` | ✅ 2 | geste réel (lien dur) |
| `delai-depasse` | ✅ 1 | **injection** (`.faute-silence`) |
| `inattendue` | ✅ 1 | **dérive fabriquée** (deux homonymes de casse) |
| `protege-en-ecriture` | ✅ **9** | **ROUGE** `PONT_MUTATION=0` |
| `abandonnee` | ✅ 1 | coupure sur commande en vol |
| `chemin-introuvable` | ⚠️ 2, **binaire antérieur** | sonde S1 |
| `canal-ferme` | ⚠️ 1, **binaire antérieur** | coupure à +1500 ms |
| `repertoire-non-vide` | ⛔ **hors d'atteinte** | voir §6 |
| `deja-present` | ⛔ **hors d'atteinte** | voir §6 |

⚠️ **UNE INJECTION PROUVE QUE LA TABLE N'EST PAS DÉCORATIVE ; ELLE NE PROUVE
PAS QUE LA CAUSE EST ATTEIGNABLE EN EXPLOITATION.** Trois des huit codes
observés le sont par injection, et c'est écrit ici plutôt que dilué.

⚠️ **`chemin-introuvable` et `canal-ferme` n'ont été observés que sur le
binaire d'AVANT la correction du §4.** Ils ne sont pas réfutés — le chemin qui
les produit n'a pas changé —, mais **ils ne sont pas relevés sur le binaire
livré**, et le dire est le minimum.

---

## §6 — ⛔ Deux codes de diagnostic sont hors d'atteinte, et la raison est mesurée

`repertoire-non-vide` et `deja-present` disent que **le miroir a dérivé**. Pour
les atteindre, il faut que le poste local porte une entrée que la VM ignore.
La dérive a été **fabriquée délibérément** (§7), et elle **ne parvient jamais
jusqu'à nous** :

- `apres` porte `vide-cote-vm\inconnu-de-la-vm.txt` : **ProjFS montre à la VM
  le contenu que seul le poste local connaît.** Windows voit donc un répertoire
  NON VIDE et refuse la suppression sans nous consulter — et le geste
  **FIGE la mesure** (deux exécutions, relevé bloqué à ce geste) ;
- `Rename-Item a-deplacer.txt -NewName occupe-cote-local.txt` rend
  **« Impossible de créer un fichier déjà existant »** : Windows résout la
  collision, encore une fois avant nous.

**Ce que cela établit** : ces deux codes sont des filets contre une dérive que
ce montage ne sait pas produire. Les laisser manquants au tableau est honnête ;
les faire tomber par un geste qui fige la mesure ne le serait pas.

---

## §7 — La casse : ce que F3 livre, et ce qu'il NE démontre PAS

**Le plan déclarait d'avance que le remède ne serait pas démontré de bout en
bout sur ce montage, et pourquoi. Cette déclaration tient.**

- la moitié **mesurée** du défaut hérité de F1 — NTFS résolvant `casse.txt` sur
  un `Casse.txt` **hydraté**, sans jamais atteindre le pont — est le
  comportement **NORMAL** de Windows, et le canonicalisateur n'y change rien ;
- la moitié **destructrice** est côté navigateur, sur un disque local
  **insensible à la casse**, et **elle n'a JAMAIS été observée** : l'instrument
  est OPFS, dont la sonde S2 mesure qu'il est **sensible** à la casse (deux
  exécutions identiques).

🔴 **Le canonicalisateur est donc livré, testé sur l'hôte, et NON ÉPROUVÉ sur
le disque où il sert.** Ce qui est mesuré, c'est qu'une casse **ambiguë** est
dénoncée (`inattendue=1`) au lieu d'être tranchée au hasard.

---

## §8 — Ce que la sonde S2 mesure du navigateur (2 exécutions identiques)

| Question | Réponse |
| --- | --- |
| `move()` sur un **fichier** | **oui**, même parent et parent différent |
| `move()` sur un **répertoire** | **NON** — d'où le repli par copie |
| `move()` écrase-t-il ? | **OUI, SILENCIEUSEMENT** (`contenu: "APRES"`) — d'où la résolution de la destination **avant** |
| `removeEntry` sans `recursive` sur un répertoire non vide | `InvalidModificationError` |
| OPFS est-il sensible à la casse ? | **OUI** — d'où §7 |

---

## §9 — La ROUGE de ① et ②, et pourquoi elle est un vrai rouge

`PONT_MUTATION=0`, **une exécution** :

```
c1a ECHEC:Une erreur interne s’est produite.  source PRESENT  cible ABSENT
c2a ECHEC:Une erreur interne s’est produite.  apres  PRESENT
c2b ECHEC:Une erreur interne s’est produite.  apres  PRESENT
protege-en-ecriture=9
WARN agent::pont: mutations DESARMEES (PONT_MUTATION=0) : renommage et
     suppression refuses au PRE_, rien n'est pousse
```

**Le mécanisme est PRÉSENT et le résultat ABSENT** : les trois gestes échouent,
la source reste PRÉSENTE, la cible ABSENTE. Les critères ① et ② ne sont donc
pas satisfaits par accident.

⚠️ **Une première rouge a été DISQUALIFIÉE et conservée**
(`s1-0-DISQUALIFIEE-*`) : elle employait `PONT_ECRITURE=0`, qui pose
`inscriptible=false` et fait donc refuser `PRE_RENAME`/`PRE_DELETE` **pour une
autre raison**. Le contrôle du plan l'a attrapée. Une seconde tentative sous
`PONT_ECRITURE=0` a été disqualifiée elle aussi (`rouge-ecriture-DISQUALIFIEE-*`) :
la racine ne se monte pas du tout, `total=0`, il n'y a **rien** à mesurer.

---

## §10 — Ce que F3 n'établit PAS

- **Aucun taux, nulle part.** Deux exécutions par critère au mieux.
- **Le renommage d'un répertoire n'est pas livrable** (§2), et le repli par
  copie de `copie.ts` **n'a donc jamais couru en conditions réelles**.
- **Le canonicalisateur n'est pas éprouvé sur un disque insensible à la casse**
  (§7).
- **Quatre des douze codes ne sont pas relevés sur le binaire livré** (§5).
- **`canal-ferme` produit APRÈS la fermeture n'est jamais RECENSÉ** : le
  recensement est émis **à la fermeture** et le fil du service retourne. Le
  compteur est incrémenté, personne ne l'imprime.
- **Aucun éditeur réel n'a été exercé** : l'idiome temporaire+renommage est
  couvert par les tests d'hôte de `mutation.rs`, **jamais par LibreOffice ni
  Word**. C'est la lacune la plus lourde de ce sous-bloc.
- **Le contrôle de flux n'est pas mesuré sous charge** : `SEUIL_TAMPON` et
  `MORCEAUX_EN_VOL` ne sont **pas calibrées**, et rejoignent `BPP_MIN`,
  `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`, `HYSTERESIS`, `TAILLE_MAX_SORTIE`,
  `DELAI_MUTATION`.
- **Aucune latence de bout en bout**, qu'aucun sous-bloc du chantier D ni du
  pont n'a jamais mesurée.
- **Un seul navigateur** (Chromium sans interface), **une seule VM**, **un seul
  utilisateur**. La File System Access API n'existe ni sur Firefox ni sur
  Safari — limite du **produit**.
- **`TAILLE_MAX_FICHIER` de la spec n'est pas implémentée**, et la raison est
  architecturale : ProjFS n'est **jamais** sur le chemin d'écriture (§11).

---

## §11 — Le fait d'architecture qui gouverne tout le terrain (hérité de F2)

**ProjFS n'est JAMAIS sur le chemin d'écriture.** Le fournisseur n'apprend une
écriture qu'à la **fermeture du descripteur**, par une notification **POST**,
et une POST **ne peut pas être refusée** — aucun `HRESULT` n'atteint personne.

Il reste donc **deux** leviers, et deux seulement :

1. **refuser en amont sur un ÉTAT** — la seule porte refusable est
   `PRE_CONVERT_TO_FULL`, à quoi F3 ajoute `PRE_RENAME` et `PRE_DELETE` ;
2. **dénoncer après coup.**

---

## §12 — Pièges neufs, à connaître avant de toucher à ce terrain

- 🔴 **UNE COUPURE CALÉE SUR UNE HORLOGE TOMBE DANS LE VIDE.** Trois coupures
  successives ont rendu `abandonnee=0` **avec un canal pourtant fermé
  proprement**, faute d'avoir quoi que ce soit à abandonner. La coupure se pose
  sur un **FAIT** — une marque écrite juste avant la commande qui ne revient
  jamais — et le délai qui la suit sert seulement à laisser la commande
  ATTEINDRE le pont. **À 1500 ms elle rend `canal-ferme` ; à 0 ms,
  `abandonnee`. Deux codes, deux instants, et le réglage les départage.**
- 🔴 **UN INSTRUMENT QUI N'ÉCRIT QU'À LA FIN FAIT DÉPENDRE TOUTE LA MESURE DU
  GESTE LE PLUS FRAGILE.** La mesure VM s'est figée sur un geste et **les
  critères ① et ②, pourtant déjà mesurés, ont été perdus avec le reste** —
  fichier à ZÉRO octet. Deux points de reprise coûtent deux lignes.
- 🔴 **UNE TENTATIVE FIGÉE TIENT SON FICHIER DE RELEVÉ, ET LA SUIVANTE ÉCRIT
  DANS LE VIDE** — « le fichier est en cours d'utilisation par un autre
  processus ». Tuer le processus figé est **nécessaire et pas suffisant** : le
  tueur lui-même a échoué en silence, par la collision de guillemets de
  `nodejs-winrm` (piège D3), et le verrou a survécu à **trois** tentatives. Le
  remède qui retire la classe entière est un **chemin de relevé neuf par
  exécution**.
- ⚠️ **`Start-Job` + `Wait-Job -Timeout` rend un INTERBLOCAGE sous tâche
  planifiée** (`BlockedJobsDeadlockWithWaitJob`, trace versée). La borne qui
  tient est celle du produit lui-même.
- ⚠️ **UN ARBRE OPFS VIDE DE MUTATIONS AVEC `vues=0` EST UNE EXÉCUTION
  INVALIDE, PAS UN RÉSULTAT.** Une exécution a rendu `c1a=ok` côté VM avec un
  arbre local intact : elle a été **rejouée**, pas rapportée.
- ⚠️ **LA VM S'HIBERNE SEULE** (piège D1, rencontré **deux fois** dans ce
  sous-bloc), et le partage tombe avec elle. Le symptôme est
  `run-agent.ps1: Aucun fichier ou dossier de ce nom`, qui se lit comme une
  panne du script.
- ⚠️ **UNE MUTATION PAR SUBSTITUTION DE CHAÎNE FRAPPE LE COMMENTAIRE AVANT LE
  CODE**, et mon propre harnais de rouge était vacueux : il prouvait la
  mutation par `git diff --numstat`, non nul sur un fichier non commité même
  quand rien n'avait bougé. Corrigé par un diff contre une **copie nommée**, et
  le garde corrigé a été **VU se déclencher** (sortie 9) sur une mutation
  blanche.

---

## §13 — Ce que F3 lègue

### À **F4** (la latence et le confort)

1. ⛔ **Aucune latence n'est mesurée**, ni de lecture ni de mutation. Le risque
   R2 tient : *le lecteur peut fonctionner et rester inutilisable.*
2. ⛔ **`SEUIL_TAMPON` et `MORCEAUX_EN_VOL` ne sont pas calibrées**, et le
   contrôle de flux n'a jamais été exercé sous charge.
3. ⛔ **Aucun éditeur réel n'a exercé l'idiome temporaire+renommage.** C'est le
   seul chemin par lequel une sauvegarde peut se perdre en silence.

### À **F5** (le rafraîchissement)

4. ⛔ **Aucun cache d'énumération**, délibérément : `Rafraichir` est un livrable
   de F5, et un cache que rien n'invalide est le défaut de l'ancien pont
   (`src/file.js`, cache **sans TTL**).
5. ⛔ **Le canonicalisateur n'a AUCUN cache**, pour la même raison — il replie
   à chaque appel.

### Sans sous-bloc assigné

6. ⛔ **Le renommage d'un répertoire est refusé par ProjFS** (§2). Le contourner
   demanderait un copier/supprimer piloté depuis la VM, c'est-à-dire un
   changement de conception.
7. ⛔ **`repertoire-non-vide` et `deja-present` sont hors d'atteinte** (§6).
8. ⛔ **Un code produit APRÈS la fermeture du canal n'est jamais recensé**
   (§10). Le remède est un recensement à l'arrêt du processus, pas seulement à
   la fermeture du canal.
9. ⛔ **Le canonicalisateur n'est pas éprouvé sur un disque insensible à la
   casse** (§7) : il y faudrait un vrai `showDirectoryPicker`, que **F1 a mesuré
   inatteignable en CDP**.
10. ⛔ **`TAILLE_MAX_FICHIER` reste non implémentée** (§11).
