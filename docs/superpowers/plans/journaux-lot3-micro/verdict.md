# Lot 3, item 9 (3.4) — les deux replis micro, et la faute jamais armée

**Mesuré le 5 septembre 2026.** ⚠️ **Aucun chiffre de ce document ne se
recopie sans relancer sa commande.**

---

## 1. Le cas NOMINAL est INATTEIGNABLE sur cette VM, et c'est mesuré

```
cable de rendu retenu   : 0 occurrence sur 168 940 lignes
micro indisponible      : 455
pas de micro            : 455
```

L'appliance **ne porte pas VB-Audio**. La désignation intégrée `"VB-Audio"`
(le défaut, en l'absence de `MICRO_PERIPHERIQUE`) ne correspond à rien, et
chaque session le dit :

```
WARN agent::demarrage::micro: micro indisponible, la session continue sans
  session=…:w-29
  erreur=aucun peripherique de rendu ne correspond a « VB-Audio »
  (MICRO_PERIPHERIQUE) : pas de micro.
  Disponibles : «Speakers (Steam Streaming Speakers)»
  [{0.0.0.00000000}.{fc5b7410-2341-47a7-88a1-894b051a02f1}].
  Le cable virtuel est-il installe ?
```

🔴 **L'étape 1 du plan — le témoin, `integree=true` et un `critere=` — n'est
donc PAS jouable ici.** Ce n'est pas un échec de mesure : c'est un état de
provisionnement, relevé.

## 2. Repli `Introuvable` : **EXERCÉ**, et il l'est en permanence

🔵 **455 fois**, une par session, sans que rien ne soit armé. Le comportement
attendu par le plan est celui qu'on observe :

- **aucun fil de rendu** — `micro indisponible, la session continue sans` ;
- **le `warn!` porte l'INVENTAIRE** des points de terminaison, avec leur
  identifiant d'endpoint ;
- **aucun repli vers le défaut de Windows** : `Speakers (Steam Streaming
  Speakers)` est disponible et **n'est pas retenu**. C'est l'arbitrage que la
  spec exige — « la voix de l'utilisateur dans le mauvais tuyau » serait une
  fuite, pas un moindre mal — et il tient.

**Bras délibéré, en plus des 455 naturels** : `MICRO_PERIPHERIQUE=NVIDIA`
posée dans le `run-agent.ps1` **généré sur la VM**, puis relue à sa place :

```
ligne 35 : $env:MICRO_PERIPHERIQUE = 'NVIDIA'
ligne 61 : & 'C:\nivuus\agent\agent.exe' *>&1 |
```

🔴 **La POSITION est le contrôle, pas la présence** : ligne 35 **avant**
l'invocation ligne 61. Le journal confirme que la valeur a atteint le
processus — le refus nomme `« NVIDIA »`, pas `« VB-Audio »`.

## 3. Repli `Ambigu` : **NON EXERÇABLE sur cette VM**, et voici pourquoi

`Win32_SoundDevice` liste **trois** périphériques — `NVIDIA High Definition
Audio`, `Steam Streaming Speakers`, `NVIDIA Virtual Audio Device (Wave
Extensible) (WDM)` — dont **deux** portent la sous-chaîne `NVIDIA`. C'est sur
cette base que la sous-chaîne a été choisie.

**Le bras a rendu `Introuvable`, pas `Ambigu`.** La raison est dans le message
lui-même : l'agent n'énumère pas les *périphériques*, il énumère les **points
de terminaison de RENDU ACTIFS**, et il n'y en a **qu'un seul** :
`«Speakers (Steam Streaming Speakers)»`. Les deux sorties NVIDIA sont des
périphériques sans endpoint actif (aucun écran branché sur les sorties HDMI).

🔴 **Avec un seul point de terminaison actif, AUCUNE sous-chaîne ne peut être
ambiguë** : le repli `Ambigu` est structurellement inatteignable ici. **Et
c'est une leçon d'instrument** : l'inventaire qui compte est celui que le
produit imprime, jamais celui de `Win32_SoundDevice` — j'ai choisi ma
sous-chaîne dans le mauvais, et seule la mesure l'a dit.

## 4. `MICRO_FAUTE_ECRITURE` : toujours jamais armée, et INARMABLE ici

```
injection de fautes d'ecriture du micro ARMEE : 0 occurrence
```

Elle fait échouer les *n* prochaines **écritures WASAPI sur le câble**
(`agent/src/wasapi/ecriture.rs:392` et `:440`). **Sans câble, aucune écriture
n'a lieu, donc aucune faute ne peut être consommée** : le budget ne pourrait
pas quitter zéro, et ce serait la panne de mesure exacte que D10 a payée. Le
bras n'a donc pas été monté — **délibérément, et pour une raison mesurée**.

## 5. Ce que cet item ÉTABLIT contre `CLAUDE.md`

⚠️ **`CLAUDE.md` écrit, au § « Par sous-projet », ligne E : « deux replis
livrés et JAMAIS COURUS ». C'est faux de l'un des deux.** Le repli
`Introuvable` court **455 fois** sur ce seul journal, à chaque session, depuis
cinq jours. Ce qui reste vrai de l'affirmation : `Ambigu` n'a jamais couru — et
ce document ajoute qu'il **ne peut pas** courir sur cette machine.

## 6. Ce que cet item N'ÉTABLIT PAS

- ⚠️ **Le chemin nominal du micro n'est pas éprouvé** : sans VB-Audio, ni
  `cable de rendu retenu`, ni `mic: true`, ni `critere=`.
- ⚠️ **Le repli `Ambigu` n'est pas éprouvé**, et ne peut l'être qu'en activant
  un second point de terminaison de rendu sur cette VM.
- ⚠️ **`MICRO_FAUTE_ECRITURE` n'est pas éprouvée**, et ne peut l'être qu'après
  le câble.
- ⚠️ **Personne n'a écouté quoi que ce soit** : aucun jugement d'écoute n'est
  porté ici, conformément au périmètre du lot.

Journal brut : `replis-20260905T0101.log`.
