# Lot 3, item 10 (3.5) — une cause naturelle de mort de capture audio

**Mesuré le 5 septembre 2026.** ⚠️ **Aucun chiffre de ce document ne se
recopie sans relancer sa commande.**

---

## 1. Le verdict

🔵 **AUCUNE DES DEUX CAUSES RÉELLEMENT TENTÉES NE TUE LA CAPTURE.** Une
troisième n'a **pas pu être tentée**, faute d'outil sur la VM.

| bras | la cause a-t-elle été appliquée ? | mort de capture |
| --- | --- | --- |
| **témoin** — `AUDIO_FAUTE_LECTURE=15` | oui, par injection | 🔴 **1 — la mort est VUE** |
| `defaut` — changer le rendu **par défaut** | **NON** — aucun module `AudioDeviceCmdlets` | *(non tentée)* |
| `veille` — désactiver puis réactiver le point de terminaison | **oui** (`Speakers (Steam Streaming Speakers)`, état final `OK`) | **0** |
| `service` — `Stop-Service Audiosrv` puis `Start-Service` | **oui** (`Running → Stopped → Running`) | **0** |

**Une cause par exécution du binaire** : ces API échouent par **plantage du
processus**, pas par code d'erreur, et deux causes dans la même exécution
rendraient l'attribution impossible.

## 2. 🔴 LE TÉMOIN NÉGATIF — L'INSTRUMENT VOIT UNE MORT QU'ON PROVOQUE

**C'est le bras qui décide de la valeur de tout le reste.** La conclusion de
cet item est une **absence** ; une absence rendue par un instrument incapable
de voir la chose ne vaut rien. On provoque donc une mort, et on la relève :

```
injection de fautes de lecture audio ARMEE (banc)        : 1
faute injectée (AUDIO_FAUTE_LECTURE)                     : 15
lecture audio échouée, nouvelle tentative                : 14
🔴 lecture audio échouée, capture arrêtée définitivement : 1
capture audio reconstruite                               : 1
reconstruction de la capture audio refusée               : 0
```

🔵 **Les comptes tombent sur les constantes**, relues dans le code :
`LECTURES_ECHOUEES_MAX = 10` (`agent/src/audio.rs:118`),
`RECONSTRUCTIONS_MAX = 3` (`:83`). Quinze fautes injectées, quatorze
re-tentatives, **une** mort définitive, **une** reconstruction — la quinzième
faute n'est pas une re-tentative, c'est celle qui tue.

⚠️ **Les six chaînes comptées sont celles du CODE QUI LES ÉMET**, relues le
jour même : `agent/src/windows_audio/fil.rs` pour les quatre premières,
`agent/src/transport/piste_audio.rs:335` et `:379` pour les deux dernières.
**Aucune ne vient du plan.**

## 3. Le témoin de vie DANS chaque bras de cause

Un « 0 mort » rendu par un bras **sans capture audio** ne dirait rien. Chaque
bras de cause porte donc la preuve qu'une capture existait **et lisait** :

```
agent::windows_audio: source audio démarrée format=process loopback pid=2376 — 48000 Hz, 2 canaux
agent::demarrage::audio: audio activé format="process loopback pid=2376 …"
agent::transport::evenements: piste négociée mid=Mid(1) kind=Audio direction=SendOnly
agent::transport::piste_audio: ordre audio applique session=…:w-1 actif=true capture_morte=false
agent::windows_audio::fil: compteurs audio pid=2376 actif=true rejetes=0 complements=1
```

🔵 **La dernière ligne est prise APRÈS que la cause a été appliquée** :
`actif=true`, `rejetes=0`. La capture était vivante et lisait ; la cause ne l'a
pas tuée. **1 fenêtre servie** dans chacun des trois bras de cause.

## 4. Ce que chaque cause a réellement fait

- **`veille`** — un seul point de terminaison `AudioEndpoint` de statut `OK`
  existe (`Speakers (Steam Streaming Speakers)`). Il a été **désactivé**
  (`Disable-PnpDevice`), laissé 12 s, puis **réactivé** ; état final relu :
  `OK`. La capture a survécu.
- **`service`** — `Audiosrv` relu **avant** (`Running`), **pendant**
  (`Stopped`, 12 s) et **après** (`Running`). La capture a survécu.
- **`defaut`** — 🔴 **CETTE CAUSE N'A PAS ÉTÉ TENTÉE, ET IL FAUT LE DIRE**
  plutôt que d'écrire « elle ne tue pas ». Le module `AudioDeviceCmdlets`, qui
  fournit `Set-AudioDevice`, **n'est pas installé** : relevé
  `Get-Module -ListAvailable -Name AudioDeviceCmdlets` → **0 module(s)**.
  Trois périphériques de rendu sont bien vus, mais rien ne permet de basculer
  le **défaut** sans installer un outil — ce que ce lot ne fait pas.

## 5. Ce que cet item N'ÉTABLIT PAS

- 🔴 **UN ÉCHEC À TROUVER N'EST PAS UNE PREUVE D'ABSENCE**, et c'est le
  livrable autant que le verdict : **la liste de ce qui a été tenté** est au
  § 1, et deux causes sur trois seulement ont été appliquées.
- 🔴 **LA TROISIÈME CAUSE RESTE DUE**, et elle est **bloquée par le
  provisionnement, pas par le produit** : il faudrait installer
  `AudioDeviceCmdlets` sur l'invité. C'est la même famille que les deux
  chemins micro rattachés à la dette **C7** — l'invité porte
  `provision_version=B1` quand le dépôt déclare `B4`. **Ne pas installer
  d'outil à la main pour lever cette réserve** : ce serait faire diverger
  l'invité de sa charge utile un peu plus.
- ⚠️ **UNE SEULE EXÉCUTION PAR CAUSE.** La règle des deux exécutions n'est pas
  satisfaite.
- ⚠️ **La liste des causes n'est pas exhaustive** : débranchement physique,
  changement de fréquence d'échantillonnage, mise en veille de la machine,
  bascule de session — aucune n'a été tentée.
- ⚠️ **Le témoin mesure une mort par ÉCHEC DE LECTURE**, la seule voie que
  `AUDIO_FAUTE_LECTURE` sait provoquer. Une cause naturelle qui tuerait la
  capture **par un autre chemin** (un plantage du processus, par exemple)
  ne porterait pas forcément la même signature — et l'instrument, réglé sur
  ces six chaînes, pourrait ne pas la voir.
- ⚠️ **Aucune ligne de produit n'a été modifiée.**

Journaux bruts versionnés : `segment-{temoin,defaut,veille,service}.log`,
`comptes-{temoin,defaut,veille,service}.log`,
`pilote-{temoin,defaut,veille,service}.log`.
