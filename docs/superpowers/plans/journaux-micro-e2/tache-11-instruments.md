# Tâche 11 — les deux instruments, éprouvés AVANT de servir

**Jouée le 20 août 2026.** Aucun fichier de code de produit n'est touché.
**Une exécution par relevé, sauf le contrôle « le juge sait rendre NON », qui en
a deux. Aucun taux n'est revendiqué.**

⚠️ **État de départ, relevé et non supposé** : la VM a été **reprise
d'hibernation**, pas démarrée à froid — `UPTIME=1.10:18:17` et un Bloc-notes
antérieur toujours vivant (`e2-etat.ps1`). Les quatre points de terminaison
audio sont `OK`, dont les deux bouts du câble, et `Audiosrv` tourne.

---

## ① La source : un VRAI `getUserMedia`, et le ton SURVIT au traitement de Chrome

**C'est la mesure que la Décision 6 déclarait « non vérifiée à ce jour ».** Le
pilote ouvre un périphérique factice alimenté par un WAV
(`--use-file-for-fake-device-…`), avec **les trois contraintes exactes de
`client/src/micro.ts`**, et mesure la fréquence dominante de la piste obtenue.

| Exécution | Piste | État | Étiquette | Dominante | Détachement / médiane | Crête temporelle |
| --- | --- | --- | --- | --- | --- | --- |
| `instrument-ton-440.json` | **oui** | `live` | `Fake Default Audio Input` | **439,45 Hz** | **+106,9 dB** | **0,398237** |
| `instrument-silence.json` | oui | `live` | `Fake Default Audio Input` | 0,00 Hz | `-Infinity` | **0,000000** |

Résolution du relevé : **5,859 Hz par bac** (FFT 8192 à 48 000 Hz), écrite par
l'instrument lui-même — E1 laissait la sienne à inférer, et son document de
résultats le compte comme une lacune. **439,45 Hz est le centre du bac qui
contient 440 Hz** : l'écart est celui de la résolution, pas du signal.

🔴 **Ce second étage n'est pas décoratif, et il n'était pas prévisible.**
`noiseSuppression: true` arme NS3, dont l'objet est d'effacer le bruit
**stationnaire** — et une sinusoïde continue en est un du point de vue de
l'algorithme. Un contrôle qui se serait arrêté à « `getUserMedia` rend une
piste » aurait passé sur un périphérique dont le contenu est intégralement
effacé, et **toute la recette aurait mesuré le silence en croyant mesurer une
voix**. La mesure tranche : le ton **continu** passe, et le WAV haché
(`ton-440-hache.wav`), écrit d'avance comme repli contre ce risque, **n'a pas
eu à servir**.

**Le repli de E1 — l'oscillateur — n'a PAS été employé.** Le `getUserMedia`
réel est exercé, et le document de résultats peut le revendiquer.

⚠️ **Deux corrections d'instrument, faites parce que la première rédaction
mentait sans se tromper** : `etatPiste` était relevé APRÈS `stop()` et rendait
donc `ended` sur une piste saine — il décrivait le geste de l'instrument, pas
l'état mesuré ; et `-Infinity` se sérialisait en `null`, qui se lit « pas
mesuré » alors qu'il veut dire « rigoureusement aucune énergie ». Les deux
relevés ci-dessus sont **postérieurs** aux corrections.

## ② Les drapeaux de durée (step 2)

Les trois drapeaux sont dans le pilote, nommés avec leur raison :
`--disable-background-timer-throttling`,
`--disable-backgrounding-occluded-windows`, `--disable-renderer-backgrounding`.
Une page jamais mise au premier plan gèle au bout de 5 minutes ; deux sessions
de 11 minutes du chantier TURN se sont interrompues à 331 s et 340 s pour cette
seule raison, et la cause avait d'abord été imputée au relais. **La tâche 13
dure dix minutes : elle y est exposée par construction.**

⚠️ **Leur PRÉSENCE est vérifiée par lecture, leur EFFET par la tâche 13** — une
exécution de 8 s ne peut rien en dire.

## ③ Le juge sait rendre NON — deux exécutions

`micro-ecoute-e2.ps1` sur `CABLE Output`, **rien ne jouant** :

| Journal | Format obtenu | Crête | Fréquence / amplitude |
| --- | --- | --- | --- |
| `e2-juge-t11-non-1.log` | 44 100 Hz, 2 canaux, 32 bits flottant | **0,000000** | 80,0 Hz / **0,000000** |
| `e2-juge-t11-non-2.log` | idem | **0,000000** | 80,0 Hz / **0,000000** |

⚠️ `FREQUENCE=80.0` est la **borne basse du balayage** : sur du silence toutes
les puissances valent 0 et c'est le premier pas qui gagne. **C'est `AMPLITUDE`
qui tranche, jamais `FREQUENCE` seule.**

⚠️ **Aucun résidu de tampon n'est apparu cette fois**, contrairement au relevé
de la tâche 1 (une crête de 0,449795 sur un spectre entièrement nul, 45 s après
la fin du son). Rien n'avait joué depuis la reprise d'hibernation : le tampon de
VB-Cable était vide. **Cela ne réfute pas le piège — cela confirme sa
condition.**

## ④ Et le juge sait rendre OUI, sur CET état de VM — une exécution

Un NON n'a de valeur que si le OUI est établi. Le témoin positif de la tâche 1
est **rejoué maintenant** (`e2-temoin-cable.ps1`), 660 Hz par `waveOut` sur le
rendu du câble en session 1, écoutés sur `CABLE Output` en session 1 :

```
METRE crete=0,000000  Haut-parleurs (Steam Streaming Speakers)
METRE crete=0,000000  HDP-V104 (NVIDIA High Definition Audio)
METRE crete=0,449982  Haut-parleurs (VB-Audio Virtual Cable)     <- discriminant
FREQUENCE=660.0 AMPLITUDE=0.449759   (e2-temoin-ecoute.log)
```

**Le câble porte toujours, après hibernation et reprise.** C'est ce qui empêche
un faux verdict éliminatoire à la tâche 12 : si l'agent n'était pas entendu, on
saurait que ce n'est ni le câble ni le juge.

## ⑤ Ce que la tâche 11 n'établit PAS

- **Rien du produit.** Aucun agent n'a tourné, aucune session WebRTC n'a été
  établie, le bouton `#micro` n'a pas été cliqué. Le contrôle d'instrument
  n'exerce que Chrome.
- **L'effet des trois drapeaux de durée n'est pas mesuré**, seulement leur
  présence.
- **Une seule fréquence de source (440 Hz), une seule amplitude (0,5), une
  seule durée (8 s)** au contrôle d'instrument.
- **Le rendu du témoin passe par `waveOut` (winmm), pas par WASAPI** — réserve
  héritée de la tâche 1, et l'agent, lui, écrira par `IAudioRenderClient`.
- **Rien de la latence**, ni de la qualité, ni de la dégradation imputable au
  rééchantillonnage 48 000 → 44 100 que `CABLE Output` impose toujours (tâche 2 :
  le remède est **inapplicable**, il casse le point de terminaison).

## Les fichiers, et pourquoi les WAV ne sont pas versés

`faire-wav-e2.mjs` est **déterministe** — aucune source d'aléa. Les quatre WAV
pèsent 4,6 Mo ; leurs empreintes sont versées (`wav-empreintes.txt`) et la
reproduction bit pour bit a été **vérifiée dans le tour même**, pas supposée :
régénération dans un autre répertoire, `sha256sum` identiques aux quatre.
