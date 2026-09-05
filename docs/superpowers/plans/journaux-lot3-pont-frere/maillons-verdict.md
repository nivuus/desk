# Lot 3, item 3 (3.8) — le répertoire frère qui disparaît

**Mesuré le 5 septembre 2026.** ⚠️ **Aucun chiffre de ce document ne se
recopie sans relancer sa commande.**

---

## 1. Le verdict

🔵 **LE DÉFAUT NE SE REPRODUIT PAS SUR LE PRODUIT D'AUJOURD'HUI.** Après un
renommage dans la VM, le répertoire frère `sous-dossier` **reste présent** dans
tous les listages — **cache armé comme cache désarmé**.

| bras | renommage | `D sous-dossier` dans les 5 listages | témoin négatif |
| --- | --- | --- | --- |
| **a** (cache armé) | `True` | **5 / 5** | tire |
| **b** (cache armé, 2ᵈᵉ exécution) | `True` | **5 / 5** | tire |
| **sans-cache** (`PONT_CACHE=0`) | `True` | **5 / 5** | tire |

Les cinq listages sont : avant le renommage, immédiatement après, un second
**sans aucun `Rafraichir`**, un troisième **après plus de
`TTL_ENUMERATION` (30 s)**, et celui du témoin négatif.

Ce que F5 avait mesuré le 21 août 2026 — *« après un renommage dans la VM, le
répertoire frère `sous-dossier` disparaît du listage alors qu'il existe
toujours dans OPFS »* — **n'est plus observable**.

## 2. 🔴 LE TÉMOIN NÉGATIF, SANS LEQUEL CE VERDICT NE VAUDRAIT RIEN

« Le frère est toujours là » serait rendu **à l'identique** par un listage
**incapable de perdre quoi que ce soit**. C'est le patron que ce dépôt punit :
un contrôle qui ne peut rendre qu'une valeur.

On supprime donc **pour de bon** un témoin — `temoin-2.txt` — depuis la VM, et
le listage suivant **doit** le perdre. Aux **trois** bras :

```
avant  : F renomme.txt   D sous-dossier   F temoin-1.txt   F temoin-2.txt
après  : F renomme.txt   D sous-dossier   F temoin-1.txt
```

**Le listage sait perdre une entrée.** La présence du frère est donc une
mesure, pas un artefact.

## 3. Les quatre maillons, chacun avec SA mesure

🔴 **Disculper un maillon ne désigne pas le coupable suivant** — et ici il n'y
a pas de coupable : chaque maillon est relevé pour lui-même.

| Maillon | Ce qui est relevé | Résultat |
| --- | --- | --- |
| **① la notification ProjFS reçue** | le journal de l'agent depuis un repère posé avant le geste | le renommage **aboutit** (`issue du renommage : True`) — le `PRE_RENAME` n'a donc pas été refusé |
| **② ce que le pont en fait** | `agent::pont::ecriture::fil` | `mutation acquittee : le poste local a suivi correlation=4` (bras a et b), `correlation=5` (sans-cache) — **la mutation est poussée ET acquittée** |
| **③ ce que le cache retient** | l'A/B `PONT_CACHE=0`, variable **lue dans le `run-agent.ps1` généré** (`ligne 35` contre l'invocation `ligne 61`) et **trace de désarmement présente** (`cache d'enumeration DESARME (PONT_CACHE=0)`) | **aucune différence** entre armé et désarmé : le frère est présent dans les deux |
| **④ ce que le poste local contient** | relecture d'OPFS par le navigateur, **arborescence complète** | `renomme.txt`, **`sous-dossier/` (avec `autre.txt`)**, `temoin-1.txt` — identique aux trois bras, et **cohérent avec le listage de la VM** |

🔵 **Le point le plus informatif est ③** : F5 avait établi que le cache
**prolongeait** le défaut jusqu'au TTL. Aujourd'hui, **le désarmer ne change
rien**, parce qu'il n'y a plus de défaut à prolonger.

## 4. Ce que ce verdict N'ÉTABLIT PAS

- 🔴 **UNE NON-REPRODUCTION N'EST PAS UNE PREUVE D'ABSENCE, ET N'EST PAS UNE
  CORRECTION IDENTIFIÉE.** Rien ici ne nomme le commit qui a fermé le défaut, ni
  n'exclut qu'il reparaisse dans une autre forme (un renommage de
  **répertoire** plutôt que de fichier, un frère plus profond, un jeu plus
  grand). **Trois bras, un seul jeu, un seul geste.**
- ⚠️ **Le jeu reproduit celui de F5 dans sa FORME, pas à l'octet** : un
  `sous-dossier` portant un enfant, un fichier renommé, deux témoins. F5
  travaillait sur un jeu plus grand, monté par ses propres pilotes.
- ⚠️ **Le geste est un renommage de FICHIER** (`a-renommer.txt` →
  `renomme.txt`). Un renommage de **répertoire** n'a pas été éprouvé.
- ⚠️ **Aucune correction de produit n'a été écrite**, et aucune n'était due :
  la mesure ne montre pas de défaut.
- ⚠️ **Les mêmes limites d'instrument que l'item 1** : OPFS à la place de
  `showDirectoryPicker()`, drapeau d'origine sûre, et une purge locale qui ne
  se propage pas à une racine ProjFS déjà hydratée (voir § 5).

## 5. Les pièges payés par cet instrument, mesurés

1. 🔴 **LA RACINE ProjFS DE LA VM SURVIT À LA PURGE D'OPFS.** À la deuxième
   exécution, `renomme.txt` y était encore, le renommage a échoué
   (`issue du renommage : False`), **et même le témoin négatif n'a plus
   tiré** : le bras était perdu pour une raison **étrangère** au défaut
   cherché. Le script purge désormais la racine de la VM **avant** que le pont
   ne monte.
2. 🔴 **UNE PURGE QUI S'IMPRIME SANS SE GARDER N'EST PAS UN CONTRÔLE.** La
   première rédaction affichait `restant apres purge : 3` **et continuait**.
   **Deux bras** ont été perdus ainsi. La purge est devenue une **porte** :
   elle réessaie, puis **abandonne en le disant**.
3. ⚠️ **Le pilote est celui de l'item 1, RÉUTILISÉ PAR PARAMÈTRE**
   (`--injection`, `--prepare`, `--relire`) et **jamais copié** — « une copie
   éprouverait la copie, pas l'instrument » (F3). Les défauts de ses options
   laissent l'invocation de l'item 1 inchangée.

Journaux bruts versionnés : `listages-{a,b,sans-cache}.log`,
`maillons-{a,b,sans-cache}.log`, `opfs-{a,b,sans-cache}.json`,
`pilote-{a,b,sans-cache}.log`.
