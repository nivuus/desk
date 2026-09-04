# Lot 3, item 11 (3.9) — les applications `NonMesuree`

**Mesuré le 5 septembre 2026.** ⚠️ **Aucun chiffre de ce document ne se
recopie sans relancer sa commande.**

---

## 1. 🔴 LE CONTRÔLE PRESCRIT PAR LE PLAN NE PEUT PAS RENDRE NON-ZÉRO

Le plan (Task 12, étape 1) prescrit :

```bash
grep -ac "NonMesuree" /media/vm/dev/agent.log
```

Mesuré sur le journal réel (`C:\nivuus\agent.log`, **169 993 lignes** au
relevé versé ici — ⚠️ ce journal grossit pendant qu'on le mesure, le même
compte rendait 168 940 lignes 25 minutes plus tôt) :

| motif | compte |
| --- | --- |
| `NonMesuree` | **0** |
| `NonMesur` | **0** |
| `mesuree` | **0** |

**Et ce zéro n'est pas une mesure.** `NonMesuree` est un **variant d'énumération
Rust** (`SourceMax::NonMesuree`), pas une chaîne journalisée : 34 occurrences
dans `agent/` et `proto/`, **toutes du code ou du commentaire**, aucune dans un
`tracing::`. Sur le fil, la sérialisation est **`"source_max":"non-mesuree"`**
(kebab-case), figée par le test
`proto/src/plateforme/tests_apps.rs:227::la_convention_de_nommage_de_source_max_est_observable`.

🔴 **Un `grep` de ce nom dans ce journal est donc structurellement incapable de
rendre autre chose que zéro**, quel que soit l'état du produit. C'est le patron
que ce dépôt punit — et il était **écrit dans le plan**.

## 2. LE CHIFFRE, pris là où il existe : le catalogue servi par la plateforme

`GET /applications?vm=…` sur `http://192.168.3.1:3445`, jeton obtenu par
`/auth/moi` :

```
total                : 41
source_max            non-mesuree  : 34
source_max            pixels:256   :  6
source_max            pixels:48    :  1
```

Relevé complet, **les 34 noms compris** : `mesurees-2026-09-04T2253.json`.

## 3. Ce que cela RÉFUTE

🔴 **`CLAUDE.md` affirme « 71 applications restent `NonMesuree` ». C'EST FAUX
AUJOURD'HUI.** Le catalogue n'en compte que **41 au total**, dont **34** sans
icône mesurée. Le nombre 71 n'a **aucune source** dans le produit
d'aujourd'hui : ni dans le journal (0 occurrence, et le motif est vacueux), ni
dans le catalogue servi (41), ni dans la ligne de réconciliation que l'agent
publie lui-même :

```
agent::apps::boucle::memoire: catalogue reconcilie total=58 retenus=45 cles=41
  icones=0 icones_echouees=0 icones_distinctes=37 apparues=0 modifiees=0
  disparues=0 duree_ms=27 declencheur="periode" notifications=2 debordements=0
```

⚠️ **Trois totaux coexistent et disent des choses différentes** — `total=58`
(raccourcis balayés), `retenus=45` (après écarts : `cible-vide`, `extension`),
`cles=41` (entrées distinctes du catalogue) — et c'est **41** qui correspond au
compte servi par la plateforme. Un lecteur qui recopierait « 58 » ou « 45 »
écrirait un nombre vrai d'autre chose.

## 4. Ce que cet item N'ÉTABLIT PAS

- 🔴 **L'ICÔNE MUTÉE SANS CHANGEMENT DE RACCOURCI N'A PAS ÉTÉ ÉPROUVÉE**
  (étape 3 du plan) : aucune icône d'exécutable n'a été remplacée, aucune
  période de réconciliation observée après coup. Le défaut annoncé — « une
  icône qui change sans que son raccourci change n'est jamais revue » —
  **reste non mesuré**.
- 🔴 **LA ROUGE `APPS_SURVEILLANCE=0` N'A PAS ÉTÉ JOUÉE** (étape 4). Le témoin
  qu'elle exige est relevé et **non nul** — `racine surveill…` = **352**,
  `mode de surveillance retenu` = **91** — donc le bras rouge serait
  discriminant ; il n'a simplement pas été monté.
- ⚠️ **Le compte de 34 est celui d'AUJOURD'HUI, sur CE corpus de 41 entrées.**
  Aucune campagne de mesure d'icônes n'a été lancée avant de le relever : c'est
  l'état au repos, pas un « après campagne ».
- ⚠️ **`icones=0` et `icones_distinctes=37` dans la même ligne** ne sont pas
  contradictoires et ne sont pas expliqués ici.
