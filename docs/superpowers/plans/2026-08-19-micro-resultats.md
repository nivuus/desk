# Chantier E — Microphone, bloc E1 : résultats

**Date** : 19 août 2026
**Plan** : `docs/superpowers/plans/2026-08-19-micro.md`
**Spécification** : `docs/superpowers/specs/2026-07-28-micro-design.md` (28 juillet 2026)
**Pièces** : `docs/superpowers/plans/journaux-micro/` — **58 fichiers suivis par git**

> **Document PERMANENT et versé.** Le sous-bloc D9 a perdu **six** constats de
> revue parce que son espace de travail (`.superpowers/sdd/`) était gitignoré et
> n'a jamais été commité — il ne restait que quatre phrases invitant à « relire »
> un rapport qui n'existait plus. **La preuve d'une affirmation portée par
> `CLAUDE.md` vit ici, pas dans un rapport de tâche.**

---

## Note de lecture des journaux — DEUX familles, et un piège d'encodage

| Famille | État | Ce qu'il faut faire |
| --- | --- | --- |
| les neuf `agent-*-plat.log` | UTF-8, **ANSI déjà retirées** (`grep -c $'\x1b'` = 0 sur les neuf) | rien |
| les neuf `agent-*.log` bruts | UTF-8, **séquences ANSI PRÉSENTES** (67 à 216 par fichier) | `sed 's/\x1b\[[0-9;]*m//g'`, ou lire le `-plat` jumeau, versé pour chacun |

⚠️ **Un seul fichier du répertoire exige `grep -a` : `vbcable-etat.log`**, qui
porte **16 octets NUL** et du mojibake (« Nom publio?=: ») — c'est le défaut à
deux réglages déjà documenté (`build-agent.sh` / `run-agent.sh` ne posent pas
`[Console]::OutputEncoding`), **toujours non corrigé**. Sans `-a`, `grep` classe
le fichier « binaire » et **rend une sortie VIDE, indiscernable d'un compte
nul**. `vbcable-format.log` est propre.

---

## 1. Ce que le bloc E1 livre, et où il s'arrête

E1 porte la voix du navigateur **jusqu'au PCM décodé dans l'agent**, et pas un
pas plus loin. La chaîne exercée est : `OscillatorNode` → seconde m-line audio
`sendonly` côté navigateur → RTP → dépaquetisation str0m → `Event::MediaData` →
dépôt → tampon de gigue → décodage Opus → PCM.

**Ce que E1 ne fait PAS, et qui est le bloc E2** : écrire ce PCM sur « CABLE
Input ». **Aucune application Windows n'entend quoi que ce soit à ce jour.**

Trois travaux ont convergé dans cette branche, et il faut les distinguer :

| Travail | Objet | Statut |
| --- | --- | --- |
| **E1** | le sens montant, jusqu'au PCM décodé | livré, 4 critères recettés |
| **« A-bis »** | le périphérique audio capté se **désigne**, il ne se subit plus | livré, 7 phases sur la VM |
| **le plafond de dissimulation** | la dissimulation Opus cesse au bout de 200 ms | **né de la recette E1**, rouge et vert sur la VM |

---

## 2. Le résultat le plus utile n'était pas un critère : c'est un défaut que la recette a exhibé

**Le critère ③ demandait « le silence ne coupe pas le flux ». Il est TENU. Et
c'est en lisant les VALEURS des lignes présentes — pas leur nombre — que le
défaut est apparu.**

Pendant les **60 s** de silence du navigateur (son DTX nominal : `packetsSent`
strictement figé à 1010 entre deux relevés distants de 60 s,
`pilote-silence-520.json`), la trace `micro mesuré` **ne s'interrompt jamais** —
133 lignes, une par seconde, `echantillons=48000` sur **chacune**. Le critère
littéral est donc satisfait.

**Mais ce que le puits rendait n'était pas du silence** :

| Phase | `deposees`/s | `plc`/s | `crete` | `frequence_hz` |
| --- | --- | --- | --- | --- |
| ton | 50–51 | 0 | 1,000 | **520,0** |
| **silence (60 s)** | **0** | **50** | **0,53 à 0,67** | **324 à 466, errante** |
| reprise | 50–51 | 0 | 1,000 | **520,0** |

C'est un **bourdon continu**. Au bloc E2 il serait sorti sur le câble virtuel,
donc dans l'application Windows : **un utilisateur qui se tait aurait fait
entendre un bourdonnement.** Pièce :
`journaux-micro/agent-silence-520-plat.log`.

### Ce n'est PAS un défaut de libopus, et c'est ce qui rend le plafond nôtre

Lu dans la source vendorée par `audiopus_sys` 0.2.2, **et non de mémoire** :

- `celt/celt_decoder.c:537` — dès la **6ᵉ perte consécutive**, CELT abandonne
  l'extrapolation par le pitch et bascule sur du **bruit** ;
- `celt_decoder.c:562` et `:566` — l'énergie décroît de 0,5 dB par trame, **mais
  bornée par le bas** : elle converge vers le plancher de bruit de fond **et s'y
  maintient**. C'est de la génération de bruit de confort, délibérée. **libopus
  ne s'arrête JAMAIS de lui-même** ;
- `silk/PLC.c:250-254` — côté SILK, l'atténuation est **saturée** dès la 2ᵉ perte.

**Borner la durée dissimulée était donc à NOUS, et à personne d'autre.**

### Le remède, et sa mesure

`agent/src/micro/dissimulation.rs` — **pur, aucun `cfg`** : un budget de **durée
consécutive** (`PLAFOND_DISSIMULATION = 200 ms`), remis à zéro par toute vraie
trame. Au-delà, on rend du **silence**. Compteur `plc_plafonnees`, **disjoint de
`plc`** — c'est ce qui rend le mécanisme observable au lieu de le masquer.

**Une exécution par bras, sur la VM. Aucun taux.** Fenêtre de silence, **59
lignes de chaque côté** (comptes symétriques) :

| Grandeur | **ROUGE** (`agent-plc-rouge-plat.log`) | **VERT** (`agent-plc-vert-plat.log`) |
| --- | --- | --- |
| `plc` / s | **50**, sur les 59 s | **0** |
| `plc_plafonnees` / s | **0**, sur les 59 s | **100** |
| `crete` | **0,526 à 0,673** | **0,000** sur les 59 lignes, sans exception |
| `frequence_hz` | **305,5 à 398,5**, errante | **« aucune »** sur les 59 lignes |

**Le rouge est un vrai rouge par CONDUITE** : le mécanisme observé (`plc` qui
court à 50/s) est **présent**, et le résultat (`plc_plafonnees` qui mord) est
**absent**. C'est la forme que D10 avait nommée après avoir produit un rouge
**vacueux** — un binaire témoin où le mécanisme observé n'existait pas du tout.

**La seconde de bascule vaut le plafond à la trame près** : une ligne unique
porte `plc=10 plafonnees=75`, soit **10 trames de 10 ms = 200 ms**. Le rallumage
n'est pas retardé (reprise immédiate à la seconde suivante, `crete=1,000`).

**Non-régression** (`agent-plc-vert-ton-440-plat.log`, 1 exécution) : dans la
fenêtre du ton, **61 lignes, toutes à `plc_plafonnees=0`**, et `frequence_hz=440,0`
sur 59 d'entre elles. **Le plafond ne mord jamais tant que la parole coule.**

**Six mutations, six tuées** (`plc-plafond-mutations.txt`), dont M1 qui **remet
le défaut d'origine en place** (3 tests tombent) et M4 qui porte le plafond à
60 s (5 tests tombent). Empreintes SHA-256 des deux fichiers **relevées avant et
après la campagne, identiques** ; référence saine `534 passed; 0 failed` aux deux
bouts.

---

## 3. Les critères de la recette E1, avec leur nombre d'exécutions

**SIX exécutions d'agent distinctes** — quatre vertes, **deux rouges de nature
différente** —, identifiées par leur champ `session=`. **Aucun taux n'est
revendiqué nulle part.**

| Journal | `session=` | Rôle | Durée d'injection |
| --- | --- | --- | --- |
| `agent-ton-440-1-plat.log` | `e1-ton-440-1` | vert, 440 Hz | 60 s |
| `agent-ton-660-2-plat.log` | `e1-ton-660-2` | vert, 660 Hz | 60 s |
| `agent-ton-880-3-plat.log` | `e1-ton-880-3` | vert, 880 Hz | ⚠️ **30 s** |
| `agent-silence-520-plat.log` | `e1-silence-520` | vert, critère ③ | 520 Hz |
| `agent-rouge-sans-variable-plat.log` | `e1-rouge-sans-variable` | **rouge de la VARIABLE** | — |
| `agent-rouge-avant-e1-plat.log` | `e1-rouge-avant` | **rouge du BINAIRE** | — |

⚠️ **Le ton 880 tourne 30 s et non 60** : ses cumuls ne se comparent pas aux deux
autres sans division.

| # | Critère | Verdict | Exéc. |
| --- | --- | --- | --- |
| ① | Deux m-lines audio de **directions opposées** | **TENU** | **5** |
| ② | Le ton traverse, et c'est **LE BON** | **TENU**, avec une réserve d'amorçage | **3** |
| ③ | Le silence ne coupe pas le flux | **TENU** — et il a exhibé le défaut du §2 | **1** |
| ④ | La latence **ajoutée par l'agent** | **TENU**, et il est **PARTIEL par construction** | **4** |
| ⑥ | La charge ajoutée au lien | **RELEVÉE, sans conclusion** | **4** |

### ① Les deux m-lines — 5 exécutions

**Trois** lignes `piste négociée` à chacune des cinq, **trois mids distincts**,
les deux audio de directions **opposées** :

```
mid=Mid(0) kind=Video direction=SendOnly
mid=Mid(1) kind=Audio direction=SendOnly    <- le son du chantier A, agent -> navigateur
mid=Mid(2) kind=Audio direction=RecvOnly    <- le micro du chantier E, navigateur -> agent
```

**Le rouge du BINAIRE discrimine, et de deux façons** : `agent-rouge-avant-e1-plat.log`
ne porte que **deux** lignes, et **le champ `direction` n'y existe pas du tout**.
Corroboré côté navigateur : `pilote-rouge-avant-e1.json` rend
`"erreur": "aucun transceiver sendonly"`.

### ② Le ton traverse, et c'est le bon — 3 exécutions

Jugé sur `frequence_hz` de la trace `micro mesuré`, c'est-à-dire **sur le PCM
décodé côté agent** — jamais sur un compte d'octets (doctrine de D7 : une piste
dont `bytesReceived` croît peut porter un spectre à −1000 dB).

| Exécution | Cible | Lignes sur cible | Écart |
| --- | --- | --- | --- |
| 440 | 440 Hz | **59** à `440.0` | **0,0 %** |
| 660 | 660 Hz | **59** à `660.0` | **0,0 %** |
| 880 | 880 Hz | **29** à `880.0` | **0,0 %** |

⚠️ **L'ÉNONCÉ LITTÉRAL DU CRITÈRE N'EST PAS TENU, et il faut le dire.** Le plan
exigeait la cible « **à chaque ligne de la fenêtre** ». Ce n'est pas le cas :
**1 ligne (440), 2 (660), 2 (880)** sont hors cible **à l'intérieur** de la
fenêtre d'injection. Ce sont des **transitoires d'amorçage**, lisibles comme tels
sur la ligne elle-même — `frequence_hz=276.5` avec `deposees=40 famines=28
deposees_total=40`, c'est-à-dire le tampon en train de se remplir. **La
conclusion tient ; la formulation du critère, non** — elle aurait dû exclure
l'amorçage, comme le fait tout palier de mesure de ce dépôt.

⚠️ **La résolution du relevé n'est pas déclarée.** La trace ne porte aucun champ
`resolution_hz` ; la présence de valeurs en demi-hertz **laisse INFÉRER** un pas
de 0,5 Hz. **Inféré, non mesuré** — et c'est une lacune de l'instrument, qui
devrait rendre sa résolution avec son résultat comme le fait `spectre.rs`.

### ③ Le silence — 1 exécution

Voir le §2 : **tenu sur ce qu'il jugeait, et c'est ce qu'il ne jugeait pas qui
importait.**

### ④ La latence ajoutée — 4 exécutions

**Le critère est PARTIEL par construction, et le plan le disait d'avance** : en
E1 il n'y a pas encore d'écriture sur le câble. La borne mesurable est **dépôt →
retrait**, c'est-à-dire l'occupation du tampon.

| Exécution | `occupation_max_ms` | Marge au plafond (200 ms) |
| --- | --- | --- |
| ton 440 | **160** | 40 |
| ton 660 | **180** | **20** |
| ton 880 | **180** | **20** |
| silence 520 | **180** | **20** |

⚠️ **Deux réserves que les pièces imposent :**

1. **Le maximum n'est atteint qu'UNE fois par exécution**, toujours dans les 1–2
   premières secondes — **la même ligne transitoire** que celle qui porte la
   fréquence hors cible au ②. Le régime établi est **120 ms**, plat.
2. **La marge est de 20 ms sur 3 exécutions sur 4**, soit 10 % du plafond.
   **Aucune pièce ne dit si c'est confortable.**

🔴 **NE PAS LIRE CE CRITÈRE COMME UNE LATENCE.** La latence de bout en bout n'est
mesurée par **aucun** sous-bloc du chantier D à ce jour, et **E1 ne la mesure pas
davantage** : l'occupation du tampon n'en est qu'une composante.

### ⑥ La charge ajoutée — 4 exécutions, relevée sans conclusion

| Exécution | **Micro montant** (mid 2) | Vidéo descendante | `packetsLost` |
| --- | --- | --- | --- |
| ton 440 | **32 363 b/s** | 2 434 645 b/s, 56,8 i/s | **0** |
| ton 660 | **32 406 b/s** | 2 546 296 b/s, 61,7 i/s | **0** |
| ton 880 | **32 358 b/s** | 2 465 424 b/s, 57,5 i/s | **0** |
| silence 520 | 14 732 b/s | 2 568 107 b/s, 59,5 i/s | **0** |

**Le micro coûte ≈ 32,4 kb/s quand il parle**, très stable sur les trois tons.
**`packetsLost` = 0 sur les 46 relevés des neuf pilotes, sans exception.**

⚠️ **Les neuf exécutions se sont jouées SANS RELAIS, sur candidats `host`** :
**exactement un** `WARN allocation TURN impossible : la session continue sans
relais` par journal, **neuf au total** (relevé par la commande sur les neuf
`-plat`). **Zéro `ERROR` sur les neuf journaux.**

---

## 4. La correction « A-bis » — le périphérique se désigne, il ne se subit plus

### Le défaut, et pourquoi le remède n'est pas celui qu'on croit

L'installation de VB-Cable (préparation de E) a fait basculer le **rendu par
défaut** de Windows sur le câble virtuel, **que rien n'alimente**. Le loopback du
chantier A, qui suivait ce défaut, s'est mis à **capter du silence sans qu'aucune
ligne de journal ne dise pourquoi**.

**Le remède retenu n'est pas « remettre les haut-parleurs par défaut »** : cela
corrigerait l'occurrence en laissant la classe de panne entière, et n'importe
quelle installation audio future la rejouerait. **Le remède est le choix
explicite** — `AUDIO_PERIPHERIQUE`.

### Cartographie établie AVANT de corriger : un seul chemin était en cause

**`LoopbackCapture::open`** (`wasapi.rs`) — donc le mode **mono-fenêtre** et la
sonde `AUDIO_PROBE`. **`pour_processus` (multi-fenêtres, D7+) n'a JAMAIS résolu
d'endpoint** : `ActivateAudioInterfaceAsync(VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK)`
vise un **arbre de processus**, jamais un périphérique. Le multi-fenêtres était
donc structurellement à l'abri de ce défaut, et il reste hors de portée du
remède.

### Ce qui est livré

Règle **PURE** dans `agent/src/wasapi/peripherique.rs` (aucun `cfg`, éprouvée sur
l'hôte), moitié COM dans `agent/src/wasapi/rendu.rs`. **Trois critères, dans cet
ordre** : identifiant d'endpoint exact, nom convivial exact, **sous-chaîne
insensible à la casse**. Convention **valuée** (précédent `MULTIFENETRE_SORTIE`) :
**absente ou vide, le comportement est exactement celui d'avant.**

🔵 **Une sous-chaîne AMBIGUË refuse de trancher, au lieu de prendre le premier.**
Prendre le premier serait retomber sur un **rang d'énumération par la porte de
derrière** — c'est-à-dire exactement le défaut que ce module existe pour
interdire, et que le dépôt a payé sur les index DXGI en D1
(`(index_adaptateur, index_sortie)` est positionnel), corrigé en D2 par une
résolution **par nom**. **La leçon est appliquée d'avance ici, pas après coup.**

### Les sept phases, une exécution chacune

⚠️ **SEPT phases sont versées, et la septième est un témoin de MÉTHODE.**

| # | Phase | `AUDIO_PERIPHERIQUE` | `critere` / `repli` | Relevé |
| --- | --- | --- | --- | --- |
| 1 | `abis-R-rouge-sans-variable` | **absente** | défaut Windows / `false` | **`echantillons=0`, `crete=0`** |
| 2 | `abis-R2-temoin-cable` | absente | défaut Windows / `false` | `echantillons=0` — ⚠️ **témoin de méthode** |
| 3 | `abis-T-temoin-defaut-cable` | absente | défaut Windows / `false` | **`dominante_hz=660,0`** |
| 4 | `abis-V-vert-nom-partiel` | `Steam Streaming` | **nom partiel** / `false` | **`dominante_hz=440,0`** |
| 5 | `abis-X1-introuvable` | `Casque Bluetooth Inexistant` | **repli** / **`true`** | `echantillons=0` |
| 6 | `abis-X2-ambigu` | `Haut-parleurs` | **repli** / **`true`** | `echantillons=0` |
| 7 | `abis-X3-identifiant` | `{0.0.0.00000000}.{8695a111-…}` | **identifiant** / `false` | **`dominante_hz=880,0`** |

**Ce que la phase 3 établit et qui est décisif** : sans la variable, une tonalité
jouée **sur le câble** est bien captée (660,0 Hz). **Le silence du rouge n'est
donc pas une capture morte — c'est un autre endpoint.** Sans ce témoin, phase 1
et « la capture est cassée » seraient indiscernables.

**Dans le cas ambigu, l'agent ne s'arrête pas : il avertit, ÉNUMÈRE les candidats
avec leurs identifiants, et se replie sur le défaut de Windows.** Le champ
`repli=true` est le **seul champ machine-lisible** qui distingue un repli d'une
sélection réussie. Aucun repli n'est silencieux.

**Le son est corroboré indépendamment de l'agent** par un crête-mètre
(`IAudioMeterInformation::GetPeakValue`, `abis-metre.ps1`) : la phase T lit
`crete=0,449982` sur VB-Cable et `0,000000` sur les deux autres endpoints.

### 🔵 L'instrument neuf : `agent/src/spectre.rs`

`AUDIO_PROBE` ne rendait qu'une **crête**, qui distingue « du son » de « rien »
mais **jamais « MON son » d'un autre**. `spectre.rs` (**pur, racine nue, filtre
de Goertzel, aucune dépendance neuve**) rend la **fréquence dominante**, et
**rend sa résolution avec son résultat** plutôt que de la supposer. C'est ce qui
transforme A-bis d'une conjecture en une mesure.

---

## 5. Les sondes d'ouverture

### Sonde 1 — str0m dépaquetise-t-il l'Opus montant ? **OUI** (bloquante, sans VM)

Trois faits relevés : charge utile traversant **octet pour octet**,
`Codec::Opus`, horloge à **48000**. **Aucune dépaquetisation à écrire.**

**La sonde PEUT rendre NON, et c'est éprouvé** (`sonde-1-rouge.txt`, deux
mutations, toutes deux tuées) : récepteur sans `enable_opus` → aucun
`Event::MediaData` en 15 s.

**Trois faits que le plan ne prévoyait pas** : str0m n'émet `MediaAdded` que du
côté qui **accepte** l'offre ; `denom()` et non `denominator()` ; et
`MediaData::data` est un `Arc<[u8]>`, pas un `Vec<u8>`.

### Sonde 2 — VB-Cable

**Installé et fonctionnel** — mais **la silenciosité de l'installation reste
INCONNUE, et le verdict le dit lui-même** : elle a été faite **par le
propriétaire du dépôt, avant la tâche**, avec ajout du certificat aux magasins
`TrustedPublisher` et `Root`. **E2 ne peut pas supposer une installation non
interactive sur une machine neuve** (risque R2, toujours ouvert).

🔴 **LES FORMATS SONT ASYMÉTRIQUES, et c'est le fait le plus important de cette
sonde** (`IAudioClient::GetMixFormat`, mode partagé) :

| Endpoint | Sens | Format |
| --- | --- | --- |
| **CABLE Input** | **rendu** — c'est là qu'E2 écrira | **48000 Hz**, 2 canaux, 32 bits flottant |
| **CABLE Output** | **capture** — c'est ce que l'application lira | 🔴 **44100 Hz**, 2 canaux, 32 bits flottant |

**Notre chemin est à 48 kHz** : le risque R3 (« le format n'est pas 48 kHz »)
**n'est pas éliminatoire**. Mais **VB-Cable rééchantillonnera 48000 → 44100 en
interne, hors de notre code et hors de notre mesure.**

⚠️ **Le remède est ÉCRIT et NON APPLIQUÉ** : l'écriture au registre et le
redémarrage d'`Audiosrv` ont été **refusés par le bac à sable**. Script prêt sur
le partage de la VM, **jamais joué**.

### Sonde 3 — CABLE Output est **DÉJÀ** le microphone par défaut

Les **trois** rôles (`eConsole`, `eMultimedia`, `eCommunications`) rendent CABLE
Output en capture — **sans aucune API non documentée** (`IPolicyConfig` n'a pas
été nécessaire). La dégradation d'usage que la spec §12 redoutait n'a pas lieu.

⚠️ **Deux réserves nommées par le verdict lui-même** : (a) **WinRM tourne en
session 0**, ce relevé n'est donc pas celui de la session interactive où
tourneront les applications — concordance *plausible, non mesurée* ; (b) c'est
cette même sonde qui a relevé que **le rendu par défaut avait basculé sur le
câble**, et l'a nommé « risque de régression sur un chantier LIVRÉ ». **C'est
exactement ce qui s'est produit, et c'est ce qu'A-bis répare.**

---

## 6. Ce que E1 n'établit PAS

- **Aucun taux, nulle part.** 3 exécutions au mieux pour un critère, **1 par bras**
  pour le plafond de dissimulation, **1 par phase** A-bis, **1 par sonde**.
- **Aucune application Windows n'entend quoi que ce soit** : c'est E2.
- **Aucun microphone réel n'est exercé.** L'instrument est un `OscillatorNode`.
  `getUserMedia`, la permission, le choix du périphérique d'entrée, l'AEC, la
  suppression de bruit et **le DTX d'un vrai locuteur qui se tait** ne sont
  éprouvés que par leurs tests d'injection.
- 🔴 **La latence de bout en bout n'est mesurée par RIEN**, ni par E1, ni par
  aucun sous-bloc du chantier D depuis D1. Le critère ④ mesure l'occupation du
  tampon, qui n'en est qu'une composante.
- **`CIBLE`, `PLAFOND`, `SEUIL_SAUT`, `SEUIL_INSERTION` et
  `PLAFOND_DISSIMULATION` ne sont pas calibrées.** ⚠️ **`PLAFOND_DISSIMULATION`
  vaut le même nombre que `micro::PLAFOND` (200 ms) par COÏNCIDENCE, pas par
  dérivation** — les deux bornent des choses différentes et se recalibreraient
  séparément. **Aucun jugement d'écoute n'a jamais été porté sur aucune
  constante de ce dépôt.**
- **La dérive d'horloge n'est pas observée sur une durée longue** : le mécanisme
  est testé sur des seuils, jamais sur dix minutes de conversation. C'est E2.
- **L'exclusivité inter-processus n'est pas exercée** : E1 n'en pose que la
  **couture** (`PuitsMicro::deposer` rend `false`, on journalise une fois). Le
  mutex nommé est E2.
- **Le refus d'un second flux montant n'est pas dit au client** : `ReadyMessage.mic`
  est décidé à l'établissement et ne peut pas exprimer un refus ultérieur.
- **Le rééchantillonnage 48000 → 44100 de VB-Cable** est hors de notre code et
  **hors de toute mesure**.
- **La silenciosité de l'installation de VB-Cable est inconnue**, et le remède de
  format est **écrit, non appliqué**.
- **Le périphérique par défaut de la SESSION INTERACTIVE n'est pas relevé** —
  tous les relevés WinRM sont ceux de la session 0.
- **Toutes les mesures se sont jouées sans relais TURN**, sur candidats `host`.
- **Une seule fenêtre**, une seule application, aucun multi-fenêtres exercé sur
  le chemin du micro.
- **Les trois couches inconnues du chantier D le restent** : le plafond de
  8 encodeurs, celui de 4 processus, et le mécanisme de l'abandon du mutex DXGI.

### Ce que les pièces laissent inexpliqué, et qu'il faut nommer

- ⚠️ **Un `WARN` non commenté, présent aux QUATRE exécutions vertes et à aucune
  autre** : `paquet micro dont la durée Opus est illisible, paquets abandonnés`
  (`agent::transport::piste_micro`), **une seule occurrence par exécution**,
  toujours ~5 s après l'ICE `Connected`. **Aucune pièce n'en dit ni la cause ni
  le nombre de paquets perdus.** Corroboré par un écart navigateur → agent de
  **30, 31 et 13 paquets** sur les trois tons.
- ⚠️ **`framesDropped: 172`** au `fin-reprise` de `pilote-silence-520.json` — le
  **seul** `framesDropped` non nul des neuf pilotes. Non expliqué.
- ⚠️ **`erreursPage: ["Uncaught"]`** dans `pilote-ton-440-1.json` seul, **tronqué
  à ce seul mot**. Non expliqué.
- 🔴 **Le binaire « rouge » du plafond de dissimulation porte DÉJÀ le compteur
  `plc_plafonnees`**, sur ses 133 lignes. **Rien dans son journal n'identifie le
  bras** — ni variable d'environnement, ni ligne d'armement : seuls le nom de
  fichier et la conduite le distinguent. **Un successeur ne peut pas rejouer
  l'attribution sur pièces.** C'est une lacune de méthode, pas de résultat : le
  rouge reste un vrai rouge par conduite (§2), mais il n'est pas **auto-portant**.

---

## 7. Pièges neufs — à connaître avant de toucher à ce terrain

- 🔴 **`cargo clean --release -p agent` NE SUFFIT PAS après un aller-retour de
  sources.** L'horloge de la VM avance sur celle de l'hôte : le rlib du crate
  **`proto`** paraît plus récent que ses sources fraîchement synchronisées, et
  cargo le **saute**. **Le symptôme ne ressemble pas à un cache périmé** — la
  compilation échoue sur une **erreur de type portant sur une signature de
  `proto` pourtant à jour dans le fichier qu'on vient de lire**, et l'on cherche
  un défaut dans du code correct. **Parade : `cargo clean --release -p proto -p agent`.**
  Règle générale : purger le crate qu'on compile ne suffit pas quand une
  dépendance interne du même dépôt a franchi le même partage réseau.
- 🔴 **Un rendu audio lancé depuis WinRM (session 0) n'atteint AUCUN endpoint de
  la session 1**, et le symptôme est `echantillons=0` — **indiscernable d'une
  capture morte**. Jouer le son par **tâche planifiée `/it`**, comme l'agent
  lui-même. Le témoin de méthode est versé (`abis-R2-temoin-cable`), avec son
  journal de tonalité **tronqué** comme pièce.
- 🔴 **`waveOutOpen`, `waveOutPrepareHeader` et `waveOutWrite` peuvent rendre `0`
  tous les trois et NE RIEN JOUER.** Une `WAVEHDR` passée par `[ref]` en
  PowerShell est une **copie marshalée** dont l'adresse meurt au retour de
  l'appel. **Trouvé par le crête-mètre `IAudioMeterInformation`, jamais par un
  code de retour** — c'est la doctrine du dépôt (« juger sur la relecture, jamais
  sur le code de retour », D8) appliquée à l'audio.
- ⚠️ **Un critère de CONTINUITÉ ne dit rien du CONTENU de ce qui continue.** Le
  critère ③ demandait « aucune ligne ne manque », et l'a obtenu — pendant que le
  puits fabriquait un bourdon. **Lire les valeurs, pas seulement les comptes.**
- ⚠️ **Un critère qui exige une propriété « à chaque ligne de la fenêtre » doit
  exclure l'amorçage**, sans quoi son énoncé littéral est faux alors que sa
  conclusion tient (critère ②, 1 à 2 lignes hors cible par exécution).
- ⚠️ **Un instrument de fréquence doit rendre sa RÉSOLUTION avec son résultat.**
  `spectre.rs` le fait ; la trace `micro mesuré` ne le fait pas, et son pas
  (0,5 Hz) n'est qu'**inféré** des valeurs observées.
- ⚠️ **Un binaire témoin doit porter de quoi s'identifier lui-même.** Le rouge du
  plafond de dissimulation ne se distingue du vert que par le nom de son fichier.
- ⚠️ **Quatre mutations ont SURVÉCU au premier jet** sur les sept campagnes de
  tâche, et **deux ont mis au jour un défaut RÉEL, pas un test faible** : une
  mutation prescrite par le plan qui **ne pouvait pas échouer** (elle changeait
  une grandeur qui était à la fois l'entrée et l'attente), et trois tests
  unitaires qui exerçaient **un jumeau du chemin de production** au lieu du
  chemin lui-même. **Un plan n'immunise pas contre le contrôle vacueux — il en
  est une source.**

---

## 8. La revue transverse de fin de branche

**Neuf affirmations devenues fausses**, toutes franchissant une frontière de
tâche. Barème des branches précédentes : D7 **5**, D8 **3**, D9 **6**, D10
**douze**, D11 **sept**, P1 **huit**, P2 **dix**, S1 **cinq**.

| # | Où | Ce qui était faux | Sort |
| --- | --- | --- | --- |
| 1 | `agent/src/micro.rs:5-6` | « **ne décode rien** » — le module décode depuis la tâche 6 (`LecteurMicro` possède l'`OpusDecoder`) | **CORRIGÉ** |
| 2 | `agent/src/micro.rs:89` | `Retrait::Manquante` annonçait **deux** issues ; il y en a **trois** depuis le plafond de dissimulation | **CORRIGÉ** |
| 3 | `agent/src/wasapi.rs:128` | le doc-comment d'`open()` disait « le périphérique de rendu **par défaut** » — alors que **l'en-tête du même fichier, douze lignes plus haut, disait déjà l'inverse** | **CORRIGÉ** |
| 4 | `agent/src/diagnostics.rs:49-52` | l'aiguillage décrivait `AUDIO_PROBE` comme sondant le défaut, et ne mentionnait pas la fréquence dominante — **l'en-tête du fichier voisin, lui, avait été annoté** | **CORRIGÉ** |
| 5 | `agent/src/transport.rs:29` | « `piste_video` et `piste_audio` (**les deux** pistes média) » — il y en a **trois** | **CORRIGÉ** |
| 6 | `agent/src/transport.rs:125` | « `mid` de **la** piste audio » — il y en a deux, et **c'est le champ dont le mauvais renseignement était le défaut muet** que la tâche 7 a corrigé | **CORRIGÉ** |
| 7 | `docs/…/2026-07-28-audio-design.md:468` | « le périphérique par défaut **suffit** ; rien dans le produit ne permet d'en choisir un autre » — **les deux moitiés** réfutées par A-bis | **ANNOTÉ** (précédent : legs n°11 de D8) |
| 8 | `docs/…/2026-08-19-micro.md`, Décision 6 | le plan **RÉSERVE** `agent/src/wasapi/rendu.rs` à E2 — **A-bis l'a pris entre-temps**, pour tout autre chose | **ANNOTÉ aux trois places** |
| 9 | `docs/…/2026-08-19-micro.md`, Décision 6 | le renvoi `agent/src/wasapi.rs:17` a dérivé à **`:25`**, dans cette branche même | **CORRIGÉ** |

**Et une dixième, dans le plan lui-même** : le critère ③ prescrivait
« **et `remplir` continue de rendre du silence** ». **Réfuté par sa propre
recette** — voir le §2. Le plan porte désormais son encadré.

### 🔴 La n°8 est la seule qui porte un risque d'ACTION, pas seulement de lecture

Le plan réserve `agent/src/wasapi/rendu.rs` au bloc E2, pour **rendre** le micro
sur « CABLE Input ». La correction A-bis, jouée dans la **même branche**, a créé
ce fichier (216 lignes) pour **la résolution** du point de terminaison que le
loopback doit *capter*. Les deux emplois parlent de « rendu » et n'ont rien de
commun. **Un implémenteur d'E2 qui suivrait le plan à la lettre écraserait A-bis**,
ou logerait deux sens contraires dans un même fichier. Les trois places du plan
portent la marque, et **E2 doit choisir un autre nom**.

### Ce que cette revue ajoute à la doctrine

**Deux des neuf (n°3 et n°4) sont des ASYMÉTRIES INTERNES** : l'en-tête d'un
module avait bien été corrigé, et la doc de la fonction qu'il décrit — ou du
fichier voisin qui aiguille vers lui — ne l'avait pas été. **Corriger un
en-tête ne corrige pas ce qu'il chapeaute**, et c'est la forme la plus discrète
du défaut que cette revue cherche : celle où quelqu'un **a bien vu le problème**
et l'a traité à un seul endroit.

**Cinq pistes nommées d'avance étaient DÉJÀ traitées** par les tâches
antérieures — l'en-tête d'`opus.rs`, celui de `wasapi.rs`, celui de
`diagnostics/audio.rs`, l'absence de toute affirmation au présent sur la crête,
et l'absence de toute description non bornée du PLC. **C'est une information
utile, et elle dit que la discipline par tâche a fonctionné là où elle
pouvait fonctionner.**

---

## 9. Vérifications de fin de branche

Toutes relancées **après la dernière édition**.

| Vérification | Résultat |
| --- | --- |
| `cargo test -p agent` | **534 passed; 0 failed** |
| `cargo check --target x86_64-pc-windows-gnu` | **sortie 0, 11 avertissements** |

### `scripts/verify-all.sh` — étape par étape, et l'étape qui échoue n'est pas la nôtre

**`scripts/verify-all.sh` rend 1**, et il faut dire exactement où et pourquoi
plutôt que de le présenter comme vert.

| # | Étape | Résultat |
| --- | --- | --- |
| 1 | `cargo test --workspace` | ✅ **534 + 52 + 0 passed, 0 failed** |
| 2 | `cargo clippy --workspace` | ✅ |
| 3 | `client : npm test` | ✅ **179 passed**, 20 fichiers |
| 4 | `client : npm run typecheck` | ✅ |
| 5 | `client : npm run design:verifier` | ✅ les **six** contrôles de script du socle S1 |
| 6 | `proto : npm test` | ✅ **57 passed** |
| 7 | `proto : npm run typecheck` | ✅ |
| 8 | `plateforme : npm run test:sqlite` | 🔴 **1 failed / 193 passed** — **le script s'arrête ici** |
| 9 | `plateforme : npm run test:postgres` | ⚠️ **NON JOUÉE** par le script (arrêt au premier échec) |
| 10 | `plateforme : npm run typecheck` | ⚠️ **NON JOUÉE** pour la même raison |

🔴 **L'unique test qui tombe est
`plateforme/src/agents/canal.test.ts > … > 🔴 le jeton rendu est VÉRIFIABLE PAR
LA GARDE, et de type `agent``.** Ce fichier est **NON SUIVI PAR GIT**
(`git status` : `?? plateforme/src/agents/canal.test.ts`), aux côtés de
`plateforme/src/agents/canal.ts` et de deux fichiers modifiés — c'est le
**travail EN COURS du sous-bloc P3**, qu'un agent concurrent écrivait dans le
même arbre pendant cette clôture. **Son nom porte le marqueur 🔴 de la
discipline rouge-d'abord du dépôt** : c'est un test délibérément rouge, pas une
régression.

✅ **Établi, plutôt que supposé** : les deux suites `plateforme` passent
**intégralement** dès qu'on exclut ce seul fichier en cours d'écriture, **sans
rien modifier** — `PLATEFORME_BASE=sqlite npx vitest run --exclude
'src/agents/canal.test.ts'` → **27 fichiers, 186 passed**, et la même chose en
`postgres` → **186 passed**. **Aucune régression de ce chantier sur
`plateforme`.**

⚠️ **Les étapes 9 et 10 ne sont donc PAS déclarées vertes** : elles n'ont pas
été jouées par le script. La 9 a été jouée **à la main et hors du script**, avec
l'exclusion ci-dessus. **La 10 (`tsc --noEmit` sur `plateforme`) n'a été jouée
sous aucune forme** — elle porterait sur du code d'un autre chantier, en cours
d'écriture, et un verdict pris à cet instant ne dirait rien de durable.

🔴 **« Tous `dead_code` » N'EST PLUS VRAI, et c'est une propriété que ce dépôt
affirmait à chaque clôture depuis D9.** Sur les 11 : **10 `dead_code`**, et
**1 `unused_variables`** — `agent/src/micro.rs:184`, un `let Some(tete) = …
else` dont la liaison n'est jamais lue (le code ne s'en sert que comme test de
vacuité). **Aucune conséquence de comportement**, remède d'un caractère
(`_tete`). **Non corrigé ici par périmètre** — cette tâche ne modifie le code
que pour redresser une affirmation fausse — et **légué** plutôt que dissimulé.

---

## 10. Ce que le chantier E lègue

### Le bloc E2, entier

1. ⛔ **`wasapi/<à nommer>.rs` — le rendu sur « CABLE Input »**, et
   `windows_micro.rs` qui assemble tampon + décodeur + rendu sur un fil dédié.
   ⚠️ **Le nom `wasapi/rendu.rs` est PRIS** (revue transverse n°8).
2. ⛔ **L'exclusivité inter-processus par mutex nommé Windows**, acquis
   paresseusement au premier paquet montant et tenu pour la vie du processus
   enfant. E1 n'en pose que la couture. ⚠️ **Le refus n'est pas dit au client**,
   et `ReadyMessage.mic` ne peut pas l'exprimer.
3. ⛔ **La recette d'écoute** : enregistreur vocal réglé sur CABLE Output, appel
   réel sans écho, **dix minutes sans dérive**.
4. ⛔ **Le format asymétrique de VB-Cable** — 48000 en entrée, **44100 en
   sortie** —, dont le remède est **écrit et non appliqué** (refusé par le bac à
   sable).
5. ⛔ **La silenciosité de l'installation de VB-Cable reste INCONNUE** (risque
   R2). E2 ne peut pas la supposer sur une machine neuve.
6. ⛔ **La licence VB-Audio est gratuite en usage PERSONNEL seulement** (R4) :
   à régler avant mise sur le marché, pas avant E2.

### Le bloc E3, conditionnel

7. ⛔ **L'AEC est structurellement incomplète en multi-fenêtres**, et **ce n'est
   pas réparable dans E**. Depuis D7 chaque fenêtre porte le son de sa propre
   application ; l'AEC de Chrome n'annule que ce que **son propre onglet**
   restitue. Sans casque, le son de la fenêtre B revient dans le micro de la
   fenêtre A, **et l'AEC de A ne le connaît pas**. La corriger demanderait soit
   de rassembler la restitution de toutes les fenêtres dans l'onglet qui capte
   (**ce qui défait D7**), soit une AEC côté agent (hors périmètre).
   **Son exercice est un critère de E2**, avec **deux** fenêtres qui jouent du
   son — pas une.

### Legs propres à E1

8. ⛔ **La latence de bout en bout n'est mesurée par rien**, et ne l'a jamais été
   depuis D1. Le critère ④ n'en mesure qu'une composante.
9. ⛔ **Cinq constantes non calibrées** — `CIBLE`, `PLAFOND`, `SEUIL_SAUT`,
   `SEUIL_INSERTION`, `PLAFOND_DISSIMULATION` —, et **aucun jugement d'écoute**.
10. ⛔ **Le `WARN` « durée Opus illisible »** aux quatre exécutions vertes : cause
    inconnue, nombre de paquets perdus inconnu.
11. ⛔ **`agent/src/micro.rs:184` — l'avertissement `unused_variables`** qui rompt
    la propriété « tous `dead_code` » du dépôt.
12. ⛔ **La trace `micro mesuré` ne rend pas sa résolution en fréquence.**
13. ⛔ **Un binaire témoin doit s'identifier lui-même** : le rouge du plafond de
    dissimulation ne se distingue que par le nom de son fichier.
14. ⛔ **Le périphérique par défaut de la session INTERACTIVE n'a jamais été
    relevé** — tous les relevés WinRM sont ceux de la session 0.
