# Tâche 13 — dix minutes sans dérive, et l'écho à deux fenêtres

**Jouée le 20 août 2026**, binaire vert **10 160 640 octets**.
**DEUX exécutions de dix minutes. Aucun taux n'est revendiqué.**

---

## Step 1 — les dix minutes : **aucune grandeur ne croît**

Configuration : mono-fenêtre, `MICRO=1`, `AUDIO_PERIPHERIQUE='Steam Streaming'`,
source = le WAV de 440 Hz joué par Chrome à la place d'un microphone, micro
allumé par un clic sur `#micro` et laissé allumé.

### Côté agent — une ligne par minute, les deux passages

Pièces : `longue-1-par-minute.txt`, `longue-2-par-minute.txt` (extraites des
journaux plats versés, une ligne sur soixante).

| | passage 1 (20:39→20:49) | passage 2 (21:00→21:09) |
| --- | --- | --- |
| lignes de trace | **615** | **630** |
| `retards` **cumulés** | **0** | **0** |
| `insertions` cumulées | **0** | **0** |
| `sauts` cumulés | **3** | **3** |
| `plc` cumulés | **18** | **10** |
| `deposees` par minute | **50** aux neuf minutes | **50** aux neuf minutes (51 à la dernière) |
| `silence` par minute | 5 à 10 trames **sur 48 000** | 5 à 9 **sur 48 000** |
| `plc_plafonnees` pendant le micro | **0** | **0** |

**Le critère est « qu'aucune de ces grandeurs ne croisse monotonement ». Aucune
ne croît du tout** : elles sont plates, et les trois `sauts` de chaque passage
tombent à l'amorçage, dans la première seconde après le clic.

⚠️ **DEUX GRANDEURS DEMANDÉES PAR LE PLAN N'EXISTENT PAS DANS CETTE TRACE**, et
c'est une lacune d'instrument que la recette révèle : le step 1 demande
« l'occupation du tampon, `sauts`, `insertions`, **famines** et `retards` ». La
trace `micro ecrit sur le cable` (`windows_micro.rs::tracer`) porte `ecrites`,
`silence`, `retards`, `deposees`, `sauts`, `insertions`, `plc` et
`plc_plafonnees` — **ni `famines`, ni `occupation_ms`**. Ces deux-là ne vivent
que dans la trace du puits de MESURE de E1 (`demarrage/micro/mesure.rs`), qui
est un instrument de banc et **ne peut pas coexister** avec le câble
(Décision 10). **Elles ne sont donc pas observables sur le chemin de
production, et ce n'est pas mesuré ici.**

### Côté navigateur — le passage 2, onze relevés

Pièce : `pilote-longue-2.json`. **Croissance exactement linéaire**, 3038 paquets
montants par minute (50,6/s) :

| Relevé | ICE | paquets montants | `packetsLost` | RTT |
| --- | --- | --- | --- | --- |
| après `ready` | `connected` | 0 | 0 | 2 ms |
| t+60 s | `connected` | 3 047 | 0 | 2 ms |
| t+300 s | `connected` | 15 200 | 0 | 2 ms |
| t+600 s | `connected` | **30 369** | **0** | 1 ms |

🔵 **C'est aussi la mesure de l'EFFET des trois drapeaux de durée**, que la
tâche 11 ne pouvait pas faire : **la page n'a pas gelé**. Deux sessions de
11 minutes du chantier TURN s'étaient interrompues à 331 s et 340 s pour cette
seule raison ; ici, ICE reste `connected` et l'émission reste linéaire à
600 s — bien au-delà du seuil de 5 minutes.

### Le câble porte encore au bout de neuf minutes — passage 1

Le juge, trois fois, sur `CABLE Output` :

| Instant | Journal | Fréquence / amplitude |
| --- | --- | --- |
| t+120 s | `e2-juge-longue-t120.log` | **440,0 Hz** / 0,056246 |
| t+300 s | `e2-juge-longue-t300.log` | **440,0 Hz** / 0,054479 |
| t+540 s | `e2-juge-longue-t540.log` | **440,0 Hz** / 0,046255 |

⚠️ **L'amplitude décroît de 0,0562 à 0,0463 sur sept minutes (−17,6 %).** Trois
points, une exécution : **c'est un relevé, pas une tendance établie**, et ce
n'est PAS une des grandeurs du critère. Rien dans le journal de l'agent ne
bouge en regard — `deposees` reste à 50, `silence` à moins de 10 sur 48 000. La
cause n'est pas identifiée ; le gain automatique de Chrome (`autoGainControl`,
armé par `client/src/micro.ts`) en est un suspect **non éprouvé**.

### Deux observations qui ne sont pas des critères

- **50 `WARN` « échec de réception UDP transitoire, ignoré »** au passage 1
  (`os error 10054`), étalés sur toute l'exécution, tous avec
  `consecutives=1`. C'est le chemin de tolérance prévu, et il ne s'accumule pas.
- **UN seul « paquet micro dont la durée Opus est illisible, paquets
  abandonnés »**, à 20:39:15, soit **4,6 s après le premier dépôt** — donc à
  l'amorçage. Une occurrence en dix minutes, et aucune ensuite.

⚠️ **Le compte rendu du navigateur du passage 1 est PERDU** : l'appel qui le
portait a été interrompu au bout de dix minutes exactement, et le pilote est
mort avec son shell avant d'écrire son JSON. Les trois lectures du juge et le
journal d'agent, eux, sont intacts. **Le passage 2 a été rejoué en arrière-plan
pour cette seule raison**, et c'est lui qui porte le relevé navigateur.

## Step 2 — l'écho à deux fenêtres : **NON MESURABLE PAR CE MONTAGE**

**Déclaré tel, et ce n'est pas un échec : c'est un résultat.**

L'annulation d'écho de Chrome (`echoCancellation: true`, posée par
`client/src/micro.ts`) n'annule que ce que **son propre onglet** restitue. Or
depuis D7 chaque fenêtre porte le son de **sa** propre application : à deux
fenêtres, la fenêtre A entend par le haut-parleur le son que restitue la
fenêtre B, dont son AEC ne sait rien, et ce son repart dans le microphone.

**Ce montage ne peut pas trancher**, et la raison est structurelle : Chrome
tourne **sans interface**, sur un hôte **sans haut-parleur ni microphone
réels**. Le microphone est un fichier, la restitution ne va nulle part —
**il n'existe aucun chemin acoustique**. Aucune mesure faite ici, quelle qu'elle
soit, ne dirait quoi que ce soit de l'écho.

**Le déclarer non mesurable est un résultat ; le déclarer tenu serait un
mensonge.**

**Ce qu'un humain doit jouer pour le trancher**, décrit assez précisément pour
être rejoué :

1. deux fenêtres du produit ouvertes côte à côte, chacune sur une application
   Windows qui **émet du son** (deux tons distincts rendent le verdict lisible) ;
2. micro allumé dans **une** des deux — celle qui reste seule à écrire, la
   seconde étant refusée par le mutex (critère ⑤ de la tâche 12) ;
3. **sans casque** : parler, et écouter sur `CABLE Output` si l'on entend le son
   de la fenêtre voisine revenir ;
4. **avec casque** : refaire, l'écho acoustique étant alors supprimé par
   construction. L'écart entre les deux est la mesure.

⚠️ **Si un humain le joue, c'est E3 qui en hérite.** Aucun élément de ce bloc
n'y prépare autre chose que la description ci-dessus.

## Step 3 — survie de la VM, contrôlée APRÈS la séquence

`virsh list --all` → **« en cours d'exécution »**. Relevé sur la VM,
23:10:32 locale : `UPTIME=1.11:23:27` — **continu**, donc **aucune
hibernation** pendant la recette. Quatre points de terminaison audio toujours
`OK`, `Audiosrv` toujours `Running`, **aucun agent** et **aucun Chrome**
résiduels.

Le journal de libvirt, consulté **après** la séquence et jamais avant, porte ses
deux dernières extinctions à **10:29:09 et 11:09:29 UTC** — soit **plus de neuf
heures avant** la première mesure de cette recette (20:12 UTC). **Aucune ne
tombe dans la fenêtre de mesure.**

---

## Ce que la tâche 13 n'établit PAS

- **Deux exécutions. Aucun taux.**
- **Ce n'est PAS une mesure de latence de bout en bout** — que rien ne mesure
  dans ce dépôt depuis D1, et que E2 ne mesure pas davantage.
- **`famines` et l'occupation du tampon ne sont pas observables** sur le chemin
  de production (voir plus haut) : deux des cinq grandeurs que le step 1
  demandait manquent, et c'est l'instrument qui manque, pas la mesure.
- **La décroissance de 17,6 % de l'amplitude sur sept minutes n'est ni
  expliquée ni confirmée** : trois points, une exécution.
- **L'écho à deux fenêtres n'est pas mesuré**, et ne peut pas l'être ici.
- **Une seule fréquence, une seule application, une seule fenêtre**, et un
  navigateur **sans interface à décodage logiciel** — pas un client réel.
