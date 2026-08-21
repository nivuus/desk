# La scission de `client/src/design/tokens.css` — la note, NON JOUÉE

**Tâche 20 du plan de G5, budgétée et délibérément NON EXÉCUTÉE.** Elle existe
pour qu'un successeur n'ait pas à redécouvrir le chemin, et pour que la décision
**D3** repose sur des mesures plutôt que sur une lecture de code.

## Pourquoi la question se pose

`tokens.css` est à **300 lignes pour une porte de 300 — marge NULLE**. Le
premier chantier qui devra **déclarer** un token devra donc le scinder d'abord.
Et il devra le déclarer, parce qu'un **repli ne suffit pas** :

> `tokensReferences` (`client/src/design/tokens.ts`) emploie
> `/var\(\s*(--[\w-]+)/g`. Sur `var(--accent-fenetre, var(--accent))`, il
> capture **`--accent-fenetre`**, qui n'est pas déclaré, et l'inclusion ① de
> §7.6 rougit.

🔵 **MESURÉ, PAS DÉDUIT** — rouge n°8 de G5, jouée puis restaurée :

```
inclusion ① — tout var(--…) est déclaré : 1 écart(s)
  NON DÉCLARÉ  --accent-fenetre  employé par client/src/hub/hub.css
```

⚠️ **La clause du sous-projet ① — « toute référence future doit porter un repli
OU déclarer le token » — est donc FAUSSE de sa première moitié.** Seule la
seconde tient.

## 🔴 Ce que la scission coûte, et pourquoi elle est plus lourde qu'annoncé

`CLAUDE.md` annonce **sept** lecteurs qui nomment `tokens.css` par son chemin.
Le plan de G5 en avait relevé **neuf**. **Il y en a ONZE**, et les deux derniers
sont **de G5 lui-même**.

> 🔴 **CETTE LIGNE A PORTÉ « NEUF » PENDANT LE TEMPS DE L'ÉCRIRE, ET C'ÉTAIT LE
> NAUFRAGE DU « 487 » DANS UNE NOTE DONT LE SUJET EST UN COMPTE.** Le neuf était
> juste **à la date du plan** ; entre-temps, `client/src/hub/manifeste.test.ts`
> et `client/src/hub/catalogue.test.ts` ont chacun ajouté un
> `import tokensCss from '…/tokens.css?raw'` — précisément parce que §7.2 m'a
> interdit d'écrire une couleur et m'a forcé à la LIRE sur la source. **Le
> remède à un contrôle a fait grandir le coût d'un autre**, et je ne l'ai vu
> qu'en relançant la commande **après** ma dernière édition, jamais en relisant.

Relevé par la commande, `dist/` exclu :

| # | Lecteur | Comment il le nomme |
| --- | --- | --- |
| 1 | `client/outils/contraste.mjs:24` | chemin par défaut |
| 2 | `client/outils/blocs-de-theme.mjs:42` | chemin par défaut |
| 3 | `client/outils/tokens-orphelins.mjs:74` | `const SOURCE` |
| 4 | `client/outils/couleurs-litterales.mjs:205` | le chemin d'**EXCLUSION** — il le nomme sans l'ouvrir |
| 5 | `client/src/design/galerie.ts:34` | `?raw` |
| 6 | `client/src/design/tokens.test.ts:9` | `?raw` |
| 7 | `client/src/design/reprise.test.ts:4` | `?raw` |
| 8 | `client/src/accent.test.ts:30` | `?raw` — **posé par ①** |
| 9 | `client/src/accent-dom.test.ts:15` | `?raw` — **posé par ①** |
| 10 | `client/src/hub/manifeste.test.ts:2` | `?raw` — **posé par G5**, pour ne pas écrire de couleur |
| 11 | `client/src/hub/catalogue.test.ts:2` | `?raw` — **posé par G5**, même raison |

🔴 **ET C'EST LE MODE DE PANNE QUI COMPTE, PAS LE COMPTE.** Scinder le fichier
sans reprendre ses lecteurs fait rendre **ZÉRO déclaration** à §7.1, §7.4 et
§7.6 — **c'est-à-dire les ÉTEINT SANS QU'AUCUN NE ROUGISSE**. Le quatrième
outil, §7.2, rougirait au contraire bruyamment : son exclusion cesserait de
désigner quoi que ce soit. **C'est le seul des quatre qui préviendrait**, et il
ne faut donc pas compter sur lui pour les trois autres.

⚠️ **Et §7.4 imposerait une contrepartie** : sa clause ③ exige que toute
**couleur** de la racine soit redéclarée dans les **deux** blocs clairs, sauf
les **SEPT** tokens de `COULEURS_HORS_THEME` (`CLAUDE.md` en publie **six**).
Déclarer `--accent-fenetre` obligerait à choisir entre une contrepartie claire —
qui n'a pas de sens pour une couleur venue d'une icône — et une **huitième**
entrée hors thème.

## La variante MOINS COÛTEUSE, nommée

**Extraire la DOCTRINE plutôt que les DÉCLARATIONS.** `tokens.css` est fait
d'environ deux tiers de commentaire — l'échelle, ses raisons, les mesures qui la
fondent. Sortir cette prose vers un `tokens/doctrine.md` (ou un en-tête réduit
renvoyant à la spec §4.4) rendrait des dizaines de lignes **sans qu'aucun des
onze lecteurs ne bouge**, puisque le fichier resterait à son chemin et
porterait les mêmes déclarations.

C'est une **EXTRACTION**, pas une compression : ce dépôt interdit nommément de
raccourcir une justification pour atteindre un compte de lignes.

## ⚠️ Ce que cette variante NE résout PAS

Elle rend de la **marge**, pas de la **structure**. Le jour où l'on voudra
vraiment séparer les couleurs des échelles — `tokens/couleurs.css` et
`tokens/echelles.css` —, les onze lecteurs devront être repris **ensemble**, et
les trois contrôles ci-dessus devront être vus **ROUGES** avant de l'être
verts : sans cela, rien ne distinguerait une scission réussie d'une scission qui
les a éteints.
