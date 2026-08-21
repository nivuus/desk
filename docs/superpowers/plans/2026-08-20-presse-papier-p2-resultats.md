# Sous-projet ① « Divers » — presse-papier, sous-bloc **P2** : le navigateur colle dans la VM — résultats

> ⚠️ **DEUX « P2 » DANS CE DÉPÔT.** Celui-ci est le **presse-papier**. Le
> sous-projet ⑤ (plateforme) en a un autre, clos et sans rapport. Quand ce
> document écrit « P1 », il désigne toujours le presse-papier P1.

Plan : `2026-08-20-presse-papier-p2.md`.
Conception : `../specs/2026-08-19-presse-papier-design.md`.
Journaux : `journaux-presse-papier-p2/`.

---

## 0. Note de lecture des journaux — RELEVÉE, pas supposée

`familles-de-lecture.txt` porte le relevé brut, pris par `file` et `grep`
**après la dernière écriture**. **Trois familles** :

| Famille | État | Ce qu'il faut faire |
| --- | --- | --- |
| les quatre `agent-*.log` **bruts** | UTF-8, **CRLF**, **séquences ANSI PRÉSENTES** | `sed 's/\x1b\[[0-9;]*m//g'` — ou lire le `-plat` jumeau, **versé pour chacun** |
| les quatre `agent-*-plat.log` | UTF-8, CRLF, ANSI retirées | rien |
| les JSON du §0 et de la recette, `familles-de-lecture.txt`, l'instrument | UTF-8, LF, ni ANSI ni CRLF | rien |

✅ **AUCUN octet NUL dans aucun fichier**, vérifié par balayage — à la
différence de D10 (558 octets NUL) et de P1 (33 à 36 dans ses journaux de
pilote), où `grep` sans `-a` rendait une sortie **vide** indiscernable d'un
zéro.

---

## 1. Le verdict, critère par critère

**QUATRE exécutions exploitées** : deux du bras armé, deux du bras désarmé
(`PRESSE_PAPIER_GARDE=0`). **AUCUN TAUX n'est revendiqué nulle part.**

| # | Critère | Verdict | Exéc. |
| --- | --- | --- | --- |
| ① | Le `paste` parvient avec le focus sur le `<video>` | **TENU** — mesuré hors VM (§0 du plan), et rejoué **implicitement** ici : un collage qui arrive prouve qu'un `paste` est né | **2** (§0) + **4** (implicite) |
| ② | Coller depuis le presse-papier local fonctionne, `clipboard-read` REFUSÉE | **TENU**, et sa rouge est **discriminante** | **4** |
| ③ | Le contenu collé est le **DERNIER** copié, jamais le précédent | **TENU** | **4** |
| ④ | Après **k** collages, **aucun** message `clipboard` ne revient | **TENU**, et sa rouge rend **exactement k** | **2** + **2** rouges |
| ⑤ | Un raccourci qui n'est pas un collage garde son `preventDefault` | 🔴 **NON MESURÉ**, et déclaré | **0** |

### ④ — le fait central, et il est double

```
bras ARMÉ      messages « clipboard » en retour :  0, 0, 0, 0   (4 collages)
bras DÉSARMÉ   messages « clipboard » en retour :  1, 2, 3, 4   (4 collages)
```

et **chacun des quatre messages du bras désarmé porte exactement le texte
qu'on venait de coller** (`pp2-desarme-{1,2}.json`). La rouge n'est donc pas
vacueuse : le compteur **peut** quitter zéro, et le zéro du bras armé dit
quelque chose.

🔴 **C'est la rouge que la spécification prescrivait qui était vacueuse, et
elle avait prévu le cas.** Elle demandait de désarmer le seul garde n°1 et
d'attendre un compte « qui croît sans borne » ; il **reste à un**, et la
démonstration tient au code (D-P2-5). D'où une variable qui désarme les **deux**
gardes. La mesure confirme les deux moitiés du raisonnement.

> 🔵 **ET CETTE MESURE COUVRE CE QU'AUCUN TEST NE POUVAIT COUVRIR.** Le code de
> P2 déclare lui-même que l'ordre dans la boucle du tour de roue n'est gardé par
> aucun test : intervertir `armer_les_gardes` et `sondeur.tour()` dans
> `registre.rs` laisse **les sept tests du module verts** (mesuré, pas
> conjecturé). Or si l'ordre était inversé, le bras **armé** verrait le
> `Sondeur` relire notre propre écriture et l'annoncer — il rendrait des
> messages, pas zéro. **Le zéro du bras armé EST l'épreuve de cet ordre**, et
> c'est la recette qui la donne. Le fait de lecture est devenu un fait mesuré.

### ② — la rouge pouvait changer quelque chose, et elle n'a rien changé

Avant tout collage, aux quatre exécutions : `clipboard-read` = **`prompt`**,
`navigator.clipboard.readText()` = **`THROW:NotAllowedError`**. **Le produit ne
demande aucune permission de presse-papier**, et c'est le meilleur résultat de
ce chantier.

Puis la rouge : on **accorde** `clipboardReadWrite` et on recolle.

| | avant | après le grant |
| --- | --- | --- |
| `clipboard-read` | `prompt` | **`granted`** |
| `readText()` | `THROW:NotAllowedError` | **`OK:gamma-arme-1-crwor9`** |
| le collage | traverse | **traverse à l'identique** |

**L'observable a bien changé** — le grant a pris —, **et le chemin du collage
n'a pas bougé d'un pixel.** Une rouge qui ne changerait rien parce que le grant
n'aurait pas pris ne prouverait rien ; celle-ci prouve.

### ③ — le dernier copié, jamais le précédent

Le Bloc-notes est vidé avant la mesure, puis croît de **19, 18, 19, 19** octets
au bras armé (22, 21, 22, 22 au désarmé, les nonces y étant plus longs) :
jamais deux fois le même texte, jamais le précédent.

```
arme-1  notepad=[alpha-arme-1-nz1w56beta-arme-1-nz1w56gamma-arme-1-nz1w56delta-arme-1-nz1w56]
```

⚠️ **La rouge que le plan prescrivait pour ③ — un binaire distinct qui enverrait
la touche `V` sur le canal d'entrées au lieu de l'injection — N'A PAS ÉTÉ
JOUÉE.** Elle exigeait un second binaire, donc un second cycle de compilation
sur une VM contendue, et le plan autorisait explicitement de « déclarer le
critère non joué » à ce titre. **Ce qui est établi est donc que l'ordre
FONCTIONNE, pas que le chemin naïf échouerait** — et le plan écrit lui-même que
« le chemin naïf marche aussi, la plupart du temps ». **La justification de D6
n'est donc pas éprouvée par cette recette.** C'est le legs n°1.

### ⑤ — NON MESURÉ, et pourquoi

L'hôte **n'a aucun serveur X** — établi par le témoin de P1 : `xclip` et
`wl-paste` absents, `xsel` échouant en « Can't open display ». Aucun Chrome
**avec interface** n'y est donc lançable, et `--headless` n'a ni onglets ni
fenêtres au sens de l'utilisateur (§0.4 du plan).

**Couverture de repli, nommée** : les **15** cas de `raccourcis.test.ts` (dont
treize refus, `Ctrl+W`/`T`/`N` compris) et les **10** d'`input.test.ts`. ⚠️ **Ils
prouvent le PRÉDICAT et sa LIAISON à l'écouteur, jamais le comportement du
NAVIGATEUR.** Déclarer ⑤ tenu sur leur foi serait faux.

---

## 2. 🔴 Le défaut que la recette a trouvé : le presse-papier fuyait EN CLAIR

**C'est le résultat le plus important de cette recette, et aucun test ne
pouvait le donner.** La première exécution armée a relevé dans `agent.log` :

```
INFO agent::demarrage: contrôle reçu session=… Clipboard { version: 3, text: "alpha-arme-1-crwor9" }
```

— **le contenu du presse-papier de l'utilisateur, en clair, dans un journal que
ce dépôt VERSE DANS GIT.** La décision D-P1-7 l'interdit nommément (« UNE SEULE
TRACE, ET JAMAIS LE TEXTE »), et P1 l'avait tenue **dans le sens descendant** :
`capteur::sommeil::presse_papier::distribuer` ne journalise qu'`octets` et
`refus`.

⚠️ **Ce n'est PAS un défaut du site de journalisation, et le distinguer change
le remède.** La trace fautive (`demarrage.rs`, `on_control`) est **antérieure**
au chantier presse-papier : elle imprime le message reçu par `?message`, ce qui
était inoffensif tant qu'aucune variante ne portait de contenu privé. **C'est P2
qui a rendu cette trace dangereuse** en ajoutant `ClientControl::Clipboard` — et
un remède posé sur le **site** aurait laissé le **prochain** site fuir.

**Le remède est au TYPE** : `Debug` est implémenté **à la main** sur
`ClientControl` **et** sur `AgentControl`, `#[derive(Debug)]` retiré des deux.
Le texte y est remplacé par sa **taille** — ce qui garde au journal tout son
pouvoir de diagnostic, la taille étant justement ce que la borne et le refus
mettent en jeu. **Exhaustif par construction** : ajouter une variante oblige
désormais à décider ce qu'elle montre.

**Les deux sens sont rédigés**, pas seulement celui qui fuyait : attendre qu'un
site apparaisse pour `AgentControl` serait attendre la même fuite dans l'autre
sens.

**Après remède, sur les quatre journaux versés** : `grep` des nonces →
**0 ligne** dans les quatre, et le journal montre `Clipboard { v: 3, octets: 22 }`.

⚠️ **Le journal qui portait la fuite a été ÉCRASÉ par la ré-exécution sous le
même nom** — c'est le piège que P1 avait nommé (« un journal d'agent s'écrase
facilement, et deux pièces ont été perdues ainsi »), payé une fois de plus. Les
quatre lignes fautives sont **citées verbatim** dans le commentaire de
`proto/src/control.rs`, et le défaut reste **re-prouvable par la rouge du
test** : remettre `#[derive(Debug)]` fait tomber exactement un test.

---

## 3. Ce que le montage a coûté, et ce qu'il enseigne

### 3.1 🔴 `FindWindowW('Notepad', $null)` rend ZÉRO sur une fenêtre présente

**PowerShell marshale `$null` en CHAÎNE VIDE pour un paramètre `string`.** La
première rédaction du lecteur cherchait donc un Bloc-notes dont le **titre** est
vide, et `FindWindowW($null, <titre>)` cherchait une **classe** vide — d'où
`ERROR_INVALID_NAME` (123). Les deux rendaient zéro sur une fenêtre bien
présente.

**Ce qui a tranché est qu'`EnumWindows`, lui, la trouvait** (`Notepad|*Sans
titre - Bloc-notes|14576`) : deux API sur le même bureau, l'une voit, l'autre
non. `[NullString]::Value` est obligatoire.

### 3.2 Deux défauts d'instrument, tous deux « lus comme une panne du produit »

- **une lecture prise pendant l'écriture** de `pp2-fait.txt` rendait une ligne
  **tronquée** (`notepad longue`) qui commençait par le bon numéro d'ordre et
  passait donc le test de correspondance — CIFS et `Set-Content` ne sont pas
  atomiques. **Le témoin de mesurabilité l'a attrapé** (`longueur=0`
  introuvable) ; un critère aurait pu être jugé sur une valeur amputée, ce qui
  est pire qu'une mesure absente. Remède : **deux relevés identiques à 300 ms
  d'écart** ;
- **une fois, `schtasks` n'a rien ouvert** : aucune ligne « fenêtre » au
  journal, `enfant lancé` = 0, et le pilote rendait « aucune page d'application
  attachée ». Remède : l'ouverture du Bloc-notes est **vérifiée** par
  `Get-Process`, trois essais.

### 3.3 L'arbre est partagé, et cela a coûté deux tentatives

- **`version_emise=3 version_recue=4`** : la plateforme tournait depuis l'arbre
  principal, où **G3** a monté `PLATEFORME_VERSION` à 4. **Le refus est
  bruyant et le produit a eu raison** — c'est la collision que la décision D6 du
  plan de P4 avait nommée d'avance ;
- **`build-agent.sh` a échoué** sur un `mod installation` dont le fichier
  n'était pas encore là : il **rsynchronise l'arbre entier**, y compris le
  travail non commité du chantier **F3**. C'est le piège écrit par P4 (« ne
  jamais lancer `build-agent.sh` quand `proto/` porte des modifications non
  commitées d'un chantier voisin »).

**Remède employé, et il est la bonne forme** : l'agent, la plateforme **et** le
client tournent tous trois depuis un **worktree isolé** au commit `14675a2`.

⚠️ **Deux interruptions d'infrastructure ont eu lieu** — la VM s'est hibernée
d'elle-même (piège documenté depuis D1), puis l'hôte a perdu son `/dev/null`.
**AUCUNE des quatre exécutions rapportées n'a traversé une coupure** : les
tentatives qui les ont rencontrées ont été rejouées entièrement.

---

## 4. Ce que P2 n'établit PAS

- **Aucun taux, nulle part.** Deux exécutions par bras.
- 🔴 **La justification de D6 n'est pas éprouvée** : la rouge de ③ — le chemin
  naïf, `V` sur le canal d'entrées — n'a pas été jouée, faute d'un second
  binaire. **Ce qui est établi est que l'ordre fonctionne, pas que son absence
  échouerait.**
- 🔴 **Le critère ⑤ n'est pas mesuré**, et sa couverture de repli ne prouve que
  le prédicat.
- **Aucun navigateur autre que Chromium**, et **aucun Chromium AVEC
  INTERFACE** : ni Firefox, ni Safari, ni un vrai geste humain. Aucun humain n'a
  collé quoi que ce soit.
- **Rien au-delà d'UNE fenêtre** — P3 est le sous-bloc des N.
- **Rien de deux collages concurrents** : `SendInput` reste global à la session
  Windows, et la seule portée mesurée du dépôt est « une frappe par fenêtre,
  sonde séquentielle ».
- **Le chemin d'échec de `commander` (borne 12 s) n'a jamais couru**, et P2
  l'emprunte désormais depuis la boucle de transport.
- **Le refus de taille entrant n'a pas été exercé** : aucun collage de plus de
  `PRESSE_PAPIER_MAX` n'a été tenté sur la VM. Les deux bornes — celle du client
  et celle de l'agent — ne sont éprouvées que par leurs tests d'hôte.
- **L'écrasement du dernier collage** (D-P2-3) n'a pas été exercé : les collages
  de la recette sont espacés de plusieurs secondes.
- **`PRESSE_PAPIER_MAX` et `PERIODE_PRESSE_PAPIER` ne sont pas calibrées**, et
  elles rejoignent la liste que ce dépôt tient depuis `BPP_MIN`.
- **Aucun jugement d'usage n'a été porté sur la latence d'un collage.** Le
  pilote attend 6 s après chaque `Ctrl+V` : c'est un délai d'instrument, pas une
  mesure.
- **Le mode MONO-FENÊTRE n'a pas été exercé** : il n'a pas de collage, il le
  DIT (`Err`), et rien ne l'a vérifié sur la VM.

---

## 5. Ce que P2 lègue

1. 🔴 **La rouge du critère ③ n'est pas jouée** : sans elle, **D6 est du coût
   dont rien n'établit la nécessité**. Elle exige un binaire distinct qui envoie
   la touche `V` sur le canal d'entrées au lieu d'injecter. Point de chute :
   `client/src/input.ts`, en retirant la retenue du scancode.
2. 🔴 **Le critère ⑤ reste NON MESURÉ.** Il faut un Chrome **avec interface**,
   donc `Xvfb` — dont le consentement d'installation a été donné en D8 et jamais
   suivi d'effet. ⚠️ **Les mesures qui en sortiraient ne se compareraient à
   aucune campagne antérieure.**
3. ⛔ **Le propriétaire MONO-FENÊTRE n'existe toujours pas** (legs n°1 de P1).
   P2 le rend **bruyant** au lieu de silencieux ; il ne le comble pas. Point de
   chute : `agent/src/demarrage.rs`, **dont la tâche 3 a rendu la marge** (491 →
   426).
4. ⛔ **Le collage écrase le précédent** (D-P2-3), **sans trace**. Remède
   nommé : une file bornée dans `evenements.rs`. Non exercé.
5. ⛔ **Le chemin d'échec de `commander` (borne 12 s) n'a jamais couru**, et P2
   l'emprunte depuis la boucle de transport : un capteur mort ferait attendre la
   boucle jusque-là.
6. ⛔ **Deux collages concurrents depuis deux fenêtres sont hors de ce qui est
   établi.** **P3 les rencontrera.**
7. ⛔ **Le legs n°4 de P1 s'aggrave** : le canal `Message` du registre reste non
   borné, et le presse-papier circule maintenant dans les **deux** sens à 64 KiB
   par geste.
8. ⛔ **Le refus de taille entrant n'est éprouvé que par des tests d'hôte**, des
   deux côtés du canal.
9. ⚠️ **Une sortie virtuelle orpheline (`\\.\DISPLAY5`) préexistait à cette
   recette** et a été purgée par le superviseur à son démarrage (`topologie
   relevée … avant purge nombre=2 … après purge nombre=1`). Nommé pour que nul
   ne l'impute à P2.
