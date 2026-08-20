# Tâche 12 — recette d'écoute sur la VM : **une application entend**

**Jouée le 20 août 2026.** Binaire vert : **10 160 640 octets**, bâti sur `main`
à `3b30e31` (`cargo clean --release -p proto -p agent` avant compilation, taille
relevée après). Binaire rouge : **10 088 448 octets**, bâti depuis un arbre de
travail détaché sur **`08dabab`**, le dernier commit avant toute ligne de code
de produit de E2 — vérifié par l'absence de `windows_micro.rs`,
`micro/exclusivite.rs`, `micro/boucle_locale.rs` et `wasapi/ecriture.rs` dans cet
arbre.

**Aucun taux n'est revendiqué nulle part.** Le nombre d'exécutions est dans
chaque énoncé.

**Configuration commune** : mode **mono-fenêtre**, fenêtre capturée = un
Bloc-notes, `MICRO=1`, et — pour toutes les vertes —
`AUDIO_PERIPHERIQUE='Steam Streaming'`. 🔴 **Ce dernier réglage n'est pas un
détail de confort : sans lui la garde de boucle locale coupe le micro**, et
c'est le critère ③ qui le montre.

---

## Le tableau des verdicts

| # | Critère | Verdict | Exéc. |
| --- | --- | --- | --- |
| ① | Une application Windows entend le bon signal | **TENU** — 440,0 Hz sur CABLE Output | **2** vertes + **1** rouge |
| ② | `ready` porte `mic: true`, et le bouton paraît | **TENU** | **2** vertes + **3** rouges de natures différentes |
| ③ | 🔴 La garde de boucle locale mord, et nomme son remède | **TENU** | **2** côté agent, dont **1** avec session établie |
| ④ | Le silence ne fait pas de bourdon | **TENU** | **2** |
| ⑤ | L'exclusivité : un seul écrivain | **PARTIELLEMENT EXERCÉ** — voir la réserve | **1** |

---

## ① Une application Windows entend le bon signal

Le juge est `micro-ecoute-e2.ps1` : il ouvre `CABLE Output` par `IAudioClient`
en mode partagé, **c'est-à-dire exactement ce que fait une application Windows
qui choisit ce microphone**. Il tourne en session 0 (via WinRM), ce que la
tâche 1 a établi licite (sa combinaison B).

| Exécution | Journal | Crête | **Fréquence / amplitude** |
| --- | --- | --- | --- |
| V1 | `e2-juge-v1.log` | 0,449513 | **440,0 Hz** / 0,050500 |
| V2, passage 1 | `e2-juge-v2a.log` | 0,058250 | **440,0 Hz** / 0,054905 |
| V2, passage 2 | `e2-juge-v2b.log` | 0,058216 | **440,0 Hz** / 0,055365 |

La source est un WAV de 440 Hz joué par Chrome à la place d'un microphone, à
travers un **vrai `getUserMedia`** — permission, contraintes et pipeline audio
compris (tâche 11).

🔴 **LE ROUGE, ET IL PORTE LE MÉCANISME.** Sur le binaire d'**avant E2**, même
configuration, même juge :

| Pièce | Relevé |
| --- | --- |
| `e2-juge-rouge.log` | crête 0,056046 — **`AMPLITUDE=0,000000`**, aucune dominante |
| `pilote-rouge.json` | `ice=connected`, deux flux entrants — **la session VIT, `ready` est bien arrivé** — et `boutonParait=false` |
| `agent-rouge-avant-e2-plat.log` | **0** ligne `windows_micro` : le module n'existe pas dans ce binaire |
| `e2-temoin-ecoute-rouge.log` | **MÊME EXÉCUTION** : le témoin `waveOut` joue 660 Hz sur le câble, et le juge rend **660,0 Hz / 0,449606** |

**Sans cette dernière ligne, « E2 n'écrit pas » et « le juge est cassé » se
liraient pareil.** C'est le rouge vacueux que D10 a produit et que ce plan
interdit nommément ; il est évité en faisant porter au rouge le **mécanisme
observé** — le juge — identique des deux côtés.

## ② `ready` porte `mic: true`, et le bouton paraît

`client/src/micro.ts` ne montre le bouton que si l'agent annonce `mic: true`
(`annoncerDisponibilite`). Le pilote lit `#micro` dans le DOM, **il ne devine
rien** :

| Exécution | `hidden` | après clic |
| --- | --- | --- |
| V1, V2 (vertes) | `false` | `dataset.etat = "actif"`, titre « Microphone actif — cliquez pour couper » |
| rouge, binaire d'avant E2 | **`true`** | — |
| rouge, garde de boucle armée | **`true`** | — |
| rouge, `MICRO=0` | **`true`** | — |

⚠️ **Les trois rouges ont une session VIVANTE** (`ice=connected` aux trois) : le
bouton est caché parce que `ready` portait `mic: false`, **pas** parce que
`ready` n'est jamais venu. Sans ce contrôle, le critère aurait été vacueux.

✅ **Le rouge que le plan nommait — `MICRO=0` — a été joué, en TROISIÈME**, et
c'est le seul des trois qui exerce le prédicat `arme_micro` de bout en bout sur
la VM :

```
INFO agent::demarrage::micro: micro DESARME (MICRO=0) session=…:e2-micro0
```

`agent-micro0-plat.log` : **0** ligne `windows_micro`. `pilote-micro0.json` :
`boutonParait=false` sur une session **`ice=connected`** portant deux flux
entrants. `e2-juge-micro0.log` : **`AMPLITUDE=0,000000`**.

⚠️ *Une première rédaction de ce document déclarait ce rouge « non joué, non
remplacé » ; il l'a été une demi-heure plus tard, et la phrase est corrigée ICI
plutôt que laissée à trouver — c'est la classe de défaut exacte que la revue
transverse de fin de branche cherche.*

## ③ 🔴 La garde de boucle locale mord — et c'est le chemin NOMINAL de cette VM

Exécution **sans** `AUDIO_PERIPHERIQUE`. Le rendu par défaut de cette VM **est
le câble** : le loopback du chantier A capterait donc le point de terminaison
même sur lequel le micro écrit.

```
WARN agent::demarrage::micro: micro indisponible, la session continue sans
  session=…:e2-boucle-2
  erreur=micro DESACTIVE : le loopback audio de cette session capte le cable meme
  sur lequel le micro ecrirait ({0.0.0.00000000}.{deec1914-…}) — l'utilisateur
  s'entendrait lui-meme. Remede : posez AUDIO_PERIPHERIQUE sur un AUTRE rendu.
  Disponibles : «Haut-parleurs (Steam Streaming Speakers)» […], «HDP-V104 (NVIDIA
  High Definition Audio)» […], «Haut-parleurs (VB-Audio Virtual Cable)» […]
```

**Les deux identifiants comparés sont le même `{deec1914-…}`** : la règle pure
de `micro/boucle_locale.rs` rend `Risque`, et le micro cède — jamais le son.
Les quatre moitiés du critère sont relevées :

| Ce qui est vérifié | Relevé |
| --- | --- |
| le `warn!` nomme le remède **et** l'inventaire | ci-dessus, avec les trois identifiants |
| `ready` porte `mic: false` | `pilote-boucle-2.json` : `boutonParait=false`, `ice=connected` |
| rien n'est écrit sur le câble | `e2-juge-boucle-2.log` : **`CRETE=0,000000`**, `AMPLITUDE=0,000000` |
| le fil de rendu n'a jamais démarré | **0** ligne `micro ecrit sur le cable` |

⚠️ **Une première exécution (`e2-boucle-1`) a rendu la moitié agent du critère
mais sa session WebRTC n'a pas abouti** ; elle est versée et **comptée pour ce
qu'elle vaut**, pas pour le critère entier.

🔵 **C'est ce critère qui donne son sens à la configuration de toutes les
autres** : `AUDIO_PERIPHERIQUE='Steam Streaming'` n'est pas un contournement,
c'est le remède que la garde nomme elle-même.

## ④ Le silence ne fait pas de bourdon

| Exécution | Situation | Journal | Crête | Amplitude |
| --- | --- | --- | --- | --- |
| silence-1 | micro allumé, WAV silencieux | `e2-juge-silence-1.log` | 0,054249 | **0,000000** |
| silence-2, passage 1 | idem | `e2-juge-silence-2a.log` | **0,000031** | **0,000000** |
| silence-2, passage 2 | idem | `e2-juge-silence-2b.log` | **0,000031** | **0,000000** |
| silence-2, passage 3 | **piste ARRÊTÉE**, `plc_plafonnees` court à 100/s | `e2-juge-silence-2c.log` | **0,000031** | **0,000000** |

Le troisième passage est le plus important : c'est le cas où E1 relevait une
crête de **0,53 à 0,67** et une fréquence errante avant le plafond de
dissimulation. Ici le plafond mord (`plc_plafonnees=100` puis `101` à chaque
seconde), `silence` vaut 100 % des trames écrites, et **le câble reste à
3,1 × 10⁻⁵**.

⚠️ **DÉCOUVERTE QUI CONTREDIT LE LIBELLÉ DU CRITÈRE, et il faut le dire :
Chrome n'engage PAS le DTX sur du silence numérique.** Le critère annonçait
« 60 s de DTX : `plc_plafonnees` **court**, `plc` **est nul** ». Le relevé
donne `deposees=50` par seconde **sans interruption**, donc `plc=0` **ET**
`plc_plafonnees=0` : il n'y a aucune trame manquante à dissimuler. La moitié
« `plc` est nul » est tenue ; la moitié « `plc_plafonnees` court » **ne l'est
pas, et ne pouvait pas l'être** dans ce montage. Le plafond a bien été observé
courir — mais sur une piste **arrêtée**, pas sur un silence.

## ⑤ L'exclusivité — exercée, mais PAS par deux enfants

⚠️ **CE QUE LE CRITÈRE DEMANDAIT N'A PAS ÉTÉ JOUÉ.** Il exigeait « deux
enfants, micro allumé dans les deux ». **Deux agents mono-fenêtre ne peuvent
pas coexister sur cette VM** : DXGI n'autorise qu'**une** duplication ouverte
par sortie, doctrine établie de ce dépôt, et le second agent mourrait avant
d'atteindre le micro. Deux enfants exigent le mode **superviseur** et ses
sorties virtuelles — hors du périmètre des tâches 11 à 13, et sur une VM dont
le registre reste pollué (legs n°4 de D9).

**Ce qui a été exercé à la place, et c'est le chemin de code exact du critère** :
un processus tiers acquiert `Global\guacamole-agent-micro-cable` — le mutex
nommé de `windows_micro/verrou.rs` — puis l'agent tente d'écrire.

| Temps | Fait | Pièce |
| --- | --- | --- |
| 22:55:20 | un tiers (pid 18792) **crée et acquiert** le mutex | `e2-tenir-mutex.log` : `cree=True`, `ACQUIS=True` |
| 20:56:09 | l'agent est **refusé**, **une seule** ligne | `agent-exclusivite-plat.log` : `micro : une autre fenetre tient deja le cable…` |
| pendant | le juge n'entend **rien** — le micro monte pourtant (6590 paquets) | `e2-juge-excl-refus.log` : `AMPLITUDE=0,000000` |
| ~20:56:54 | le tiers est **TUÉ BRUTALEMENT** (`Stop-Process -Force`) | `DETENTEUR TUE` |
| 20:56:56 | l'agent **acquiert après le refus** | `micro : cable acquis apres un refus — une autre fenetre l'a relache` |
| après | le juge entend **440,0 Hz** | `e2-juge-excl-reprise.log` : 440,0 / 0,054179 |

**Deux acquis que rien d'autre n'établissait** :

1. 🔵 **`WAIT_ABANDONED` survient réellement.** Le détenteur n'a jamais appelé
   `ReleaseMutex` — il a été tué. Le seul chemin par lequel l'agent pouvait
   obtenir le mutex ensuite est celui que `verrou.rs` déclare nominal :
   `issue == WAIT_ABANDONED`. C'était une **prédiction** de la Décision 2 ;
   c'est désormais un relevé.
2. 🔵 **Le refus n'est PAS collant, et la correction que E1 n'avait pas vue
   fonctionne.** La tentative est refaite à chaque dépôt : 46 s après le
   refus, sans redémarrage ni renégociation, la session redevient sonore. Et
   **le journal, lui, est unique** — une seule ligne de refus, une seule ligne
   d'acquisition tardive.

⚠️ **Ce que ce montage n'établit PAS** : que **deux enfants** s'arbitrent. Un
processus PowerShell n'est pas un enfant de l'agent ; il tient le même objet du
noyau, ce qui exerce `Exclusivite::arbitrer` et `MutexNomme::tenter`, et rien de
plus.

---

## Step 3 — ce qui se lit gratuitement, et n'est pas un critère

**Relevé aux CINQ exécutions vertes** (`e2-v1`, `e2-v2`, `e2-silence2`,
`e2-longue`, `e2-excl`), identique aux cinq, une ligne chacune :

```
micro : ecriture sur le cable ARMEE  format=48000 Hz, 2 canaux, 32 bits, flottant
                                     reveil="evenement"  espace_mutex="Global"
```

| Grandeur | Valeur relevée | Ce que cela tranche |
| --- | --- | --- |
| **espace du mutex** | **`Global`** aux 5 | Le repli `Local\` **n'a jamais été emprunté** : `SeCreateGlobalPrivilege` est disponible à l'utilisateur de cette VM. Le repli reste **du code jamais couru** |
| **réveil** | **`evenement`** aux 5 | `AUDCLNT_STREAMFLAGS_EVENTCALLBACK` est **accepté**. La spec §6 le prédisait sans l'avoir mesuré ; c'est le premier flux de **rendu** WASAPI jamais ouvert par ce dépôt. **Le repli sur échéance n'a jamais couru** |
| **format retenu** | 48 000 Hz, 2 canaux, 32 bits flottant | Côté **CABLE Input**. Le juge, lui, relève **44 100 Hz** sur CABLE Output à ses 19 exécutions : l'asymétrie de la tâche 2 est intacte, et le remède reste **inapplicable** |
| **`retards`** | **0** partout, y compris cumulés sur dix minutes | La question du `Mutex` que l'en-tête de `windows_micro.rs` posait est **tranchée par le chiffre** : le fil de rendu ne rate aucune échéance. Une file sans verrou serait du travail écrit sans besoin constaté |
| **`packetsLost`** | **0** aux six exécutions instrumentées | Relevé côté navigateur (`pilote-*.json`) |
| **désignation du câble** | `demande=VB-Audio integree=true … critere="nom partiel"` | La désignation **intégrée** de la Décision 4 est celle qui sert ; `Choix::Defaut` n'est jamais atteint |

## Step 4 — la part qui ne s'automatise pas : **NON JOUÉE**

L'enregistreur vocal Windows réglé sur CABLE Output (spec §11.1) **n'a pas été
ouvert**, et personne n'a **écouté**. C'est une limite d'agent, pas un oubli :
juger une écoute demande une oreille.

⚠️ **Conséquence, à écrire dans le document de résultats** : le critère de fin
de la spec §13 n'est atteint ici que par un **juge logiciel**. Ce juge ouvre le
point de terminaison exactement comme le ferait l'application, et il juge sur
la fréquence dominante — mais il ne dit rien de ce que cela **vaut à
l'oreille** : ni la qualité, ni la distorsion, ni la dégradation imputable au
rééchantillonnage 48 000 → 44 100 que le câble impose toujours.

---

## Ce que la tâche 12 n'établit PAS

- **Aucun taux.** Deux exécutions par critère au mieux, une pour ⑤.
- **⑤ n'exerce pas deux enfants** (voir sa réserve).
- **Aucune écoute humaine**, donc rien de la qualité ni de la latence
  de bout en bout — que rien ne mesure dans ce dépôt depuis D1.
- **Le repli `Local\` du mutex et le repli sur échéance du réveil sont du code
  JAMAIS COURU** : cette VM emprunte les deux chemins nominaux.
- **Une seule fréquence de source (440 Hz), un seul débit, une seule
  application, une seule fenêtre.**
- **`MICRO_FAUTE_ECRITURE` n'a pas été armée** : le chemin d'échec d'écriture
  WASAPI n'a jamais couru.
