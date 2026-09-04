# Lot 3, item 6 — la file du capteur en charge réelle

**Mesuré le 5 septembre 2026.** Journal de l'agent : `C:\nivuus\agent.log`,
**169 993 lignes au moment du relevé versé ici** (`releve-20260905T0108.log`),
cinq jours de fonctionnement continu.

🔴 **CE JOURNAL GROSSIT PENDANT QU'ON LE MESURE.** Le même compte relevé
25 minutes plus tôt dans cette campagne rendait 168 940 lignes et 449
attachements. **Aucun des nombres ci-dessous n'est une constante** : ce sont
des instantanés, et seule leur RELATION (un témoin non nul contre un refus nul)
est le résultat.

⚠️ **Aucun chiffre de ce document ne se recopie sans relancer sa commande.**

---

## 1. La constante, RELUE dans le code avant tout dimensionnement

```
agent/src/capteur/sommeil/file.rs:84 : pub(crate) const PROFONDEUR_MAX: usize = 64;
agent/src/capteur/sommeil/file.rs:135:     if file.len() >= PROFONDEUR_MAX {
agent/src/capteur/sommeil/file.rs:367:         profondeur_max = PROFONDEUR_MAX,
agent/src/capteur/sommeil/file.rs:368:  "file d'une session pleine : message REFUSE (trace au palier, puissance de deux)"
```

**`PROFONDEUR_MAX = 64`**, relue et non recopiée.

## 2. 🔴 LE TÉMOIN QUE LE PLAN PRESCRIT EST UN CONTRÔLE QUI NE PEUT PAS RENDRE NON-ZÉRO

Le plan (Task 7, étape 2) prescrit deux comptes :

```bash
grep -ac "message REFUSE" …/agent.log     # attendu : 0
grep -ac "attache au capteur" …/agent.log # attendu : > 0, le TÉMOIN
```

**Le second motif ne peut jamais rien rendre.** Le produit écrit
`"attaché au capteur"` — **avec l'accent** — à `agent/src/capteur/tube.rs:153`.
Mesuré, sur le même journal, dans le même appel :

| motif | compte |
| --- | --- |
| `attache au capteur` (celui du plan) | **0** |
| `attach. au capteur` (celui du code) | **457** |
| `canal rattach. au capteur` | 0 |
| `message REFUSE` (sensible à la casse) | **0** |

🔴 **Sans cette vérification, l'item se serait conclu sur « zéro refus, et le
témoin est zéro aussi » — c'est-à-dire sur un zéro strictement
ININTERPRÉTABLE, indiscernable d'un produit entièrement en panne.** C'est le
piège que `CLAUDE.md` écrit en toutes lettres : *le nom d'un `grep` de recette
se vérifie contre le CODE, jamais contre la spec*. Ici, la spec **et le plan**
portaient le mauvais nom.

⚠️ **`-CaseSensitive` est nécessaire pour `message REFUSE`** : sans lui,
`Select-String` rend **61** faux positifs, tous de la forme
`l'encodeur refuse le réglage du débit à chaud` — un `refuse` minuscule, d'un
tout autre mécanisme.

## 3. Le verdict

🔵 **En charge réelle — 457 attachements au capteur, cinq jours, 169 993
lignes —, la file bornée n'a REFUSÉ AUCUN MESSAGE.** Le palier de 64 n'a
jamais été atteint.

Le zéro est interprétable : le témoin, avec le motif que le produit écrit
réellement, rend **457**.

## 4. Ce que cet item N'ÉTABLIT PAS

- 🔴 **LE BRAS « BOUCHER » N'A PAS ÉTÉ JOUÉ.** Le plan (étape 3) prescrivait
  `boucher.ps1`, qui suspend le fil de fenêtre d'une session (`SuspendThread`
  sur le PID relevé) pendant que sept autres poussent leurs variantes. **Il
  n'a pas été écrit ni exécuté.** Le refus au palier, la progression en
  puissances de deux, `session_cible` et `profondeur_max=64` dans la trace :
  **rien de tout cela n'est mesuré**, et `message REFUSE = 0` ne dit rien de
  leur exactitude — seulement que le cas ne s'est pas présenté.
- ⚠️ **« Charge réelle » ici veut dire le trafic de production de cette VM**,
  pas les huit fenêtres en régime nominal que le plan décrit. Aucune séance de
  huit fenêtres sur trois minutes n'a été montée.
- ⚠️ **Un `Sommeil` refusé reste PERDU** (tracé, non réémis) : legs déclaré par
  le lot 2, que cet item ne ferme pas — et qu'il n'a pas pu observer, faute de
  refus.
