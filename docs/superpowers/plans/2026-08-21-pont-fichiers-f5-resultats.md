# Sous-bloc F5 — la vie longue : résultats

**21 août 2026. DERNIER sous-bloc du sous-projet ③.** Ce que F5 ne prend pas
sortira du sous-projet **sans destinataire** : il n'y a pas de F6.

Plan : `2026-08-21-pont-fichiers-f5.md`. Spécification :
`specs/2026-08-19-pont-fichiers-design.md`, §F5 — **annotée** par F5 sur sa
clause ROUGE, jamais réécrite. Journaux et instrument :
`journaux-pont-fichiers-f5/`, dont `familles-de-lecture.txt` **relevé par la
commande** (17 fichiers portent des séquences ANSI ; chacun a son jumeau
`-plat.log` versé).

---

## 1. Le verdict, avec le nombre d'exécutions dans chaque énoncé

**Aucun taux n'est revendiqué nulle part.** Deux exécutions établissent la
reproductibilité, jamais une fréquence.

| # | Critère | Verdict | Exéc. |
| --- | --- | --- | --- |
| ① | un fichier ajouté sur le poste local apparaît **après** `Rafraichir` et pas avant | **TENU** | **3** vertes + **2** rouges |
| ② | le journal survit à un **redémarrage complet** et se vide à la reconnexion | **TENU** | **1** |
| ③ | le retour sur un répertoire **différent** retient au lieu d'écrire, et le **dit** | **TENU** | **2** |
| ④ | un `Rafraichir` ne casse rien de F2 ni de F3 | **TENU**, et il a **trouvé un défaut** | **2** + **1** sans cache |
| P1 | quelle grandeur d'occupation disque est lisible sans traverser | **TRANCHÉE** | **2** |
| P2 | la racine ProjFS survit-elle à un redémarrage | **TRANCHÉE à moitié** | **1** |
| P3 | le filtre nous rappelle-t-il sur une création locale | **TRANCHÉE, avec sa réserve** | **2** |
| T17 | la latence de listage aux rangs de F4, cache armé | **MESURÉE** | **2** |

---

## 2. 🔴 LE FAIT QUI PÉRIME F4, ET C'EST LE VERT QUI LE FAIT

La spec écrivait que le **rouge** de F5 signifierait « que la mesure de F4
portait sur autre chose que ce que le produit livre ». **La prémisse est juste,
la conclusion est fausse, et c'est F4 qui la réfute** : son annotation D2 dit
qu'il mesure « **le chaud que le produit A** », donc **une borne HAUTE** d'un
produit **sans** cache. Un rouge le confirmerait ; c'est le **vert** qui le
périme.

**Le verdict est VERT**, donc F5 a remesuré (§0.1, tâche 17) :

| Rang | Froid (aller-retour) | **Chaud (cache)** |
| --- | --- | --- |
| 10 | 94 / 63 ms | **0 / 0 ms** |
| 100 | 283 / 338 ms | **0 / 9 ms** |
| **1 000** | 3 110 / 2 799 ms | **31 / 32 ms** |

⚠️ **Les FROIDS ne se comparent PAS aux ~6 s de F4** : F5 liste un
sous-répertoire de fichiers vides, F4 listait sa racine. **Seul le rapport
froid/chaud intra-série est de F5.**

⚠️ **LE MUR DE ~3 150 ENTRÉES NE BOUGE PAS.** Il tient à la taille d'**un**
message, pas à la répétition.

**La prédiction du §0.2, écrite avant la mesure** : sa clause (3) — un second
`Get-ChildItem` dans la fenêtre du TTL coûte `lister=n:0` — est **confirmée**
(0 ms). Sa clause (2) — « approximativement divisée par deux » — est **largement
dépassée** : facteur ~90 à mille entrées. ⛔ **Le MÉCANISME des deux `Lister`
par geste reste inexpliqué** (legs n°4 de F4) : le cache en absorbe un, il ne
dit pas pourquoi il y en a deux.

---

## 3. 🔴 LE DÉFAUT QUE LA RECETTE A TROUVÉ, ET SON ATTRIBUTION PAR A/B

Après un **renommage dans la VM**, le répertoire frère `sous-dossier` **disparaît
du listage** alors qu'il **existe toujours dans OPFS** (vérifié : l'arbre porte
`sous-dossier` et son enfant `autre.txt`).

**L'A/B tranche l'attribution :**

| | après renommage | après suppression |
| --- | --- | --- |
| cache **armé** | `sous-dossier` **disparaît** | **il ne revient pas** (5 entrées) |
| `PONT_CACHE=0` | `sous-dossier` **disparaît aussi** | **il revient** (6 entrées) |

⇒ **LE DÉFAUT EST PRÉEXISTANT — il n'est pas de F5 — ET LE CACHE LE PROLONGE**,
d'un listage à au plus `TTL_ENUMERATION`. C'est littéralement *« un cache rend un
mode d'échec plus discret »*, l'argument que D10 employait pour le mur de
listage, rencontré ici sur un défaut de renommage.

⚠️ **LA CLAUSE DE NON-LIVRABILITÉ NE SE DÉCLENCHE PAS.** Elle vise un critère ④
qui tombe **sans cause identifiée** ; ici il ne tombe pas — ses quatre assertions
sont vraies — et la cause est identifiée **et mesurée**.

---

## 4. Les trois portes

**P3 — le filtre nous rappelle, et notre invalidation est ce qui re-demande.**
Le journal porte `notification ProjFS code=4
chemin=ne-vient-pas-du-navigateur.txt`, et **le relistage qui suit coûte 49 ms**
— un aller-retour, pas les 7-8 ms d'un succès de cache. La mémoire du répertoire
avait donc bien été oubliée. ⚠️ **Cela ne sépare PAS** « le filtre fusionne aussi
de lui-même » : notre invalidation le précède.

**P1 — deux candidates retenues, une réserve chacune.**
`GetDiskFreeSpaceEx` : 814,7 Go libres / 1 184,9 Go utilisés — ⚠️ **confondeur
NON LEVABLE**, tout le reste de la VM y compte. La somme des longueurs des
fichiers hydratés : **300 081 octets en 28 ms**, **sans aucune traversée** du
pont. ⚠️ C'est une taille **logique**, pas une occupation.

**P2 — la racine survit ; son MARQUAGE n'est pas décidable par cet instrument.**
Survivent, **octet pour octet** : `ecritures.journal` (178 o, sha256
`55199198…`), `instance.guid`, `racine.nom`. `PrjStartVirtualizing` réussit,
`PrjFlt` est chargé (altitude 189800).
🔴 **Le redémarrage est VOULU et non SUBI, et c'est vérifié** : `LastBootUpTime`
12:36:51 UTC suit le `Restart-Computer` de 12:36:33, et le compteur libvirt
`terminating on signal` reste à 167. *F2 n'avait traversé qu'une **hibernation**,
qui restaure la mémoire.*

> 🔴 **ET LA RAISON POUR LAQUELLE LE MARQUAGE N'EST PAS DÉCIDABLE EST UNE
> AFFIRMATION FAUSSE DANS LE PRODUIT.** `racine.rs` écrivait : « ce qui est
> connu, c'est que re-marquer **échoue** ». C'est **faux** :
> l'exécution `p2-dues` a tourné **sans purge et sans redémarrage** sur une
> racine déjà marquée dont l'empreinte existait, et
> `PrjMarkDirectoryAsPlaceholder` a **RÉUSSI**.
>
> Deux conséquences : **la branche de tolérance n'a JAMAIS couru**, et la trace
> disait « marquée pour la **PREMIÈRE** fois » dans les **trois** cas — donc
> elle ne pouvait pas répondre à la question que P2 lui posait. Elle rapporte
> désormais `empreinte_preexistante`. ⚠️ **Le comportement n'est pas changé** :
> le rendre fatal sur trois exécutions d'**une** machine serait l'inverse de ce
> que ce paragraphe reproche.

---

## 5. Ce que le code livre

| Étage | Fichier | Nature |
| --- | --- | --- |
| le cache | `agent/src/pont/cache.rs` (149) + `cache/tests.rs` (150) | **PUR**, horloge **injectée**, 11 tests, expiration **sans dormir** |
| la règle de reprise | `agent/src/pont/bonjour.rs` (84) + `bonjour/tests.rs` (61) | **PUR**, 8 tests |
| l'aiguillage | `agent/src/pont/service.rs` | **AVANT `table.resoudre`** — la décision qui porte le sous-bloc |
| les deux annonces | `agent/src/pont/service/annonces.rs` (104) | extrait **AVANT** la revue transverse |
| le contrat du fil | `agent/src/pont/ecriture/fil/contrat.rs` (88) | extrait **AVANT** l'addition |
| la 4ᵉ famille | `proto/src/fichiers.rs`, `fichiers/entetes.rs`, `proto/ts/fichiers*.ts` | `TYPE_BONJOUR`=68, `TYPE_RAFRAICHIR`=69, `Dues.retenues` **REQUIS** |
| le client | `client/src/fichiers/protocole.ts`, `canal.ts`, `shell*.ts`, `shell.html` | les deux boutons, `Bonjour`, et le `send()` refusé qui **dénonce** |

**Trois divergences tranchées CONTRE le plan**, chacune avec sa raison écrite
dans le code : `Dues.retenues` est **REQUIS** et non `#[serde(default)]` (un
défaut à `false` vaudrait « le pont pousse », c'est-à-dire le sens dangereux) ;
`Rafraichir` n'a **aucune structure** d'en-tête (précédent de `TYPE_FAIT`) ; et
la rouge de la tâche 8 telle que le plan l'écrivait **ne pouvait pas rougir des
deux côtés** — TypeScript ne lit pas le Rust, et F1 avait posé qu'**un seul côté
suffit**.

---

## 6. Ce que F5 n'établit PAS

- **Aucun taux, nulle part.**
- ⛔ **Aucun jugement d'usage** : personne n'a dit si 31 ms ou 3 s est
  acceptable. La lacune que ce dépôt traîne depuis `BPP_MIN`.
- ⛔ **Deux constantes non calibrées de plus** : `TTL_ENUMERATION` (30 s) et
  `DELAI_BONJOUR` — ⚠️ *cette seconde n'a d'ailleurs PAS été implémentée* : le
  cas « aucun `Bonjour` n'arrive » n'a pas de minuteur, et le pont reste
  simplement silencieux. **Déclaré, pas dissimulé.**
- 🔴 **Le MODÈLE DE PERMISSION n'est éprouvé par rien** : `showDirectoryPicker`,
  `queryPermission`, `requestPermission` ne sont appelés nulle part. **F5 mesure
  que la règle du NOM fonctionne ; il ne mesure pas qu'elle suffise.**
- 🔴 **Le mur de ~3 150 entrées n'est pas déplacé**, et sa rouge (le gel de 20 s
  qui revient si l'on retire le renvoi d'`Echec`) **n'a pas été jouée** : elle
  exige un répertoire de plus de 3 200 entrées.
- ⛔ **`MORCEAUX_EN_VOL` (D11) n'a pas été éprouvé** : la règle d'admission était
  écrite, la mesure n'a pas été jouée. **Le legs n°3 de F4 ressort tel quel.**
- ⛔ **Le débit du canal reste à ~33 Kio/s** (legs n°1 de F4, non pris).
- ⛔ **Aucune politique d'éviction** : `PrjDeleteFile` reste sans appelant, et
  **quatre des cinq entrées ProjFS sans jumeau `PRJ_*_CB` le restent**.
- ⛔ **Rien d'un client réel** : un seul Chrome, sans interface, sur l'hôte qui
  porte la VM.

---

## 7. 🔴 Trois défauts d'ENVIRONNEMENT ont coûté six exécutions

**Aucun n'est du produit, et les trois se lisaient comme s'ils l'étaient.**

1. **`/tmp` est un tmpfs de 10 Go, et le harnais y laisse ~100 Mo par
   exécution.** À 100 %, OPFS refuse d'écrire, et **tout** ressemble à une panne
   produit : `removeEntry … modifications are not allowed`, `[object Object]`
   comme message de montage, `fetch failed`. **Quatre exécutions perdues avant
   de penser à mesurer `df`.** *Purger `/tmp/*/udd-*` entre les campagnes.*
2. **L'hôte a tué la VM DEUX fois en pleine campagne** — `terminating on signal
   15`, compteur 167 → 168 → 169 : c'est `libvirtd --timeout 120`. **C'est
   exactement la distinction que P2 exigeait**, et elle a servi deux fois.
3. **Un `pkill -f` depuis un shell dont la ligne de commande porte le motif a
   tué mon propre shell** (exit 144) — piège déjà écrit dans `CLAUDE.md`.

⚠️ **Et une misattribution évitée de justesse** : j'ai conclu « la plateforme est
DOWN » d'un `fetch failed`, et elle répondait **200**. C'est la règle du dépôt —
*remesurer avant d'attribuer* — appliquée à temps.

---

## 8. Ce que F5 lègue, et ③ se ferme derrière

**La liste est COMPLÈTE : il n'y a pas de F6, et rien de ce qui suit n'a de
destinataire.**

1. 🔴 **Le canal du pont plafonne à ~33 Kio/s** ; la parade est un changement de
   conception de la boucle de `pont::transport` (F4 n°1).
2. 🔴 **Aucun listage de plus de ~3 150 entrées n'aboutit.** F5 en fait une
   erreur immédiate au lieu d'un gel de 20 s — **il ne le déplace pas**, et
   **cette rouge-là n'a pas été jouée**.
3. 🔴 **L'idiome « fichier temporaire + renommage » sur un éditeur réel** (F3
   n°3) — *le seul chemin par lequel une sauvegarde peut se perdre en silence*.
4. 🔴 **Un renommage dans la VM fait DISPARAÎTRE un répertoire frère du
   listage** (§3). Préexistant, **prolongé par le cache** jusqu'au TTL.
5. 🔴 **`showDirectoryPicker()`, le modèle de permission, le mode `readwrite`** —
   exigent `Xvfb` + `xdotool`, consentement donné **en D8**, jamais suivi d'effet.
6. ⛔ **Aucune politique d'éviction**, et le disque de la VM grossit — **F5 le
   mesure et le fait croître**.
7. ⛔ **Quatre des cinq entrées ProjFS sans jumeau `PRJ_*_CB`** (R7). La
   cinquième, `PrjClearNegativePathCache`, a gagné son premier appelant.
8. ⛔ **Le mécanisme des deux `Lister` par `Get-ChildItem`** (F4 n°4).
9. ⛔ **`MORCEAUX_EN_VOL` non éprouvé** (F4 n°3), règle d'admission écrite.
10. ⛔ **Le condensat SHA-256 de bout en bout** (F1 n°6) ; **les lectures qui
    calent sans expirer** (F1 n°4) ; **le coût de la canonicalisation de casse**
    (F4 n°6) ; **la cause du delta M2 − M1 inverse** (F4 n°5).
11. ⛔ **La branche de tolérance de `racine.rs` n'a jamais couru**, et le
    marquage d'une racine à travers un redémarrage n'est **pas décidé**.
12. ⛔ **`DELAI_BONJOUR` n'existe pas** : le cas « aucun `Bonjour` n'arrive » est
    silencieux, sans minuteur ni trace.
13. ⛔ **La calibration**, et **le jugement d'usage** : personne ne les portera
    dans ③.
