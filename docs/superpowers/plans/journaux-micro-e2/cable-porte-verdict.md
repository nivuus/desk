# Sonde éliminatoire de E2 — **le câble porte-t-il, et entre quelles sessions ?**

**Tâche 1 du plan `2026-08-20-micro-e2.md`. Jouée le 20 août 2026.**
Binaire de produit : **aucun** — cette sonde ne fait tourner ni l'agent ni une
seule ligne de code de E2. Elle n'emploie que des scripts PowerShell.

---

## ✅ VERDICT : LE CÂBLE PORTE. La sonde est REÇUE, et sur les quatre combinaisons.

**Les quatre passent, une exécution chacune.** Le couple qui décide — celui du
produit, **session 1 → session 1** — passe, et les trois témoins aussi.

| # | Rendu (`abis-jouer.ps1`, `waveOut`) | Capture (`micro-ecoute-e2.ps1`, WASAPI) | 🔴 Témoin : crête sur CABLE **Input** | Juge : fréquence sur CABLE **Output** | Verdict |
| --- | --- | --- | --- | --- | --- |
| **A** | **session 1** (tâche `/it`) | **session 1** (tâche `/it`) | **0,449982** | **660,0 Hz**, ampl. 0,449630, crête 0,449795 | ✅ **PASSE** |
| B | session 1 (tâche `/it`) | session 0 (WinRM) | 0,449982 | **660,0 Hz**, ampl. 0,449630, crête 0,449795 | ✅ PASSE |
| C | session 0 (WinRM) | session 1 (tâche `/it`) | 0,449982 | **660,0 Hz**, ampl. 0,449624, crête 0,449795 | ✅ PASSE |
| D | session 0 (WinRM) | session 0 (WinRM) | 0,449982 | **660,0 Hz**, ampl. 0,449681, crête 0,449795 | ✅ PASSE |

Tonalité émise : **660 Hz**, amplitude 0,45, stéréo 16 bits à 48 000 Hz, pendant
26 s. Fréquence retrouvée : **660,0 Hz** aux quatre, à la résolution du balayage
(1 Hz). Journaux : `micro-e2-ecoute-{A,B,C,D}.log` et
`micro-e2-jouer-{A,B,C,D}.log`.

**Ce que A établit, et c'est tout ce que le chantier attendait** : ce qui est
écrit sur le point de terminaison de **rendu** du câble (`Haut-parleurs
(VB-Audio Virtual Cable)`, `{0.0.0.00000000}.{deec1914-…}`) ressort sur le point
de terminaison de **capture** de l'autre bout (`CABLE Output (VB-Audio Virtual
Cable)`, `{0.0.1.00000000}.{5fae72b2-f885-4038-9827-0d91f862a7d5}`) —
**deux endpoints distincts, deux GUID distincts, deux sens distincts**. C'est
exactement l'inconnue que le plan déclarait intacte : la phase T d'A-bis avait
retrouvé 660 Hz **par un loopback sur CABLE Input**, donc au même bout du tuyau.

---

## 🔴 Le témoin positif, et pourquoi il n'est pas décoratif

Un verdict négatif n'aurait été fondé que si la crête sur CABLE **Input** avait
été **non nulle** pendant la mesure : trois zéros ne disent rien du câble,
seulement de l'instrument. Le témoin est `abis-metre.ps1`, appelé par
`abis-jouer.ps1` **dans la même exécution**, et il lit la crête de **chaque**
point de terminaison de rendu :

```
METRE crete=0,000000  Haut-parleurs (Steam Streaming Speakers)
METRE crete=0,000000  HDP-V104 (NVIDIA High Definition Audio)
METRE crete=0,449982  Haut-parleurs (VB-Audio Virtual Cable)
```

Il est **discriminant** : il ne rend pas 0,45 partout, il le rend sur le câble et
sur lui seul. Aux quatre combinaisons, à l'identique.

---

## 🔴 L'instrument PEUT rendre l'autre valeur — et il ment sur la crête

**Deux contrôles, joués, et le second est une trouvaille.**

**① Silence vrai** (`micro-e2-ecoute-silence.log`, 8 s, plus de 30 s après tout
son, et **après** qu'une capture antérieure a déjà couru) :

```
PAQUETS=798 TRAMES=351918 TRAMES_SILENCE=0
CRETE=0.000000
FREQUENCE=80.0 AMPLITUDE=0.000000
```

Le juge rend **zéro** quand rien ne joue. **Le contrôle est atteignable dans les
deux sens** — sans quoi « le câble porte » aurait été une tautologie.
⚠️ `FREQUENCE=80.0` est la borne basse du balayage : sur du silence, toutes les
puissances valent 0 et c'est le premier pas qui gagne. **C'est `AMPLITUDE` qui
tranche, jamais `FREQUENCE` seule.**

**② Résidu — la crête seule aurait menti** (`micro-e2-ecoute-residu.log`).
Observé d'abord par accident, puis **reproduit délibérément** : on joue 26 s,
on laisse **45 s** s'écouler — le son est fini depuis ~19 s et le flux `waveOut`
fermé depuis ~10 s —, puis on ouvre une capture neuve :

```
PAQUETS=798 TRAMES=351918 TRAMES_SILENCE=0
CRETE=0.449795
FREQUENCE=80.0 AMPLITUDE=0.000000
```

**Une crête de 0,449795 — la crête exacte de la tonalité — alors que plus rien ne
joue, et une amplitude nulle à toutes les fréquences.** Le pilote VB-Cable
remet à un client de capture neuf le contenu resté dans son tampon. Il ne le
remet **qu'une fois** : c'est pourquoi le contrôle ① rend 0, une capture
antérieure l'ayant déjà drainé.

⚠️ **C'est la doctrine D7 confirmée par la mesure, sur ce matériel exact : sur ce
câble, une crête seule aurait dit « ça porte » sans que rien ne porte.** Toute
recette de E2 juge à la **fréquence dominante**, jamais à la crête.

---

## Le format, relevé par le juge lui-même

Aux **six** exécutions du juge (A, B, C, D, silence, résidu), sans exception :

```
FORMAT tag=EXTENSIBLE HZ=44100 canaux=2 bits=32 sous-format=00000003-0000-0010-8000-00aa00389b71
```

**CABLE Output est à 44 100 Hz**, quand CABLE Input est à 48 000 — l'asymétrie
que la tâche 2 traite. Le juge calcule donc sa fréquence sur **44 100**, et non
sur 48 000 : un juge qui aurait supposé 48 000 aurait rendu **719 Hz** pour la
même tonalité, faux de 8,8 %, sans que rien ne le dise.

**Et le critère survit à l'asymétrie, c'est mesuré ici et pas seulement
raisonné** : la tonalité est émise à 48 000 Hz, rééchantillonnée par le pilote,
et retrouvée à **660,0 Hz** — un rééchantillonnage préserve la fréquence d'un ton
pur.

---

## Ce que cette sonde N'ÉTABLIT PAS

- **Une exécution par combinaison. Aucun taux.**
- 🔴 **Le rendu est fait par `waveOut` (winmm), pas par WASAPI.** E2 écrira par
  `IAudioRenderClient` en mode partagé. Les deux passent par le même moteur audio
  de Windows, donc c'est un **substitut raisonnable — ce n'est pas le même
  appel**. Que `IAudioClient::Initialize` en mode partagé réussisse sur ce
  périphérique n'est **pas** établi ici : c'est la tâche 7, et elle n'a pas
  d'antécédent dans ce dépôt (aucun code n'a jamais ouvert un flux de **rendu**
  WASAPI).
- **Rien de la latence** du câble, rien de sa **qualité**, rien de la dégradation
  imputable au rééchantillonnage 48 000 → 44 100.
- **Rien à deux écrivains** : un seul flux de rendu a jamais été ouvert. C'est
  précisément ce que la Décision 2 (mutex nommé) existe pour empêcher, et cette
  sonde ne l'exerce pas.
- **Rien du produit** : ni l'agent, ni `LecteurMicro`, ni un `getUserMedia`
  réel. Une seule fréquence (660 Hz), une seule amplitude (0,45), une seule
  durée (26 s).
- **Rien de la boucle locale** (Décision 3) : la sonde ne fait tourner aucun
  loopback de capture.

## Deux observations de méthode, versées parce qu'elles ont coûté

1. 🔴 **`nodejs-winrm` rend la main dès la PREMIÈRE ligne reçue et le processus
   distant est TUÉ.** La première version de l'orchestrateur de la combinaison A
   journalisait le retour de `schtasks /run` ; le second `/run` n'a **jamais**
   eu lieu, et `schtasks /query /v` le dit noir sur blanc — *Dernier résultat
   **267011*** (`0x41303`, `SCHED_S_TASK_HAS_NOT_RUN`), *Heure de la dernière
   exécution **30/11/1999***. **Un orchestrateur invoqué par WinRM ne doit rien
   écrire sur sa sortie standard avant d'avoir fini** (`*> $null` partout, un
   seul `Write-Output` final). Sans le `/query`, l'absence de journal se serait
   lue comme « la capture en session 1 ne marche pas ».
2. **Le juge écrit son propre journal par `StreamWriter`**, et ne dépend donc
   pas de ce que WinRM rapporte. C'est ce qui a permis de diagnostiquer le
   point 1 : la sortie était tronquée, le fichier ne l'était pas.

## Conséquence pour la suite du bloc

**La forme de E2 ne change pas.** Le chantier est livrable : le câble porte, le
couple du produit passe, et le juge (`micro-ecoute-e2.ps1`) est éprouvé dans les
deux sens. Deux acquis pour le **protocole** des recettes 12 et 13 :

- **le juge peut tourner depuis WinRM** (combinaison B) — pas besoin d'une tâche
  planifiée pour lui, ce qui simplifie le pilote ;
- **le câble relaie d'une session à l'autre, dans les deux sens** (B et C).
  ⚠️ Cela **ne réfute pas** le relevé de E1 « un rendu lancé depuis WinRM
  n'atteint aucun endpoint de la session 1 » : celui-là portait sur un
  **loopback de capture** du point de terminaison de rendu, chemin que cette
  sonde n'emprunte pas. Les deux relevés coexistent et ne mesurent pas la même
  chose.
