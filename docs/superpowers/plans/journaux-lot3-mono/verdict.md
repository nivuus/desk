# Lot 3, item 8 (3.2) — le propriétaire mono-fenêtre

**Mesuré le 5 septembre 2026.** ⚠️ **Aucun chiffre de ce document ne se
recopie sans relancer sa commande.**

---

## 1. Le verdict

🔵 **EN MODE MONO-FENÊTRE, NI LE PRESSE-PAPIER NI L'ACCENT NE POUSSENT UN SEUL
MESSAGE** — là où le produit livré en pousse **3 et 1** sur le même geste.

| bras | `presse-papier de la VM` | `accent de la fenetre Windows` | fenêtre servie | agent vivant |
| --- | --- | --- | --- | --- |
| **témoin** — produit LIVRÉ | **3** | **1** | **oui** (21 lignes) | oui |
| **mono** — `SUPERVISEUR=0 CAPTEUR=0` | **0** | **0** | — | oui (16 lignes) |
| **rouge** — `PRESSE_PAPIER=0 ACCENT=0` | **0** | **0** | **oui** (1) | oui |

🔵 **LE BRAS ROUGE VAUT, ET SES DEUX TRACES DE DÉSARMEMENT SORTENT** — voir
§ 4. ⚠️ **Il a fallu le rejeter DEUX FOIS avant**, pour une cause qui n'était
pas celle que j'avais d'abord nommée.

⚠️ **Les deux chaînes comptées sont celles du CODE QUI LES ÉMET**, relues le
jour même, jamais celles du plan :
`agent/src/capteur/sommeil/presse_papier.rs:71` → `"presse-papier de la VM"` ;
`agent/src/capteur/fenetre/accent.rs:81` → `"accent de la fenetre Windows"`.

## 2. 🔴 LE TÉMOIN À DEUX FACES, ET POURQUOI LE ZÉRO VEUT DIRE QUELQUE CHOSE

Le bras **témoin** joue **le geste identique** — trois écritures du
presse-papier, en **session 1**, relues et vérifiées identiques :

```
session = 1
ecriture 1 : pose='ITEM8-temoin-025727-1' relu='ITEM8-temoin-025727-1' identique=True
ecriture 2 : … identique=True
ecriture 3 : … identique=True
```

et le produit livré répond :

```
agent::capteur::sommeil::presse_papier: presse-papier de la VM octets=21 refus=false   (×3)
fenetre{session=…:w-1}: agent::capteur::fenetre::accent_fenetre: accent de la fenetre Windows   (×1)
```

**Un message par écriture, exactement.** Sans ce bras, le `0` du mode
mono-fenêtre serait rendu **à l'identique** par un instrument qui ne sait pas
voir ces messages.

🔵 **Et il localise les deux propriétaires** : les deux traces sortent de
`agent::capteur::…`. Le chemin ENFANT (`demarrage.rs`) ne fait que lire le
drapeau `crate::presse_papier::actif()` — il n'héberge aucun sondeur.

## 3. Le bras MONO — ce qu'il établit, et ce qu'il ne peut pas séparer

Variables **lues dans le `run-agent.ps1` GÉNÉRÉ**, avec leur **position** :

```
ligne 34 : $env:SUPERVISEUR   = '0'
ligne 35 : $env:WINDOW_TITLE = 'Notepad'
ligne 36 : $env:CAPTEUR = '0'
ligne 62 : & 'C:\nivuus\agent\agent.exe' *>&1 |
```

Le geste aboutit (session 1, trois écritures relues identiques), et le compte
est **0 et 0**.

🔵 **L'agent a bel et bien couru le chemin mono-fenêtre** — 16 lignes de
journal, `aucune fenêtre visible` = **0** :

```
agent::demarrage::source: capture de la fenêtre Windows (recadrage) bitrate=12000000 fps=90
agent::apps::surveillance::fil: surveillance des raccourcis armée racines=4
agent::capture: duplication de sortie établie desktop_width=1280 desktop_height=800
agent::encode::fabrique: encodeur matériel retenu encodeur=NVIDIA H.264 Encoder MFT
```

🔴 **MAIS IL MEURT AVANT DE SERVIR UNE SESSION**, sur un défaut **déjà connu**
(lot 31) :

```
WARN agent::encode: NVENC natif indisponible : repli sur la MFT …
   erreur=nvEncOpenEncodeSessionEx : NVENCSTATUS 2
Error: activation de l'encodeur H.264 matériel (ActivateObject) :
   Catastrophic failure (0x8000FFFF)
```

⚠️ **CE QUE CELA EMPÊCHE DE SÉPARER, ET IL FAUT LE DIRE** : « il n'y a pas de
propriétaire » et « il n'y a pas de session » produisent **le même zéro**. La
lecture de code les sépare — les deux propriétaires vivent dans le **capteur**,
qui n'existe pas quand `CAPTEUR=0` — **mais ce document mesure, et la mesure
seule ne tranche pas**. Le verdict du § 1 est donc **cohérent avec** le legs,
il ne le **démontre** pas seul.

🔴 **DEUX PREMIÈRES VERSIONS DE CE BRAS ONT ÉTÉ REJETÉES**, et c'est ce qui a
mené à celle-ci :
1. `-replace` a **corrompu** la ligne (`$$env:SUPERVISEUR   = 0`) — sa chaîne
   de remplacement traite `$` comme un renvoi de groupe. L'agent n'a plus rien
   journalisé : **segment d'UNE ligne, 0 capteur, 0 enrôlement**. Le fichier de
   l'invité a dû être **réparé**. Remède : `.Replace()` **littéral**.
2. Sans fenêtre, l'agent mono-fenêtre **meurt immédiatement** :
   `Error: aucune fenêtre visible dont le titre contient « firefox »` —
   `WINDOW_TITLE` retombe sur `"firefox"`
   (`agent/src/demarrage/source.rs:52`), absent de cette VM. Remède : ouvrir un
   Bloc-notes en session 1 et viser **son** titre.

## 4. Le bras ROUGE — et la fausse attribution que j'ai dû retirer

`PRESSE_PAPIER=0 ACCENT=0`, variables **à la bonne position** (lignes 35–36
contre l'invocation ligne 62), **1 fenêtre servie**, et :

```
WARN fenetre{session=…:w-1}: agent::accent: accent de fenetre DESARME (ACCENT=0) …
WARN agent::presse_papier: presse-papier DESARME (PRESSE_PAPIER=0) …      (×2)
messages 'presse-papier de la VM'       : 0
annonces 'accent de la fenetre Windows' : 0
```

🔴 **DEUX PREMIÈRES TENTATIVES DE CE BRAS ONT RENDU `fenetres SERVIES : 0`, ET
J'AI ATTRIBUÉ CE ZÉRO AU MAUVAIS DÉFAUT.** J'ai écrit — ici et dans
`CLAUDE.md` — qu'il était « bloqué par le défaut du lot 31 »
(`Catastrophic failure 0x8000FFFF`). **C'ÉTAIT FAUX.**

La vraie cause : le pilote de recette était invoqué **sans son paramètre
`APP`**, donc il retombait sur son motif par défaut
`chrome|edge|bloc.?notes|notepad`, qui retient **Microsoft Edge en premier** —
l'application dont **cette campagne venait elle-même de mesurer** (item 7)
qu'elle n'est **jamais adoptée** (règle d'appartenance). Relevé qui l'a
établi, sur les journaux des trois bras :

```
item8 / temoin : "nom":"Microsoft Edge"
item8 / mono   : "nom":"Microsoft Edge"
item8 / rouge  : "nom":"Microsoft Edge"
```

Avec `APP='^Notepad$'` imposé, le bras rouge sert **1** fenêtre et les deux
traces sortent.

🔴 **ET CELA RETIRE AUSSI UNE AFFIRMATION QUE J'AVAIS ÉCRITE DANS LES PIÈGES
TRANSVERSES** : je concluais que les traces de désarmement étaient
**impossibles à produire** parce qu'elles vivent dans un `OnceLock::get_or_init`.
Le mécanisme est réel — sans fenêtre servie, `actif()` n'est jamais appelé — **mais
l'exemple était faux** : elles sortent parfaitement dès qu'une fenêtre est
servie. La réserve utile, conservée dans `CLAUDE.md`, est désormais énoncée
**dans les deux sens**, avec le compte de fenêtres servies à relever à côté.

⚠️ **`fenetres SERVIES` est devenu un garde du script**, imprimé à chaque bras :
un bras qui rend zéro est à rejeter, quelle qu'en soit la cause.

## 5. Ce que cet item N'ÉTABLIT PAS

- ✅ ~~**LA ROUGE PRESCRITE PAR LE PLAN EST BLOQUÉE PAR UN DÉFAUT PRODUIT
  NOMMÉ**~~ **RETIRÉ : C'ÉTAIT UNE FAUSSE ATTRIBUTION, LA MIENNE.** La rouge
  est **acquise** (§ 4) ; ce qui la bloquait était un paramètre d'instrument
  non passé, pas le défaut du lot 31. **L'attribution erronée a vécu le temps
  d'un commit, et elle est corrigée ici et dans `CLAUDE.md`.**
- 🔴 **LE ZÉRO DU MODE MONO-FENÊTRE N'EST PAS ATTRIBUÉ PAR LA SEULE MESURE** :
  l'agent y meurt à l'activation de l'encodeur (défaut du lot 31), donc
  « pas de propriétaire » et « pas de session » restent superposés.
- ⚠️ **UNE SEULE EXÉCUTION PAR BRAS.** La règle des deux exécutions n'est pas
  satisfaite ; les bras rejetés ont consommé les créneaux.
- ⚠️ **Le niveau 2 du presse-papier** — qu'un humain puisse coller — reste
  **non mesurable**, comme depuis P1.
- ⚠️ **Aucune ligne de produit n'a été modifiée**, et le défaut d'encodeur en
  mono-fenêtre n'est **pas** de cet item : il est celui du lot 31, rencontré
  ici sur un autre chemin.

Journaux bruts versionnés : `segment-{temoin,mono,rouge}.log`,
`comptes-{temoin,mono,rouge}.log`, `copieur-{temoin,mono,rouge}.txt`,
`pilote-{temoin,mono,rouge}.log`.
