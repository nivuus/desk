# Chantier E — Microphone, bloc E2 : **une application Windows entend**

**Recette jouée le 20 août 2026.** Plan :
`docs/superpowers/plans/2026-08-20-micro-e2.md`. Spécification :
`docs/superpowers/specs/2026-07-28-micro-design.md` (écrite le 28 juillet 2026,
**avant les chantiers A, B, C et D** — ne rien en recopier sans passer par les
tableaux de vieillissement du plan de E1 et de celui-ci).
Journaux : `docs/superpowers/plans/journaux-micro-e2/`.

**Aucun taux n'est revendiqué nulle part.** Le nombre d'exécutions est dans
chaque énoncé.

---

## Note de lecture des journaux — DEUX familles, et aucun piège d'encodage

**128 fichiers suivis par git**, relevés par la commande à la clôture :

| Famille | Compte | Ce qu'il faut faire |
| --- | --- | --- |
| les journaux d'agent **bruts** (`agent-*.log`) | **10** | **séquences ANSI de `tracing` PRÉSENTES** : `sed 's/\x1b\[[0-9;]*m//g'` — **ou lire le jumeau `-plat`, versé pour chacun des dix**, vérifié un par un |
| tout le reste — `*-plat.log`, journaux du juge, `*.json`, `*.ps1`, `*.mjs`, `*.md`, `*.txt` | **118** | rien |

✅ **AUCUN octet NUL dans aucun fichier** (balayage `tr -dc '\000'` sur les 128) :
contrairement à D10 et au presse-papier P1, aucun `grep -a` n'est nécessaire ici.
⚠️ **70 fichiers portent des CRLF** — ils viennent de la VM, et cela ne gêne
aucun `grep`.

---

## 1. Ce que E2 livre, et où il s'arrête

E1 avait porté la voix du navigateur jusqu'au **PCM décodé dans l'agent**. E2
écrit ce PCM sur le point de terminaison de rendu du câble virtuel, par un fil
WASAPI dédié, et **une application Windows qui choisit `CABLE Output` comme
microphone l'entend**.

🔴 **Ce qui n'est PAS livré, et qu'il ne faut pas déduire du succès** :

- **personne n'a écouté** (§7) ;
- **la latence ajoutée par l'agent n'est pas mesurée**, donc le troisième
  critère de la spec §13 **n'est pas jugé** (§8) ;
- **l'écho à deux fenêtres est déclaré NON MESURABLE par ce montage** (§6) ;
- **le rééchantillonnage 48 000 → 44 100 du câble subsiste**, et sa qualité
  n'est mesurée par personne (§9).

**Binaires mesurés** : vert **10 160 640** octets ; rouge (avant E2, `08dabab`)
**10 088 448** octets. Les deux bâtis après
`cargo clean --release -p proto -p agent`, taille relevée **après** compilation.

---

## 2. Le résultat qui gouverne : 440,0 Hz sur CABLE Output

Le juge (`micro-ecoute-e2.ps1`) ouvre `CABLE Output` par `IAudioClient` en mode
partagé — **exactement ce que fait une application Windows qui choisit ce
microphone**. La source est un WAV de 440 Hz joué par Chrome **à la place d'un
microphone**, à travers un `getUserMedia` réel.

| Exécution | Journal | Crête | **Fréquence / amplitude** |
| --- | --- | --- | --- |
| V1 | `e2-juge-v1.log` | 0,449513 | **440,0 Hz** / 0,050500 |
| V2, passage 1 | `e2-juge-v2a.log` | 0,058250 | **440,0 Hz** / 0,054905 |
| V2, passage 2 | `e2-juge-v2b.log` | 0,058216 | **440,0 Hz** / 0,055365 |

🔴 **LE ROUGE PORTE LE MÉCANISME, PAS LE RÉSULTAT** — c'est le rouge vacueux de
D10 évité. Sur le binaire d'**avant E2**, même configuration, même juge :

| Pièce | Relevé |
| --- | --- |
| `e2-juge-rouge.log` | **`AMPLITUDE=0,000000`**, aucune dominante |
| `pilote-rouge.json` | `ice=connected`, deux flux entrants — **la session VIT** — et `boutonParait=false` |
| `agent-rouge-avant-e2-plat.log` | **0** ligne `windows_micro` |
| `e2-temoin-ecoute-rouge.log` | **MÊME EXÉCUTION** : le témoin joue 660 Hz sur le câble et le juge rend **660,0 Hz / 0,449606** |

**Sans cette dernière ligne, « E2 n'écrit pas » et « le juge est cassé » se
liraient pareil.**

---

## 3. Les cinq points que l'abstention de l'implémenteur avait laissés ouverts

Le bloc d'implémentation (tâches 7 à 10) s'est **délibérément abstenu** de
toucher la VM. Il a nommé ce que cela laissait non vérifié. **Les cinq sont
mesurés.**

| # | Point | Verdict | Pièce |
| --- | --- | --- | --- |
| 1 | espace de nommage du mutex | **`Global`**, aux **5** exécutions vertes | `espace_mutex="Global"` |
| 2 | `WAIT_ABANDONED` survient-il ? | **OUI** | §5 |
| 3 | la garde de boucle locale se déclenche-t-elle ? | **OUI, et c'est le chemin NOMINAL** | §4 |
| 4 | quel réveil a servi ? | **`evenement`**, aux **5** | `reveil="evenement"` |
| 5 | le compteur de retards | **0**, y compris **cumul 0 sur deux fois dix minutes** | §6 |

**La ligne unique qui porte quatre d'entre eux**, identique aux cinq exécutions
vertes (`e2-v1`, `e2-v2`, `e2-silence2`, `e2-longue`, `e2-excl`) :

```
micro : ecriture sur le cable ARMEE  format=48000 Hz, 2 canaux, 32 bits, flottant
                                     reveil="evenement"  espace_mutex="Global"
```

🔴 **DEUX CHEMINS DE REPLI SONT DU CODE LIVRÉ ET JAMAIS COURU**, et c'est le
corollaire qu'il ne faut pas perdre : le repli **`Local\`** du mutex
(`windows_micro/verrou.rs`) et le repli **sur échéance** du réveil
(`Reveil::Echeance`). Leurs `warn!` n'ont jamais été émises, sur aucune
machine. Les deux sont raisonnés et compilés, **pas éprouvés**. Les deux
fichiers le portent désormais dans leur en-tête.

🔵 **`AUDCLNT_STREAMFLAGS_EVENTCALLBACK` est accepté** : la spec §6 l'affirmait
sans l'avoir mesuré, et **aucun code de ce dépôt n'avait jamais ouvert un flux
de RENDU WASAPI**. C'est mesuré, et la spec avait raison.

---

## 4. 🔴 La garde de boucle locale mord — et c'est le chemin nominal de cette VM

Sans `AUDIO_PERIPHERIQUE`, le rendu par défaut de cette VM **est le câble** :
le loopback du chantier A capterait le point de terminaison même sur lequel le
micro écrit, et l'utilisateur s'entendrait lui-même.

```
WARN agent::demarrage::micro: micro indisponible, la session continue sans
  erreur=micro DESACTIVE : le loopback audio de cette session capte le cable meme
  sur lequel le micro ecrirait ({0.0.0.00000000}.{deec1914-…}) — l'utilisateur
  s'entendrait lui-meme. Remede : posez AUDIO_PERIPHERIQUE sur un AUTRE rendu.
  Disponibles : «Haut-parleurs (Steam Streaming Speakers)» […], «HDP-V104 […]»,
  «Haut-parleurs (VB-Audio Virtual Cable)» […]
```

Les deux identifiants comparés sont **le même** `{deec1914-…}`. Quatre moitiés
relevées : le `warn!` nomme le remède **et** l'inventaire ; `ready` porte
`mic: false` **sur une session `ice=connected`** ; le juge rend
**`CRETE=0,000000`** ; et **0** ligne `micro ecrit sur le cable` — le fil de
rendu n'a jamais démarré.

🔵 **C'est ce critère qui donne son sens à la configuration de tous les
autres** : `AUDIO_PERIPHERIQUE='Steam Streaming'` n'est pas un contournement de
recette, c'est **le remède que la garde nomme elle-même**.

⚠️ Une première exécution (`e2-boucle-1`) n'a rendu que la moitié agent — sa
session WebRTC n'a pas abouti. Elle est versée et **comptée pour ce qu'elle
vaut**.

---

## 5. L'exclusivité : ce qui est mesuré, et ce qui ne l'est PAS

⚠️ **LE CRITÈRE ⑤ TEL QUE LE PLAN L'ÉCRIVAIT N'A PAS ÉTÉ JOUÉ.** Il exigeait
« deux enfants, micro allumé dans les deux ». **Deux agents mono-fenêtre ne
peuvent pas coexister sur cette VM** : DXGI n'autorise qu'**une** duplication
ouverte par sortie — doctrine établie de ce dépôt —, et le second mourrait
avant d'atteindre le micro. Deux enfants exigent le mode **superviseur** et ses
sorties virtuelles, hors du périmètre des tâches 11 à 13, sur une VM dont le
registre reste pollué (legs n°4 de D9).

**Ce qui a été exercé est le chemin de code exact du critère, avec un
contendant qui n'est pas un enfant** : un processus tiers acquiert
`Global\guacamole-agent-micro-cable`.

| Temps | Fait | Pièce |
| --- | --- | --- |
| 22:55:20 | un tiers **crée et acquiert** le mutex | `e2-tenir-mutex.log` |
| 20:56:09 | l'agent est **refusé**, **une seule** ligne | `micro : une autre fenetre tient deja le cable…` |
| pendant | le juge n'entend **rien**, alors que 6590 paquets montent | `e2-juge-excl-refus.log` : `AMPLITUDE=0,000000` |
| ~20:56:54 | le tiers est **TUÉ** (`Stop-Process -Force`), sans `ReleaseMutex` | `DETENTEUR TUE` |
| 20:56:56 | l'agent **acquiert après le refus** | `micro : cable acquis apres un refus` |
| après | le juge entend **440,0 Hz** | `e2-juge-excl-reprise.log` |

**Deux acquis que rien d'autre n'établissait** :

1. 🔵 **`WAIT_ABANDONED` survient réellement.** Le détenteur n'a jamais relâché ;
   le seul chemin par lequel l'agent pouvait obtenir le mutex est celui que
   `verrou.rs` déclarait nominal. C'était une **prédiction** de la Décision 2.
2. 🔵 **Le refus n'est PAS collant**, et la correction que E1 n'avait pas vue
   fonctionne : 46 s après le refus, **sans redémarrage ni renégociation**, la
   session redevient sonore — et **le journal, lui, reste unique**.

⚠️ **Ce montage n'établit PAS que deux enfants s'arbitrent.**

---

## 6. Dix minutes, deux fois : aucune grandeur ne croît

| | passage 1 | passage 2 |
| --- | --- | --- |
| lignes de trace | 615 | 630 |
| `retards` **cumulés** | **0** | **0** |
| `insertions` cumulées | **0** | **0** |
| `sauts` cumulés | 3 (amorçage) | 3 (amorçage) |
| `plc` cumulés | 18 | 10 |
| `deposees` / minute | **50** aux neuf | **50** aux neuf |
| `silence` / minute | 5 à 10 **sur 48 000** | 5 à 9 **sur 48 000** |

**Côté navigateur** (passage 2, onze relevés, `pilote-longue-2.json`) :
croissance **exactement linéaire**, 3038 paquets montants par minute, RTT 1 à
2 ms, **`packetsLost = 0` aux onze**, `ice=connected` de bout en bout —
**30 369** paquets à t+600 s.

🔵 **C'est aussi la mesure de l'EFFET des trois drapeaux anti-gel**, que la
tâche 11 ne pouvait pas faire : deux sessions de 11 minutes du chantier TURN
s'étaient interrompues à 331 s et 340 s faute de ces drapeaux, et la cause avait
été imputée au relais. **La page n'a pas gelé.**

Le juge, trois fois sur le passage 1 : **440,0 Hz** à t+120 s, t+300 s et
t+540 s.

⚠️ **NON EXPLIQUÉ, et déclaré tel : l'amplitude décroît de 0,056246 à 0,046255
sur sept minutes, soit −17,6 %.** Trois points, une exécution : **c'est un
relevé, pas une tendance établie**, et ce n'est aucune des grandeurs du critère.
Rien ne bouge en regard côté agent. `autoGainControl` — armé par
`client/src/micro.ts` — est un suspect **non éprouvé**.

⚠️ **Le compte rendu navigateur du passage 1 est PERDU** : l'appel qui le portait
a été interrompu au bout de dix minutes exactement, et le pilote est mort avec
son shell avant d'écrire son JSON. Le passage 2 a été rejoué **pour cette seule
raison**.

---

## 7. Ce que E2 n'a PAS fait, et qu'il ne faut pas déduire du succès

🔴 **PERSONNE N'A ÉCOUTÉ.** L'enregistreur vocal Windows réglé sur CABLE Output
(spec §11.1) n'a pas été ouvert. **Le critère de fin de la spec §13 n'est donc
atteint que par un JUGE LOGICIEL.** Ce juge ouvre le point de terminaison
exactement comme le ferait l'application et juge sur la fréquence dominante,
mais il ne dit rien de ce que cela vaut **à l'oreille** — ni qualité, ni
distorsion, ni dégradation du rééchantillonnage.

**L'appel réel sans écho** (spec §11.2) exige un correspondant humain : non
joué.

---

## 8. La latence : le troisième critère de la spec §13 N'EST PAS JUGÉ

Rien dans ce dépôt ne mesure la latence de bout en bout depuis D1, et E2 ne la
mesure pas davantage. Ce que E2 relève est **une composante sur deux** :
les **retards d'échéance** du fil de rendu (**0**).

⚠️ **L'occupation du tampon, l'autre composante, N'EST PAS OBSERVABLE sur le
chemin de production.** La trace `micro ecrit sur le cable` porte `ecrites`,
`silence`, `retards`, `deposees`, `sauts`, `insertions`, `plc` et
`plc_plafonnees` — **ni `famines`, ni `occupation_ms`**. Ces deux-là ne vivent
que dans la trace du puits de **mesure** de E1, qui est un instrument de banc et
**ne peut pas coexister avec le câble** (Décision 10 : deux puits ne consomment
pas un même `LecteurMicro`). **C'est l'instrument qui manque, pas la mesure.**

---

## 9. Le câble, et ce qu'il impose toujours

**`CABLE Output` reste à 44 100 Hz** quand `CABLE Input` est à 48 000 — relevé
par le juge à ses **19** exécutions, sans exception. Le remède au registre est
**INAPPLICABLE** : la tâche 2 a établi qu'il **casse** le point de terminaison
(`0x88890008`, `AUDCLNT_E_UNSUPPORTED_FORMAT`).

**Le critère d'écoute y survit** — un rééchantillonnage préserve la fréquence
d'un ton pur, et 440 Hz est retrouvé à 440,0. ⚠️ **La qualité et la latence que
cette conversion coûte ne sont mesurées par PERSONNE**, et elles ne le seront
pas par un juge de fréquence.

---

## 10. 🔴 Le piège le plus réutilisable : LA CRÊTE MENT, et elle a menti trois fois

VB-Cable **rejoue une fois** le contenu resté dans son tampon à tout client de
capture neuf. Dans cette recette seule, trois relevés portent une crête franche
sur un spectre **rigoureusement nul** :

| Journal | Crête | Amplitude | Ce que la crête rejouait |
| --- | --- | --- | --- |
| `e2-juge-v1.log` (premier passage) | 0,449513 | *(440,0 Hz réels)* | le 660 Hz du témoin |
| `e2-juge-boucle-1.log` | 0,054569 | **0,000000** | le 440 Hz de V2 |
| `e2-juge-silence-1.log` | 0,054249 | **0,000000** | idem |
| `e2-juge-excl-refus.log` | 0,449822 | **0,000000** | le 660 Hz du témoin |

**Une recette qui aurait jugé à la crête aurait conclu « ça porte » là où rien
ne portait, et trois fois.** On juge à la **fréquence dominante**, jamais à un
compte d'octets ni à une crête seule — doctrine payée en D7, confirmée par la
sonde de la tâche 1, et re-confirmée quatre fois ici.

⚠️ Corollaire de lecture : `FREQUENCE=80.0` est la **borne basse du balayage**.
Sur du silence, toutes les puissances valent 0 et c'est le premier pas qui
gagne. **C'est `AMPLITUDE` qui tranche, jamais `FREQUENCE` seule.**

---

## 11. Deux silences, et un critère à moitié réfuté

**Le critère ④ annonçait « 60 s de DTX : `plc_plafonnees` court ».** La moitié
`plc_plafonnees` **est réfutée, et elle ne pouvait pas être tenue ainsi.**

- **Le silence de E1** : la **source** de la piste est arrêtée
  (`OscillatorNode` éteint) ⇒ Chrome n'a plus rien à encoder, `packetsSent` se
  fige. C'est du DTX, et c'est là que le plafond de dissimulation existe.
- **Le silence de E2** : un **périphérique de capture vivant** qui produit du
  silence **numérique** ⇒ Chrome émet **50 paquets/s sans interruption**,
  `plc = 0` **ET** `plc_plafonnees = 0`. Aucune trame ne manque.

✅ **Ce que le critère jugeait vraiment est TENU** : `AMPLITUDE=0,000000` aux
deux exécutions. **Et le plafond a bien été vu courir** — sur une piste
**ARRÊTÉE**, `plc_plafonnees=100/s`, le câble restant à **3,1 × 10⁻⁵** là où E1
relevait 0,53 à 0,67.

⚠️ **La spec §« Silence » voit sa prémisse rétrécie, sa conclusion tenir** :
« le câble doit être alimenté en continu » est juste **pour les deux silences**.
⚠️ *La recette a mesuré du silence NUMÉRIQUE, pas une pièce calme : l'écart
entre les deux n'est éprouvé par rien.*

---

## 12. L'instrument, et la mesure qui n'était pas prévisible

Le pilote ouvre un `getUserMedia` **réel**, avec les trois contraintes exactes
de `client/src/micro.ts`. E1 posait un `OscillatorNode` sur le `sender` : ni la
permission, ni les contraintes, ni le pipeline audio de Chrome n'étaient
exercés. **Le repli de E1 n'a pas eu à servir.**

| Contrôle | Piste | Dominante | Détachement | Crête temporelle |
| --- | --- | --- | --- | --- |
| `instrument-ton-440.json` | oui, `live` | **439,45 Hz** | **+106,9 dB** | 0,398237 |
| `instrument-silence.json` | oui, `live` | — | `-Infinity` | **0,000000** |

Résolution **5,859 Hz par bac**, écrite par l'instrument — E1 laissait la sienne
à inférer, et son document de résultats le compte comme une lacune.

🔴 **Ce second étage n'était pas prévisible.** `noiseSuppression: true` arme NS3,
dont l'objet est d'effacer le bruit **stationnaire** — et une sinusoïde continue
en est un du point de vue de l'algorithme. Un contrôle qui se serait arrêté à
« `getUserMedia` rend une piste » aurait passé sur un périphérique dont le
contenu est **intégralement effacé**, et toute la recette aurait mesuré le
silence en croyant mesurer une voix. Le WAV **haché**, écrit d'avance comme
repli contre ce risque, **n'a pas eu à servir**.

⚠️ **Deux corrections d'instrument, faites parce que la première rédaction
mentait sans se tromper** : `etatPiste` était relevé **après** `stop()` et
rendait `ended` sur une piste saine ; `-Infinity` se sérialisait en `null`, qui
se lit « pas mesuré » au lieu de « rigoureusement aucune énergie ».

**Les WAV ne sont pas versés** (4,6 Mo) : le générateur est **déterministe**,
les empreintes sont versées, et la reproduction bit pour bit a été **vérifiée
dans le tour même**.

---

## 13. La revue transverse de fin de branche — huit affirmations, quatorze places

Barème du dépôt : 5 en D7, 3 en D8, 6 en D9, douze en D10, sept en D11, huit en
P1, dix en P2, cinq en S1, neuf en E1, douze en P3, douze en S2, treize en S3,
huit en P4, huit en G1, vingt-sept en S4, dix-sept au presse-papier P1.
**Huit ici, sur quatorze places.** Toutes franchissent une frontière de tâche.

| Place | Ce qui était devenu faux |
| --- | --- |
| `wasapi/ecriture.rs`, doc de `Reveil` | « C'est une **prédiction** » — mesuré : `evenement` 5/5 |
| `wasapi/ecriture.rs`, `Reveil::Echeance` | présenté sans dire qu'il **n'a jamais couru** |
| `windows_micro/verrou.rs`, en-tête | le repli `Local\` présenté comme un chemin employé |
| `windows_micro/verrou.rs`, `tenter` | `WAIT_ABANDONED` « cas nominal » — raisonnement devenu **relevé** |
| `windows_micro.rs`, en-tête | « si ce compteur reste à zéro, la question est tranchée » — **il l'est** |
| `micro.rs` | « le bloc E2 **n'aura qu'à** appeler » — au futur. ⚠️ Édition **neutre en lignes** : 483 pour une porte à 500 |
| plan E2, critère ④ | la moitié `plc_plafonnees` **réfutée** |
| plan E2, tâche 13 step 1 **et** § « n'établira pas » | **deux** grandeurs sur cinq n'existent pas — **deux places** |
| spec, §Silence | prémisse rétrécie, conclusion intacte |
| plan **E1**, trois marques | « E2 doit en choisir un autre » — **E2 a tranché** : `wasapi/ecriture.rs`, et `rendu.rs` **réutilisé** |

Et une **addition** qui n'est pas une correction : `micro/dissimulation.rs`
porte désormais qu'**il y a deux silences** (§11). Cela ne réfute rien de ce
module ; cela **borne** le mot qu'il emploie.

🔴 **LE CONTRÔLE DE COMPILATION A DEMANDÉ UN MONTAGE, ET LE RÉSULTAT EST À
DÉCLARER : `cargo check --target x86_64-pc-windows-gnu` ÉCHOUE sur l'arbre
courant, et ce n'est PAS de ce chantier.** Deux erreurs dans
`agent/src/apps/boucle.rs` (`IconesManquantes` non couvert, champs `icone` et
`source_max` manquants) : le protocole **v3** du chantier **G2** est livré dans
`proto/` et `agent/` ne l'a pas encore rattrapé. **Établi et non affirmé** :
**HEAD nu**, dans un arbre isolé et sans une seule de mes corrections, rend les
**deux mêmes erreurs**. Mes corrections ont donc été contrôlées sur **`016b611`**
— mon dernier commit avant celui de G2, où l'arbre compile : **16 avertissements
tous `dead_code`**, **706 tests d'hôte**, conformes aux références d'entrée.

---

## 14. Ce que E2 n'établit PAS

- **Aucun taux.** Deux exécutions par critère au mieux, une pour l'exclusivité.
- **⑤ n'exerce pas deux enfants** (§5).
- **Personne n'a écouté** (§7) ; **la latence n'est pas jugée** (§8).
- **L'écho à deux fenêtres n'est pas mesurable par ce montage** (§6 du plan,
  tâche 13 step 2).
- **Les deux replis — `Local\` et échéance — sont du code jamais couru** (§3).
- **`MICRO_FAUTE_ECRITURE` n'a jamais été armée** : le chemin d'échec d'écriture
  WASAPI n'a pas couru.
- **Une seule fréquence de source, un seul débit, une seule application, une
  seule fenêtre**, et **un navigateur sans interface à décodage logiciel** —
  pas un client réel, pas de HiDPI.
- **La qualité du rééchantillonnage du câble n'est mesurée par personne** (§9).
- **La décroissance de 17,6 % de l'amplitude n'est ni expliquée ni confirmée**
  (§6).
- **Aucune constante n'est calibrée**, et aucun **jugement d'écoute** n'a jamais
  été porté sur aucune constante de ce dépôt.

---

## 15. Pièges neufs — à connaître avant de toucher à ce terrain

- 🔴 **LA CRÊTE MENT, quatre fois dans cette seule recette** (§10).
- 🔴 **Sous zsh, `"$VAR:suffixe"` applique le modificateur d'historique `:e`** et
  **mange la valeur** : `$P:e2-v1` rend `2-v1`. Reproduit et isolé contre bash.
  Le symptôme est un `SESSION_ID` tronqué dans `C:\dev\run-agent.ps1`, donc un
  agent qui s'enregistre sous un nom que personne n'appelle. **Toujours des
  accolades.** *(C'est le jumeau du piège déjà documenté « zsh ne découpe pas
  les variables en mots » — qui a d'ailleurs fait rendre zéro au premier `grep`
  de la revue transverse.)*
- 🔴 **`AGENT_VM` attend l'IDENTIFIANT de la VM, pas son NOM.** Le refus est
  **volontairement indiscernable** d'un mauvais secret (aucun oracle
  d'énumération) : rien sur le fil ne dit lequel des deux est en cause. Seul le
  journal de la plateforme le montre. ⚠️ **Cinq refus arment un frein** de
  ~900 s, en mémoire — relancer le service le vide.
- 🔴 **`getUserMedia` n'existe pas sur une origine non sûre.**
  `http://192.168.3.1:5173` — l'adresse qu'employait E1 — n'en est pas une ;
  `http://127.0.0.1:5173` en est une. E1 ne s'en apercevait pas : il n'appelait
  jamais `getUserMedia`.
- ⚠️ **Un agent survivant tient `C:\dev\agent.log`** : le nouveau `StreamWriter`
  ne peut pas l'ouvrir et **l'on relit le journal de la tentative précédente**.
  Payé trois fois en D8, une fois de plus ici — d'où `e2-tuer.ps1` en tête de
  chaque lancement.
- ⚠️ **`nodejs-winrm` rend la main dès la PREMIÈRE ligne et TUE le processus
  distant.** Le juge écrit dès sa première ligne : il lui faut une enveloppe
  muette (`e2-juge.ps1`).
- ⚠️ **`scripts/run-agent.sh` exige `.env`, et `.env` porte
  `SIGNALING_URL=…:8080`** — la plateforme d'un AUTRE chantier. Un
  `set -a; source .env` **postérieur** aux réglages de recette les écrase en
  silence, et l'agent va parler au mauvais service. **L'ordre compte.**
- ⚠️ **Corrélation relevée sur quatre exécutions, cause NON identifiée** : quand
  le client arrive **~10 s** après l'agent, ICE passe `Connected` aussitôt
  (`écoulé=11,5 s`) ; quand il arrive **~42 à 48 s** après, ICE reste en
  `Checking` puis se déconnecte au bout de 18 s. Quatre points ; **rien n'exclut
  une coïncidence**. Nommée parce qu'elle a coûté deux exécutions.
- ⚠️ **Le harnais borne un appel à dix minutes** : une épreuve de dix minutes
  menée au premier plan perd le compte rendu de son pilote. La mener en
  arrière-plan.

---

## 16. Ce que E2 lègue

### À E3

1. ⛔ **L'écho à deux fenêtres**, déclaré **non mesurable par ce montage** —
   Chrome sans interface, sans haut-parleur ni microphone réels, n'a **aucun
   chemin acoustique**. Le protocole qu'un humain doit jouer est écrit au §« Step
   2 » de la tâche 13 : deux fenêtres émettant du son, micro dans une seule,
   **sans casque** puis **avec**. L'écart est la mesure.
2. ⛔ **Le refus d'exclusivité n'est PAS dit au client** (Décision 9) : à deux
   fenêtres, le bouton micro de la perdante s'allume et **rien ne sort**. Le
   fermer demande un message de contrôle neuf, donc `proto/` **et** `client/`.

   ✅ **FERMÉ SUR PIÈCES par le bloc E3** (21 août 2026), et par exactement le
   remède annoncé : `AgentControl::MicState { granted }`, `proto/` **et**
   `client/`. Émis **SUR TRANSITION** — cinquante messages par seconde
   entreraient sinon dans une file bornée à 32. Le bouton reste `'actif'`,
   seuls le libellé et le bandeau changent : éteindre un bouton dont le
   navigateur émet réellement est le mensonge visuel que la spec §9 écarte.
   ⚠️ **FERMÉ SUR PIÈCES, PAS EXERCÉ SUR LA VM** : cinq tests d'hôte et trois
   rouges (R3, R4, R5) l'établissent ; **aucune session réelle ne l'a montré**,
   la VM étant tenue par un chantier concurrent pendant tout E3.
3. ⛔ **L'exclusivité à DEUX ENFANTS n'est pas exercée** (§5) — il y faut le mode
   superviseur.

   ⛔ **TOUJOURS PAS EXERCÉE après E3, et pour une raison EXTERNE au produit** :
   la VM Windows était tenue par le chantier « pont fichiers F5 » pendant tout
   le bloc. ⚠️ **Une des trois raisons que ce document donnait est en revanche
   PÉRIMÉE** : « le registre reste pollué » (§5) ne borne plus rien depuis D10,
   qui fait tolérer au superviseur une sortie née trop grande — 3 → 10 fenêtres,
   32 → 0 erreur, sur un registre laissé sale. Les deux autres tiennent.

### Sur le câble

4. ⛔ **Le rééchantillonnage 48 000 → 44 100 subsiste**, le remède est
   **inapplicable** (il casse le point de terminaison), et **sa qualité comme sa
   latence ne sont mesurées par personne**.
5. ⛔ **La licence VB-Audio est gratuite en usage PERSONNEL seulement** — legs de
   E1, **à régler avant mise sur le marché**.

### Propres à E2

6. 🔴 **Personne n'a écouté** : le critère de fin de la spec §13 n'est atteint
   que par un juge logiciel.
7. 🔴 **La latence ajoutée par l'agent n'est pas jugée**, et l'occupation du
   tampon **n'est pas observable** sur le chemin de production (§8). La rendre
   observable demanderait de l'ajouter à la trace du câble.

   ✅ **La SECONDE moitié est fermée par le bloc E3** : `occupation_ms` et
   `famines` sont sur la trace `micro ecrit sur le cable`, lus **sous le même
   verrou** que les compteurs pour qu'ils décrivent le même instant. Les deux
   grandeurs existaient déjà et n'étaient lues par personne sur ce chemin.
   🔴 **La PREMIÈRE moitié tient ENTIÈREMENT** : la latence de bout en bout
   n'est mesurée par **rien** dans ce dépôt depuis D1, et **le troisième critère
   de la spec §13 reste NON JUGÉ**. E3 en rend la seconde composante observable,
   il ne la juge pas — et **le contrôle qui la lirait n'a pas été joué**, faute
   de VM.
8. ⛔ **Deux replis livrés et jamais courus** : `Local\` et `Reveil::Echeance`.
9. ⛔ **`MICRO_FAUTE_ECRITURE` n'a jamais été armée.**
10. ⛔ **La décroissance de 17,6 % de l'amplitude** sur sept minutes, non
    expliquée ; `autoGainControl` suspect non éprouvé.
11. ⛔ **Aucune constante calibrée**, aucun jugement d'écoute.
