# Lot 3, item 7 (3.1) — la latence capture → affichage

**Mesuré le 5 septembre 2026**, sur la VM `Windows` (appliance), agent
`C:\nivuus\agent\agent.exe`, binaire **bit à bit identique** à celui que
`scripts/build-agent-croise.sh` produit depuis `ffdc839` :

```
sha256 bâti sur l'hôte  : 3c270c6c2c65d6db7cca261e42afb57902b63d2a76dc2eefc8c3185a6713940c
sha256 déployé sur la VM: 3C270C6C2C65D6DB7CCA261E42AFB57902B63D2A76DC2EEFC8C3185A6713940C
taille                  : 20 583 167 octets, des deux côtés
```

⚠️ **Aucun chiffre de ce document ne se recopie sans relancer sa commande.**

---

## 1. Le verdict, en une phrase

🔴 **LA MÉTHODE PRESCRITE PAR LE PLAN NE PEUT PAS RENDRE DE CHIFFRE SUR CE
PRODUIT, ET LA RAISON EST MESURÉE, PAS DÉDUITE.** `metadata.captureTime` est
**absent de 9 084 trames sur 9 084**, huit sessions, quatre bras. Ce n'est pas
« pas de flux » : les 9 084 trames sont bien arrivées et ont bien été décodées.

🔵 **Un SUBSTITUT a été mesuré à la place, et il est discriminant** — deux
exécutions par bras, la rouge le déplace d'un facteur **4,4**.

## 2. Le tableau des relevés

Substitut = `currentRoundTripTime / 2 + totalProcessingDelay / framesDecoded`.

| étiquette | régime | taille servie | framesDecoded | tpd (ms) | jitter (ms) | RTT (ms) | **substitut (ms)** | trames sonde / sans `captureTime` |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `nominal-1f-A` | 1 | 780×492 | 1854 | 7,677 | 6,841 | 3,0 | **9,177** | 1725 / 1725 |
| `nominal-1f-B` | 1 | 780×492 | 1853 | 8,542 | 7,721 | 1,0 | **9,042** | 1718 / 1718 |
| `nominal-4f-A` | 4 | 780×492 | 3141 | 8,967 | 7,908 | 2,0 | **9,967** | 2375 / 2375 |
| `nominal-4f-A` | 4 | 780×492 | 3317 | 5,846 | 4,903 | 2,0 | **6,846** | 0 / 0 |
| `nominal-4f-A` | 4 | 780×492 | 3567 | 11,273 | 9,970 | 2,0 | **12,273** | 0 / 0 |
| `nominal-4f-A` | 4 | 780×492 | 3863 | 5,361 | 4,226 | 1,0 | **5,861** | 0 / 0 |
| `rouge-adsl-1f-A` | 1 | **390×246** | 1852 | 7,271 | 6,759 | **70,0** | **42,271** | 1651 / 1651 |
| `rouge-adsl-1f-B` | 1 | **390×246** | 1852 | 8,723 | 8,132 | **62,0** | **39,723** | 1615 / 1615 |

Le tableau lisible par machine : `synthese-substitut.json`. Les relevés bruts,
`getStats()` complet compris : `distribution-*.json`.

## 3. La ROUGE, et ce qu'elle déplace

`scripts/netem.sh adsl` (8 Mbit/s, 30 ms, gigue 5 ms), reposé `off` après
**chacun** des deux bras.

- **RTT : 1–3 ms → 62–70 ms**, soit **×23**. La dégradation atteint bien le
  chemin mesuré, et l'instrument la voit.
- **Substitut : 9,04–9,18 ms → 39,72–42,27 ms**, soit **×4,4**.
- 🔵 **Et le PRODUIT réagit, ce qui n'était pas demandé mais est mesuré** : la
  taille servie tombe de **780×492 à 390×246** — l'estimateur de bande passante
  divise la définition par deux, sans que la cadence de trames bouge
  (1852 trames décodées dans les deux bras, contre 1853–1854 au nominal).
- ⚠️ **`totalProcessingDelay` seul ne bouge PAS** (7,27–8,72 contre
  7,68–8,54 ms) — **et c'est correct** : il ne contient pas le réseau. C'est
  précisément pourquoi le substitut lui ajoute `RTT/2` ; l'écrire seul aurait
  fait conclure « la dégradation n'a aucun effet ».

## 4. Le TÉMOIN NÉGATIF de la sonde

🔴 **Une sonde qui ne peut pas rendre `[]` ne prouve rien quand elle rend
autre chose.** Injectée sur `about:blank`, à chaque exécution :

```json
{"page":"about:blank (aucun <video>)","attache":{"videos":0},
 "echantillons":[],"compteurs":{"trames":0,"sans_capture":0,"videos":0}}
```

`videos: 0`, `trames: 0`, tableau vide. Sur la page de session, la même sonde
rend `videos: 1` et jusqu'à **2 500 trames en 60 s**. Le zéro d'échantillons du
bras de mesure n'est donc pas le zéro d'une sonde morte.

## 5. POURQUOI `captureTime` est absent — la cause est TRANCHÉE, pas supposée

Deux causes possibles, qui ne se ressemblent pas : ① l'agent n'annonce pas
l'instant de capture au pair ; ② le navigateur ne le rend pas. Les confondre
attribuerait au produit un défaut du navigateur, ou l'inverse. Trois mesures
indépendantes tranchent :

**① est ÉLIMINÉE — sur le fil.** `instrument/sr-rtcp.sh`, 110 s de capture
(`sr-rtcp-nominal-1.txt`) :

```
udp9_eq_200=204        (sender report RTCP)
udp9_eq_201=154        (receiver report)
udp9_eq_205=2020       (transport feedback)
paquets_udp_total=35446
SR : 204 paquets, TOUS  192.168.3.2.56948 > 192.168.3.1.58406
```

Les sender reports partent bien **de la VM vers l'hôte**, à ~1,85/s.
(En SRTCP l'en-tête n'est pas chiffré, le type de paquet se compte donc.)

**① est ÉLIMINÉE une seconde fois — dans le navigateur.** `getStats()` sur la
page de session rend, pour la piste **vidéo** :

```
remote-outbound-rtp : { kind: "video", reportsSent: 63,
                        remoteTimestamp: 1788561558257.9731, packetsSent: 3495 }
```

Chrome **reçoit ET analyse** ces sender reports : `remoteTimestamp` en est
extrait. Le produit fait donc ce que le plan lui prête.

**② EST LA CAUSE.** Sur la même page, `getSynchronizationSources()` rend :

```
[{ source: 4244494958, timestamp: 1788561558592 }]
```

**Ni `captureTimestamp`, ni `senderCaptureTimeOffset`.** Ces deux champs — et,
avec eux, `metadata.captureTime` — viennent de l'extension d'en-tête RTP
**`abs-capture-time`**, PAS du sender report. Et cette extension n'est
négociée nulle part :

```bash
grep -rn "abs-capture-time\|absolute-capture-time\|extmap" agent/src/ --include='*.rs' | wc -l
# 0
```

🔴 **CE QUE CELA VEUT DIRE POUR LE PLAN.** Le commentaire de la sonde que le
plan dicte — « l'agent annonce l'instant de capture au pair par le sender
report RTCP […] le navigateur le rend dans `metadata.captureTime` » — est
**FAUX dans sa seconde moitié**. La première moitié est vraie et vérifiée
(204 SR sur le fil, 63 reportsSent lus par Chrome) ; la seconde ne l'est pas :
le sender report alimente `remoteTimestamp`, jamais `captureTime`.

⚠️ **LE REMÈDE EST UN CHANGEMENT DE PRODUIT, ET IL N'A PAS ÉTÉ FAIT** :
négocier l'extension `abs-capture-time` dans l'agent. Ce n'est pas une
correction d'instrument, cela touche `agent/src/transport/`, et **ce lot
mesure, il ne répare pas**. C'est un legs neuf, chiffré, inscrit tel quel.

## 6. Ce que cette mesure N'ÉTABLIT PAS

- 🔴 **La latence VERRE À VERRE reste non mesurée**, et le substitut ne s'en
  approche pas : il ne contient **ni la capture, ni l'encodage** côté agent
  (le segment que `captureTime` aurait justement ouvert), ni le maillon
  d'affichage après `presentationTime` (compositeur, balayage de la dalle).
  La **boucle d'entrée** n'y est pas du tout.
- ⚠️ **Le substitut n'est pas une latence de bout en bout** : c'est
  `RTT/2 + totalProcessingDelay`, soit le réseau aller simple plus le segment
  « premier paquet reçu → trame rendue au pipeline ». Il **borne par le bas**.
- ⚠️ **La mire du plan n'a PAS été employée.** `it-mire.ps1` ouvre un Chrome
  DANS l'invité ; la règle d'appartenance (lot 32I) écarte toute fenêtre que
  desk n'a pas lancée. L'animation vient donc d'un `pointermove` à **30 Hz**
  dispatché sur l'élément `<video>` — le chemin du produit
  (`client/src/input.ts`), qui déplace le curseur distant. **C'est un autre
  stimulus que celui que le plan prescrivait**, et la cadence de la source
  n'entre dans aucun des chiffres ci-dessus.
- ⚠️ **En régime 4 fenêtres, la sonde ne s'attache utilement qu'à UNE page**
  (2 375 trames contre 0 sur les trois autres), alors que les quatre sessions
  décodent (3 141 à 3 863 trames, relevées par `getStats()`).
  `requestVideoFrameCallback` ne tire que sur la page que Chrome rend ; en
  mode sans interface, une seule l'est. **Les quatre lignes du tableau restent
  valides** — elles viennent de `getStats()`, pas de la sonde — mais un
  relevé de `captureTime` en multi-fenêtres serait, lui, non mesuré.
- ⚠️ **Une seule taille servie a été observée au nominal** (780×492) : le
  substitut n'est pas mesuré à d'autres définitions.

## 7. Ce que la campagne a mesuré au passage, et qui n'est pas cet item

🔴 **`Microsoft Edge`, lancé par desk lui-même, est ÉCARTÉ par sa propre
règle d'appartenance.** Journal du 4 septembre 2026 à 22:26:10 **UTC** — soit le 5 septembre à
00:26 heure locale, l'heure de tout ce document —, dans cet
ordre et à 60 ms d'intervalle :

```
agent::apps::lancement: raccourci lancé chemin="…\Microsoft Edge.lnk"
agent::apps::boucle: lancement demande=… issue=Raccourci
agent::superviseur::hook: fenêtre ÉCARTÉE : desk ne l'a pas lancée
    (règle d'appartenance) titre="New tab - Personal - Microsoft​ Edge"
    pid=7184 processus=msedge.exe
```

**Aucune page de session ne s'ouvre** : 90 s d'attente, aucune cible
`?session=` (relevé complet dans `distribution-*.json`, champ `cibles_vues`).
**`Notepad`, lui, est adopté** et sert une session en 4 s — c'est avec lui que
tout le tableau ci-dessus a été mesuré.

⚠️ **Ce n'est PAS le legs connu du lot 32I**, qui dit qu'une application du
Windows Store paraît sous `ApplicationFrameHost.exe`. Edge n'est pas une
application du Store : il est lancé par un `.lnk`, et le processus qui porte
la fenêtre n'est pas celui que desk a créé. **Le legs est donc plus large que
ce que `CLAUDE.md` en dit**, et le cas mesuré est celui d'un navigateur
ordinaire — non corrigé, consigné.

## 8. Les instruments

| Fichier | Ce qu'il fait |
| --- | --- |
| `instrument/sonde-latence.js` | `requestVideoFrameCallback`, injectée par CDP. Compte les trames **et** celles sans `captureTime` : un `[]` a deux causes, ce compteur les sépare |
| `instrument/pilote-latence.mjs` | établit la session, pose l'amorce **au niveau navigateur** (`Target.setAutoAttach` + `waitForDebuggerOnStart`, l'idiome du lot 32C), anime, relève, écrit le JSON |
| `instrument/sr-rtcp.sh` | compte les sender reports RTCP **sur le fil**, par type de paquet et par sens |

🔴 **Deux détours à connaître, tous deux mesurés ici :**
① recharger une page de session la fige (`Runtime.evaluate` sans réponse après
20 s) — le rôle `client` est **exclusif** et la page rechargée demande un rôle
que la précédente n'a pas rendu ; d'où l'amorce au niveau navigateur, qui ne
recharge et ne ferme rien.
② un accent grave dans un commentaire **à l'intérieur d'un littéral de
gabarit** ferme le littéral : `node --check` rend « missing ) after argument
list » et désigne une autre ligne. Le piège des accents graves du dépôt, payé
dans cet instrument même.
