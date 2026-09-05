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
| ~~**rouge** — `PRESSE_PAPIER=0 ACCENT=0`~~ | ~~0~~ | ~~0~~ | **NON (0)** | oui | 

🔴 **LE BRAS ROUGE EST REJETÉ, PAS ENCAISSÉ** — voir § 4.

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

## 4. 🔴 LE BRAS ROUGE EST REJETÉ — DEUX FOIS

`PRESSE_PAPIER=0 ACCENT=0`, variables **à la bonne position** (lignes 35–36
contre l'invocation ligne 62), agent vivant, capteur lancé
(`capteur démarré tube="\\.\pipe\agent-capteur"`), et **0 message, 0 accent**.

**Ce zéro ne prouve rien**, pour une raison mesurée :

```
fenetres SERVIES : 0     (bras témoin : 21 lignes)
```

**Aucune fenêtre n'a été servie**, donc **aucun accent n'était possible**, avec
ou sans `ACCENT=0`. Le zéro est rendu par l'absence de fenêtre, pas par le
désarmement — *une rouge qui rougit pour la mauvaise raison*.

⚠️ **ET UNE SECONDE ANOMALIE, NON EXPLIQUÉE** : les traces de désarmement que
`CLAUDE.md` promet — `presse-papier DESARME (PRESSE_PAPIER=0)` et
`accent de fenetre DESARME (ACCENT=0)`, qui existent bien dans le code
(`agent/src/presse_papier.rs:96`, `agent/src/accent.rs:238`, toutes deux dans
un `OnceLock::get_or_init`) — **ne sont sorties ni l'une ni l'autre**. Elles ne
peuvent sortir que si `actif()` est **appelé**, ce qui n'a pas eu lieu.
**Consigné, pas expliqué.**

🔴 **LE BRAS ROUGE RESTE DÛ.** Il a été rejoué une seconde fois après purge des
fenêtres résiduelles (`notepad restants : 0`) et a **de nouveau** servi zéro
fenêtre. La cause n'est pas établie.

## 5. Ce que cet item N'ÉTABLIT PAS

- 🔴 **LA ROUGE PRESCRITE PAR LE PLAN N'EST PAS ACQUISE** (étape 4 de la
  Task 9) : deux tentatives, deux rejets, aucune fenêtre servie.
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
