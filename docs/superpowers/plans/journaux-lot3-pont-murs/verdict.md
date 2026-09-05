# Lot 3, item 2 (3.6) — les trois murs du pont, re-situés

**Mesuré le 5 septembre 2026**, deux exécutions. Le recensement est armé par
`PONT_MESURE=1`, variable **lue dans le `run-agent.ps1` GÉNÉRÉ** :
`ligne 35 : $env:PONT_MESURE = '1'` contre l'invocation `ligne 61`.

⚠️ **Aucun chiffre de ce document ne se recopie sans relancer sa commande.**
Les valeurs de F4 citées en regard sont **relues dans son document**, jamais de
mémoire.

---

## 1. Le tableau des trois murs, et l'écart à F4

| Mur | F4 (21 août 2026) | **Aujourd'hui** | Écart |
| --- | --- | --- | --- |
| **débit soutenu** | 30 à 33 Kio/s | **32,82 Kio/s** (palier de 78,0 s) | **aucun** — dans la fourchette |
| **taille de lecture maximale** | 128 Kio | **128 Kio** (192 échoue) | **aucun** |
| **rang d'énumération maximal** | ~3 150 (3 200 échoue) | **≥ 3 200** (4 000 échoue) | 🔴 **LE MUR A BOUGÉ** |

## 2. L'échelle de TAILLE — le mur est à 128 Kio, deux fois

| barreau | exécution 1 | exécution 2 |
| --- | --- | --- |
| 4 Kio | ABOUTIT 166 ms — 24,16 Kio/s | ABOUTIT 151 ms — 26,57 Kio/s |
| 16 Kio | ABOUTIT 510 ms — 31,40 Kio/s | ABOUTIT 513 ms — 31,22 Kio/s |
| 32 Kio | ABOUTIT 1 074 ms — 29,80 Kio/s | ABOUTIT 1 170 ms — 27,34 Kio/s |
| 64 Kio | ABOUTIT 2 080 ms — 30,77 Kio/s | ABOUTIT 2 026 ms — 31,59 Kio/s |
| **128 Kio** | **ABOUTIT** 4 132 ms — 30,98 Kio/s | **ABOUTIT** 4 165 ms — 30,73 Kio/s |
| **192 Kio** | **ÉCHOUE** 10 493 ms | **ÉCHOUE** 10 264 ms |
| 256 Kio | ÉCHOUE 2 005 ms | ÉCHOUE 2 012 ms |
| 512 Kio | ÉCHOUE 2 037 ms | ÉCHOUE 2 024 ms |

L'erreur est `An internal error occurred.` — **pas** un dépassement de délai.
⚠️ Le premier barreau qui échoue (192 Kio) met **10,3 s** à échouer, les
suivants **2,0 s** : le premier paie un délai que les suivants ne paient plus.

🔵 **C'est un MUR et non une panne, et c'est le témoin POSITIF qui le dit** :
le plus petit barreau **aboutit**, aux deux exécutions. F4 énonce la règle —
*« un montage où même 10 entrées échoueraient mesurerait une panne, pas un
mur »*.

## 3. L'échelle d'ENTRÉES — le mur a bougé au-delà de 3 200

| rang | exécution 1 | exécution 2 |
| --- | --- | --- |
| 100 | *(voir § 5, anomalie)* | ABOUTIT 28 ms — **100 COMPLET** |
| 1 000 | ABOUTIT 3 160 ms — 1 000 COMPLET | *(voir § 5, anomalie)* |
| 2 000 | ABOUTIT 5 406 ms — 2 000 COMPLET | ABOUTIT 5 828 ms — 2 000 COMPLET |
| 3 000 | ABOUTIT 8 167 ms — 3 000 COMPLET | ABOUTIT 8 484 ms — 3 000 COMPLET |
| 3 150 | ABOUTIT 8 783 ms — 3 150 COMPLET | ABOUTIT 8 573 ms — 3 150 COMPLET |
| **3 200** | **ABOUTIT** 8 530 ms — **3 200 COMPLET** | **ABOUTIT** 9 052 ms — **3 200 COMPLET** |
| **4 000** | **ÉCHOUE** 2 237 ms | **ÉCHOUE** 2 421 ms |

🔴 **F4 DONNAIT 3 200 POUR ÉCHOUANT ; IL ABOUTIT AUJOURD'HUI, COMPLET, AUX DEUX
EXÉCUTIONS.** Le mur est donc **strictement entre 3 200 et 4 000**, là où F4 le
situait entre 3 150 et 3 200.

⚠️ **`COMPLET` est le mot qui compte, pas `ABOUTIT`** : un listage qui rendrait
**moins** d'entrées qu'il n'en existe serait un mur **déguisé en succès**. Le
compte rendu est comparé au compte créé, à chaque rang.

⚠️ **La cause de l'écart n'est PAS identifiée.** F4 rattachait ce mur à la
taille d'un message SCTP ; rien ici ne dit ce qui l'a déplacé — ni un commit,
ni un changement de MTU, ni une version de navigateur. **L'écart est mesuré,
pas expliqué.**

## 4. Le DÉBIT SOUTENU — 32,82 Kio/s, et la première mesure était FAUSSE

```
fichiers lus=20  echecs=0  octets=2 621 440  duree=78,0 s
DEBIT SOUTENU = 32,82 Kio/s   (palier de 78,0 s, soit 7,8 periodes de recensement)
```

Le palier dépasse **7,8 fois** la période de recensement (`PERIODE_RECENSEMENT
= 10 s`, relue dans `agent/src/pont/service.rs:78`), comme l'exige la règle
*« un palier doit être plusieurs fois plus long que la temporisation du
mécanisme qu'il observe »*.

🔴 **LA PREMIÈRE RÉDACTION A RENDU `1 312 669,35 Kio/s`, ET IL A FALLU LA
REJETER.** Elle relisait **le même fichier** en boucle : 615 327 tours,
80,6 Go en 60 s — **quarante mille fois** le débit du pont. Elle mesurait le
**cache de fichiers de Windows**, pas la traversée. ⚠️ **Le symptôme n'était
pas une erreur : c'était un nombre**, et un nombre se lit comme une mesure. Le
remède est **vingt fichiers DISTINCTS** de 128 Kio, lus une fois chacun.

🔵 **Le recensement du produit corrobore, par DIFFÉRENCE entre deux relevés**
(les compteurs sont cumulatifs) : `lire=n:40` puis `lire=n:45`, `moy_us`
≈ 2,67 s par lecture de 128 Kio — soit ≈ 48 Kio/s au niveau de la traversée
seule, contre 32,82 Kio/s bout en bout mesurés côté invité.

## 5. 🔴 UN DÉFAUT NEUF, QUE LA SONDE POSÉE EN PREMIER A ISOLÉ

**Après un échec de lecture, la résolution du répertoire SUIVANT échoue avec
« does not exist » — sur un répertoire qui EXISTE.**

- Exécution 1 : `rang-100` → `Cannot find path … because it does not exist`,
  alors que la relecture d'OPFS montre `rang-100` avec **100 entrées**.
- Exécution 2, `rang-100` **sondé AVANT toute lecture** → **ABOUTIT, 100,
  COMPLET, en 28 ms**. Puis, après les trois échecs de lecture, c'est
  `rang-1000` qui échoue de la même façon.

🔵 **L'anomalie SUIT la position, pas le rang** : elle frappe le premier
répertoire résolu **après** un échec de lecture. C'est ce déplacement qui la
qualifie — et c'est la sonde placée en tête, ajoutée entre les deux exécutions,
qui l'a rendu visible. Un cache de chemin négatif de ProjFS est le suspect
(F5 relevait déjà `PrjClearNegativePathCache` comme l'entrée sans jumeau qui
venait de gagner son premier appelant), **mais rien ici ne l'établit**.

⚠️ **NON CORRIGÉ, et hors du périmètre de cet item** : il est mesuré et
consigné, pas réparé.

## 6. Le bras TÉMOIN, et les deux témoins négatifs

**Le bras « armé, AUCUN geste »** — deux périodes de recensement (2 × 10 s)
sans toucher au pont — rend, aux deux exécutions :

```
traversees attributs=n:0 lister=n:0 lire=n:0 ecrire=n:0 mutation=n:0
```

🔴 **Le contrôle qui vaut n'est pas que la ligne SORTE, c'est qu'elle COMPTE ce
qu'elle dit.** C'est ce `n:0` qui rend les `n:40` du § 4 discriminants.

**Les deux témoins négatifs**, aux deux exécutions :

| ce qu'on force | résultat |
| --- | --- |
| lire un fichier **absent** | `temoin OK : la lecture d un fichier absent ECHOUE — MethodInvocationException` |
| lister un répertoire **absent** | `temoin OK : le listage d un repertoire absent ECHOUE — ItemNotFoundException` |

Sans eux, « tous les barreaux aboutissent » serait rendu **à l'identique** par
un instrument incapable de rapporter un échec.

## 7. Ce que cet item N'ÉTABLIT PAS

- 🔴 **AUCUN MUR N'A ÉTÉ DÉPLACÉ, ET AUCUNE CAUSE N'EST EXPLIQUÉE.** Cet item
  **situe**, il ne corrige pas. L'écart du rang d'énumération est constaté sans
  qu'aucun mécanisme ne soit nommé.
- ⚠️ **Le mur d'entrées n'est encadré qu'entre 3 200 et 4 000** : aucun barreau
  intermédiaire n'a été joué, donc sa valeur exacte reste inconnue.
- ⚠️ **Le mur de taille n'est encadré qu'entre 128 et 192 Kio**, pour la même
  raison.
- ⚠️ **Le défaut du § 5 n'est pas caractérisé** : ni sa portée, ni sa durée, ni
  s'il se répare seul. Une seule occurrence par exécution a été observée.
- ⚠️ **Les mêmes limites d'instrument que les items 1 et 3** : OPFS à la place
  de `showDirectoryPicker()`, drapeau d'origine sûre, purge de la racine ProjFS
  imposée entre exécutions.
- ⚠️ **`analyser-f4.py` n'a pas été employé** : ce relevé mesure des murs, pas
  la distribution de traversées que F4 analysait — les deux instruments ne
  répondent pas à la même question.

Journaux bruts versionnés : `paliers-{1,2}.log`, `opfs-{1,2}.json`,
`pilote-{1,2}.log`.
