# Mesures préalables au chantier D — résultats

**Date** : 31 juillet 2026 · **Branche** : `chantier-mesures-prealables` ·
**Plan** : `docs/superpowers/plans/2026-07-31-mesures-prealables-chantier-d.md`

Ce chantier n'a livré aucune fonctionnalité. C'était un **bloc de mesure** :
lever les quatre inconnues que la sonde de capture multi-fenêtres du
30 juillet 2026 avait laissées ouvertes, et qu'elle désignait comme
**bloquantes avant de spécifier le chantier D** (« multi-fenêtres »).

Il s'appuie sur son prédécesseur, dont il ne répète pas les résultats :
`docs/superpowers/plans/2026-07-30-sonde-capture-multifenetre-resultats.md`.

**Discipline d'énoncé.** Tout chiffre cité ici provient d'un journal versé,
**nommé à sa place**. `docs/superpowers/plans/journaux-mesures-prealables/`
contient **19 fichiers `.log`**, tous en **UTF-8** — c'était la tâche 1 du
chantier — plus `canal-de-controle.md`, qui est un document de reconnaissance et
non un journal. Il ne faut donc **pas** appliquer à ces `.log` la conversion
`iconv` que réclament certains de la sonde précédente. Les quelques chiffres
repris de cette sonde nomment ses journaux à elle
(`journaux-sonde-multifenetre/`). **Deux relevés seulement n'ont pas de journal
versé** : ils sont signalés là où ils apparaissent, et repris dans « Ce qui
reste ouvert ». Rien n'est reconstitué de mémoire, rien n'est extrapolé au-delà
du rang mesuré.

Cette discipline n'est pas un ornement. La sonde précédente a passé onze rondes
de revue à corriger des rapports qui concluaient au-delà de leur relevé, jamais
des bugs — et **ce chantier a reproduit le schéma à chaque tâche** : quasiment
toutes les corrections de revue ont porté sur des énoncés, pas sur du code.
Le présent document est celui où ces énoncés se figent.

---

## Ce qui est acquis

Les quatre chiffres, chacun avec son journal.

1. **Le pilote d'affichage virtuel accepte 10 sorties simultanées ; la 11ᵉ est
   refusée.** Mesure ①. Refus du pilote sur `IOCTL_ADD_VIRTUAL_DISPLAY`
   (`0x00222000`) en `0x80070044` (`ERROR_TOO_MANY_NAMES`). Preuve par
   **identité** et non par cardinal : la ligne de refus énumère nommément les
   onze sorties présentes — au relevé qui précède immédiatement le refus, à
   **0,673 ms** près, et non par une relecture prise au moment même (voir
   §Mesure ①).
   Journal : `moniteurs-montee-en-n.log`.
   **La cible du chantier D étant 8 fenêtres, la voie recommandée par la sonde
   tient, avec 2 de marge.**

2. **Le plafond d'encodage ne bouge pas quand on sépare les périphériques
   D3D11 : 8 dans les deux modes.** Mesure ②. 8 encodeurs H.264 matériels se
   construisent, le 9ᵉ est refusé — que les encodeurs partagent un unique
   périphérique D3D11 (témoin) ou qu'ils en aient chacun un neuf
   (`peripheriques=9` côté mode séparé).
   Journaux : `nvenc-partage-temoin.log`, `nvenc-separe.log`.
   **Le partage du périphérique n'était donc pas la contrainte**, et le coût
   d'une séparation par fenêtre n'a pas à être payé — sous la réserve à deux
   variables du §Mesure ②.

3. **Windows compose bien des fenêtres sur un moniteur virtuel sans écran
   physique, et Desktop Duplication en rend l'image exacte.** Mesure ③.
   900 verdicts « Juste » sur 900 à N=1, 450 sur 450 avant recouvrement à N=2,
   **zéro image noire**, 90,0 images/s par fenêtre.
   Journaux : `moniteurs-capture.log`, `moniteurs-capture-n1-encodage.log`.
   C'était **l'hypothèse fondatrice** de la voie recommandée, jusqu'ici garantie
   « par construction » et jamais vérifiée. Elle tient.

4. **`PrintWindow` ne tient pas l'échelle : 17,6 i/s par fenêtre à N=4, 8,8 à
   N=8** (17,2 et 8,8 avec l'encodage, passe menée à son terme,
   `verdicts_faux = 0` aux deux rangs).
   Journaux : `printwindow-n4.log`, `printwindow-n8.log`.
   Recollés aux deux rangs mesurés par la sonde précédente
   (`banc-printwindow-1.log`, `banc-printwindow-2.log`), les quatre points
   donnent une décroissance **monotone du débit de pixels sur toute la plage** —
   **116,64 → 75,43 → 45,62 → 20,28 MP/s**, une division par 5,8 — sans le
   palier qu'affiche la voie `duplication` (208–258 MP/s quasi constants).

Trois acquis d'outillage viennent en plus des quatre chiffres, et ils
conditionnent la spécifiabilité autant qu'eux :

5. **Notre propre code commande le pilote d'affichage virtuel, sans Apollo.**
   La sonde précédente ne pouvait faire paraître une sortie virtuelle qu'en
   demandant au propriétaire du poste de lancer une session Apollo. Le canal est
   désormais identifié (IOCTL sur le GUID d'interface SudoVDA — document
   `canal-de-controle.md`), implémenté, et **éprouvé dix fois en création comme
   en destruction** (`moniteurs-montee-en-n.log`). La dépendance à Apollo pour
   *piloter* le pilote est levée ; celle au pilote SudoVDA lui-même demeure.

6. **Une purge autonome rattrape les sorties orphelines**, éprouvée sur un état
   réellement sale qu'elle n'avait pas produit : 8 sorties laissées par un
   processus tué net, constatées par un processus tiers (9 sorties DXGI contre
   1), retirées, retour à 1 confirmé par un quatrième processus.
   Journaux : `moniteurs-purge-salissure.log`, `moniteurs-purge-etat-sale.log`,
   `moniteurs-purge-execution.log`, `moniteurs-purge-etat-restaure.log`. Elle a
   ensuite servi pour de bon, sur une orpheline laissée par le plantage de la
   mesure ③ (`moniteurs-capture-purge-orpheline.log`).

7. **Les journaux de l'agent sont lisibles.** `scripts/run-agent.sh` écrit
   désormais en UTF-8 sans BOM, lignes entières, accents intacts. Il fallait
   **deux** réglages, pas un — voir les pièges.

### Une attribution démentie, au passage

Le composant qui refuse le 9ᵉ encodeur n'est **ni l'un ni l'autre** des deux
appels que la sonde précédente et le plan de ce chantier désignaient. C'est
`SetOutputType` de la MFT NVIDIA. Le détail, et pourquoi l'attribution
antérieure s'était égarée, sont au §Mesure ②. `CLAUDE.md` porte déjà le bloc de
correction correspondant.

---

## Ce qui reste ouvert

> ✅ **Note du 31 juillet 2026, postérieure à ce document — les deux premiers
> points ci-dessous sont LEVÉS**, par
> `2026-07-31-duplications-paralleles-resultats.md`. Les énoncés d'origine sont
> conservés intacts en dessous ; ce qui a changé :
>
> - **le défaut de libération est diagnostiqué et corrigé**, et il n'était pas
>   déterministe comme « jamais expliqué » et le bornage du §Mesure ③ le
>   laissent croire, mais **intermittent** (2 plantages sur 6 exécutions). La
>   cause : la MFT NVIDIA a un élément de travail encore en vol quand on relâche
>   l'encodeur. `MFShutdown`, d'abord accusé, a été **réfuté par la mesure** ;
> - **l'arrangement de la voie recommandée est mesuré, et la voie est reçue** :
>   90,1 i/s par fenêtre en capture+encodage à N=8, zéro verdict faux, huit
>   duplications ouvertes de front. **Une exécution par rang, donc aucun taux**,
>   et **rien au-delà de 8 sorties**.
>   ⚠️ **Portée resserrée le 1ᵉʳ août 2026 (sous-bloc D1) : le banc créait ses
>   N sorties AVANT d'ouvrir la moindre duplication, ce qui n'est pas l'ordre du
>   produit.** Rien n'est réfuté, mais en exploitation une fenêtre s'ouvre alors
>   que d'autres capturent, et la création de sa sortie tue toutes les sessions
>   en cours (`0x887A0026`) —
>   `2026-08-01-multifenetres-tranche-verticale-resultats.md`.
>   ✅ **Cet ordre-là passe depuis le sous-bloc D2** (1ᵉʳ août 2026,
>   `2026-08-01-multifenetres-arrangement-dynamique-resultats.md`) : le mutex est
>   toujours abandonné, mais la reprise l'encaisse et **aucune session n'en
>   meurt**. ⚠️ En **processus distincts**, la **5ᵉ** duplication est en revanche
>   refusée (`0x887A0022`) — plafond observé à **4**, dont **la couche n'est pas
>   identifiée**. **Ne pas transposer le 8 au multi-processus.**
>
> Cette liste compte **neuf** points, dont **trois sont levés** : les deux
> ci-dessus, plus « Aucune image n'a été soumise aux 8 encodeurs ». Chacun des
> trois porte sa propre note ✅ à sa place dans la liste — c'est là qu'il faut
> la lire, pas ici. Les **six** autres **restent ouverts**.
>
> *(Une première rédaction de cette note annonçait « les trois autres points
> restent ouverts » pour une liste de neuf : compte faux, corrigé en revue
> finale de la branche des duplications parallèles.)*

- **Un défaut ouvert et non diagnostiqué : la passe d'encodage du banc tue le
  processus**, sur la voie `duplication`. Localisé, jamais expliqué — bornage
  exact au §Mesure ③. C'est la seule réserve de ce chantier qui porte sur un
  comportement du code plutôt que sur la portée d'un relevé.
  > ✅ **Note postérieure (31/07/2026) — LEVÉ** : diagnostiqué et corrigé
  > (`2026-07-31-duplications-paralleles-resultats.md` §7,
  > `agent/src/encode/arret.rs`). Il n'était pas déterministe mais
  > **intermittent** (2 plantages sur 6 exécutions) ; cause : la MFT NVIDIA
  > gardait un élément de travail en vol au relâchement de l'encodeur.
  > `MFShutdown`, d'abord accusé, a été **réfuté par la mesure**. Après
  > correctif : **0 récidive sur 20 exécutions** du cas comparable — *ce n'est
  > pas une preuve d'absence*.
- **L'arrangement que la voie recommandée propose réellement n'est pas
  mesuré.** La mesure ③ a posé N fenêtres sur **une** sortie virtuelle. La voie 2
  promet **une** fenêtre par sortie, donc **N sorties virtuelles et N duplications
  DXGI ouvertes de front**. Que N duplications simultanées tiennent — en cadence,
  en mémoire, et vis-à-vis de la règle « une seule duplication par sortie » — n'est
  établi par rien ici. C'est la mesure suivante, et elle est courte.
  > ✅ **Note postérieure (31/07/2026) — LEVÉ** : montage exercé, **voie reçue**
  > — 90,1 i/s par fenêtre en capture+encodage à N=8, zéro verdict faux, huit
  > duplications ouvertes de front
  > ⚠️ **Portée resserrée le 1ᵉʳ août 2026 (sous-bloc D1)** : le banc créait ses
  > N sorties **avant** d'ouvrir la moindre duplication ; ce n'est pas l'ordre du
  > produit, et cet ordre-là échoue — la création d'une sortie pendant que
  > d'autres capturent tue toutes les sessions (`0x887A0026`).
  > `2026-08-01-multifenetres-tranche-verticale-resultats.md`.
  > ✅ **Cet ordre-là passe depuis le sous-bloc D2** (1ᵉʳ août 2026,
  > `2026-08-01-multifenetres-arrangement-dynamique-resultats.md`) : le mutex est
  > toujours abandonné, mais la reprise l'encaisse et **aucune session n'en
  > meurt**. ⚠️ En **processus distincts**, la **5ᵉ** duplication est en revanche
  > refusée (`0x887A0022`) — plafond observé à **4**, dont **la couche n'est pas
  > identifiée**. **Ne pas transposer le 8 au multi-processus.**
  > (`2026-07-31-duplications-paralleles-resultats.md`). **Une exécution par
  > rang, donc aucun taux**, et **rien au-delà de 8 sorties**.
- **La comparaison des deux modes d'encodage porte sur deux variables
  confondues** : le mode `separe` n'ouvre aucune duplication DXGI là où le mode
  `partage` en ouvre une. Le témoin propre — mode séparé **avec** duplication —
  n'a pas été exercé.
- **La couche qui impose le plafond de 8 encodeurs n'est pas identifiée**
  (NVENC ? pilote NVIDIA ? Media Foundation ? virtualisation ?). La mesure ②
  établit la moitié négative, pas la positive.
- **Que détruire un encodeur libère la place n'est pas établi.** La séquence
  « créer 8 → en détruire 1 → tenter un 9ᵉ » n'a jamais été exercée. La mise en
  sommeil des fenêtres masquées repose donc sur une conjecture.
- **Aucune image n'a été soumise aux 8 encodeurs.** La mesure ② porte sur la
  **création** de sessions, jamais sur leur fonctionnement : rien ne dit que 8
  encodeurs tiennent la cadence ensemble.
  > ✅ **Note postérieure (31/07/2026) — LEVÉ** : huit encodeurs ont été
  > **alimentés ensemble** pendant 10 s et ont tenu **90,1 i/s par fenêtre**,
  > zéro verdict faux
  > (`2026-07-31-duplications-paralleles-resultats.md` §3.2). Portée exacte, à
  > ne pas élargir : **huit périphériques D3D11 distincts** (un par sortie
  > virtuelle), 1280×720 à 60 Hz et 8 Mb/s, **une** exécution par rang, et
  > aucune unité H.264 décodée ni regardée. Le montage de la mesure ② —
  > périphérique unique partagé — n'a, lui, toujours jamais été alimenté.
- **La cause du refus à la 11ᵉ sortie n'est pas isolée** — borne codée dans
  SudoVDA, imposée par IddCx/Windows, ou fonction de la configuration
  d'adaptateur. Et **on ignore si le vivier de 10 est global au pilote ou par
  client** : Apollo n'avait aucune sortie ouverte pendant la mesure, mais
  `ApolloService` tourne sur la même VM et pingue le même pilote. Si le vivier
  est global, une session Apollo concurrente réduit d'autant ce qui reste.
- **L'unité du `delai = 3` du chien de garde du pilote reste entièrement
  inconnue** — **aucune unité n'est exclue, pas même la seconde**. Voir le
  §Mesure ① : la mesure du plafond ne dépend pas de cette inconnue, mais une
  exploitation durable des sorties virtuelles, elle, en dépendra.
- **Deux relevés cités dans ce document n'ont pas de journal versé** :
  - **la sonde de contrat de la tâche 5** (`MULTIFENETRE_CONTRAT`) — version de
    protocole `{majeure 0, mineure 2, increment 1, version de test}` et watchdog
    `delai=3 decompte=3` — est citée depuis `/media/vm/dev/agent.log`, jamais
    extraite dans un fichier versé. Atténuation : les deux valeurs de watchdog
    sont, elles, reproduites dans `moniteurs-chien-de-garde.log` ;
  - **le relevé Apollo de 3413×960 contre 5120×1440**, hérité de la sonde
    précédente, qui le signalait déjà comme son seul chiffre sans pièce et comme
    non reproductible sans le propriétaire du poste. Il ne fonde plus rien ici —
    la voie ne repose plus sur lui — mais il **sert d'unique contre-exemple** à
    l'énoncé du facteur d'échelle de la mesure ③ (« la borne est *cette
    résolution-là*, pas *cette VM* »). Sa réserve l'accompagne à cet endroit.

  *(Le facteur d'échelle 1,0 et les coordonnées de texture de la mesure ③,
  eux, figurent bien dans `moniteurs-capture.log` : ce point est couvert.)*
- **La disposition des structures du contrat IOCTL reste une lecture amont.**
  Le GUID d'interface et 2 des 6 codes IOCTL sont confirmés **par octets** dans
  le `SudoVDA.dll` installé (md5 `200ec71b297ca42469652256c4fa896b`, offsets
  53656 / 16316 / 16284). Les **quatre autres codes et toutes les dispositions
  de structures** viennent d'un en-tête amont du 2024-09-08, contre un pilote
  `DriverVer 07/14/2025` — onze mois d'écart. Deux tampons simples sont
  confirmés empiriquement, `ADD_VIRTUAL_DISPLAY` (56 octets) l'est par le seul
  fait qu'il fonctionne. Détail : `canal-de-controle.md`.
- **`printwindow` n'a pas été rejoué sur le banc d'aujourd'hui.** Le banc a
  gagné en cours de chantier une relecture de pixel qui court sur toute la passe
  et non plus seulement après recouvrement — voir le §Mesure ④ pour le bornage
  du risque.
- **Le plafond de 10 n'est éprouvé qu'en création.** La première sortie a vécu
  **au moins 27 s** — le journal la donne créée à 10:05:41,947 (confirmation
  DXGI) et la dernière destruction tombe à 10:06:09,019 ; comptée depuis l'IOCTL
  de création qui la précède (10:05:38,945) la durée est de 30,1 s. Rien n'est
  établi sur leur tenue au-delà, ni sur ce que Windows fait d'un bureau à onze
  écrans réellement utilisé.

---

## Mesure ① — le plafond de sorties virtuelles simultanées : **10**

*C'était la question ouverte la plus utile laissée par la sonde. Elle y était
bloquée par un obstacle matériel — il fallait un second appareil client apparié
à Apollo. Ce chantier l'a contournée en cessant de passer par Apollo : notre
code commande le pilote directement.*

### Le relevé

`moniteurs-montee-en-n.log`, `MULTIFENETRE_VDD=1` :

```
10:05:38  topologie relevée moment="avant toute création" nombre=1 attachees=1
          sortie … nom=\\.\DISPLAY1 attachee=true x=0 y=0 largeur=2400 hauteur=1080
10:05:41  sortie virtuelle créée rang=1  id=258 … parues=["\\.\DISPLAY5"]
   …                                                (un nom neuf par rang)
10:06:08  sortie virtuelle créée rang=10 id=259 … parues=["\\.\DISPLAY14"]
10:06:08  plafond de sorties virtuelles atteint — le pilote refuse la suivante,
          et toutes les précédentes sont encore là, nommément
          plafond=10
          causes=création d'une sortie 1280x720@60 (IOCTL 0x00222000) :
                 La limite de nom pour la carte réseau de l'ordinateur local
                 a été dépassée. (0x80070044)
          presentes=["\\.\DISPLAY1", "\\.\DISPLAY10", "\\.\DISPLAY11",
                     "\\.\DISPLAY12", "\\.\DISPLAY13", "\\.\DISPLAY14",
                     "\\.\DISPLAY5", "\\.\DISPLAY6", "\\.\DISPLAY7",
                     "\\.\DISPLAY8", "\\.\DISPLAY9"]
10:06:09  (dix destructions, en ordre inverse, toutes réussies)
10:06:12  état initial restauré — mêmes sorties, nommément noms=["\\.\DISPLAY1"]
```

*(Le champ `presentes` est reproduit dans l'ordre du journal — un tri
lexicographique, où `DISPLAY10` précède `DISPLAY5`. Onze noms, un seul écran
physique et dix sorties virtuelles. Les horodatages sont tronqués à la seconde
et les lignes de topologie détaillée élidées ; tout le reste est verbatim.)*

Le libellé français que Windows attache à `0x80070044` parle de cartes réseau :
c'est le texte standard de l'erreur Win32 68 `ERROR_TOO_MANY_NAMES`, sans
rapport avec le réseau ici.

### Ce que le relevé autorise à conclure

- **Le mode d'échec est un refus du pilote**, à la création. Les deux autres
  modes concevables sont **exclus par identité, pas par cardinal** : à chaque
  rang `parues` contient exactement **un** nom neuf (donc aucune création
  acceptée n'est restée invisible), et aucun nom déjà relevé ne manque à aucun
  rang (donc pas de retrait silencieux, ni de **remplacement** à la
  `ensure_only_display` — le piège d'Apollo relevé par la sonde). Un cardinal
  n'aurait pas suffi : une addition externe par Apollo aurait compensé
  exactement un retrait.
- **Corroboration indépendante** : les `TargetId` rendus par le pilote se
  prennent dans un **vivier de dix valeurs exactement, 256 à 265**, réemployées
  d'une exécution à l'autre et dans un ordre qui diffère entre les deux montées
  du journal. Un vivier fixe, donc, et non un compteur qui avancerait — ce qui
  va dans le même sens que `ERROR_TOO_MANY_NAMES`. **Corroboration, pas
  preuve** : le code du pilote n'a pas été lu.
- **L'état initial est restauré, vérifié depuis un processus neuf** —
  `moniteurs-etat-initial.log`, lancé après toutes les mesures : `nombre=1`,
  `\\.\DISPLAY1`, 2400×1080 à (0,0). Le contrôle interne au processus mesureur
  est juge et partie ; celui-là ne l'est pas.
- **`IOCTL_REMOVE_VIRTUAL_DISPLAY` est éprouvé dix fois**, et sa structure
  d'entrée de 16 octets avec lui — l'un des quatre codes que la reconnaissance
  n'avait **pas** confirmés par octets.

### Le chien de garde du pilote — pourquoi il ne fausse pas ce chiffre

C'était le risque n°1 : le pilote annonce `delai = 3` d'unité non documentée.
Si ces 3 étaient des secondes, une montée sans ping aurait vu ses sorties mourir
en route et aurait publié le plafond du chien de garde — une mesure fausse, et
fausse dans le sens qui condamnerait à tort la voie recommandée.

**Ce qui l'écarte est le journal de la montée lui-même, et rien d'autre.** Au
moment du refus, les onze sorties sont présentes **nommément** ; `\\.\DISPLAY5`
a été créée à 10:05:41,947 et elle est encore là à 10:06:08,965, soit
**27,0 secondes plus tard**. Cet argument ne dépend d'aucune hypothèse sur
l'unité de `delai`, ni sur l'efficacité du ping, ni sur ce que fait Apollo.

**Une nuance sur cette liste, qu'il faut écrire.** Le champ `presentes` de la
ligne de refus n'est **pas une relecture prise au moment du refus** :
`montee.rs:346-353` y journalise la liste mémorisée au relevé précédent, et le
libellé de la ligne le laisse pourtant croire. L'écart réel se lit dans le
journal, et il se compte **depuis le début du relevé mémorisé**, seul instant
où son contenu est celui qui sera recopié : le relevé de topologie « après
création 10 » s'ouvre à 10:06:08,964164 et le refus tombe à 10:06:08,964837,
soit **0,673 ms**. (La ligne `sortie virtuelle créée rang=10`, horodatée
10:06:08,964362, est émise *après* que ce relevé était terminé : la prendre
pour repère resserrerait l'écart à 0,5 ms, dans le sens flatteur, en mesurant
autre chose que ce que le champ porte.) À cette échelle la conclusion tient
largement, et la borne des 27 s ci-dessus s'appuie de toute façon sur les
relevés de topologie eux-mêmes, qui sont frais. Mais c'est un libellé de journal
qui promet plus que ce qu'il porte, et il est reporté comme tel au tri des points
reportés.

L'épreuve dédiée (`moniteurs-chien-de-garde.log`, `MULTIFENETRE_VDD_VEILLE=180`,
une sortie créée, **aucun ping de notre part**, relevé à 1 Hz) n'établit rien de
plus que ceci : **rien n'a été retiré en 180 s sur CETTE VM, Apollo pinguant le
même pilote**. Le décompte oscille entre 2 et 3 par paliers, sans jamais
approcher de zéro — ce qui est **exactement** ce que produirait un délai de trois
secondes réarmé chaque seconde par autrui. **Aucune unité n'est exclue, pas même
la seconde.**

`IOCTL_DRIVER_PING` (`0x0022_2220`) est **prouvé accepté, seulement indiqué
agissant**. Le journal porte une lecture de décompte avant et après le ping —
`decompte_avant_ping=Some(2) decompte_apres_ping=Some(3)` — donc le critère
posé (« seul un décompte qui REMONTE prouverait un réarmement ») est rempli, une
fois. Deux réserves qui interdisent d'en faire une preuve :

- les deux lectures sont émises dans **une seule ligne de journal**, à
  10:04:26,546144 : le journal ne permet pas de mesurer l'intervalle qui les
  sépare, et une version antérieure de ce document affirmait à tort un
  encadrement « à la microseconde près » ;
- **le décompte remonte spontanément de 2 à 3 quatre fois pendant l'épreuve, aux
  secondes 57, 107, 109 et 111, sans un seul ping de notre part**. C'est la
  donnée qui chiffre le mot « improbable » : des remontées non provoquées par
  nous surviennent bel et bien, à raison de quatre en 180 s. Une coïncidence
  dans la fenêtre du ping reste peu vraisemblable, mais elle n'est **pas**
  formellement exclue, et ce relevé-là est la seule mesure de sa vraisemblance.

### Ce que la mesure n'isole pas

- La **cause** du refus (borne SudoVDA, IddCx, ou configuration d'adaptateur).
- Si le vivier de 10 est **global au pilote ou par client**. Apollo n'avait
  aucune sortie ouverte pendant la mesure — le partage n'a donc pas été éprouvé.
- **L'unité de `delai`**, et le sort d'un client seul et muet. La seule voie
  d'isolement serait d'arrêter `ApolloService`, écartée : Apollo pilote la
  configuration d'affichage de cette VM, et la mesure du plafond n'en dépend pas.
- **La tenue dans la durée.** Créées puis détruites en **au moins 27 s**
  (30,1 s si l'on compte depuis le premier IOCTL de création).
- **Ce que valent ces dix sorties une fois capturées** — c'est la mesure ③, et
  elle n'a porté que sur **une** d'entre elles.

---

## Mesure ② — le plafond d'encodage sur périphériques D3D11 séparés : **8, inchangé**

### Le relevé

Deux journaux, deux modes, un seul chiffre.

`nvenc-partage-temoin.log` (témoin, un périphérique D3D11 partagé, une
duplication de sortie ouverte) :

```
11:51:50.801  encodeur créé rang=1 mode="partage"
   …
11:51:51.015  encodeur créé rang=8 mode="partage"
11:51:51.044  plafond d'encodeurs atteint — création du suivant refusée
              plafond=8 mode="partage"
              causes=configuration du type de sortie de l'encodeur H.264
                     (transform matériel) : Le type d'entrée n'est pas pris en
                     charge pour le périphérique D3D. (0xC00D6D76)
11:51:51.045  relâchement des encodeurs et des périphériques encodeurs=8 peripheriques=0
11:51:51.066  relâchement terminé — le processus a survécu à la destruction
```

`nvenc-separe.log` (un périphérique D3D11 neuf par tentative, aucune
duplication) :

```
11:52:06.628  encodeur créé rang=8 mode="separe"
11:52:06.678  plafond d'encodeurs atteint — création du suivant refusée
              plafond=8 mode="separe"   causes=(la même chaîne, mot pour mot)
11:52:06.678  relâchement des encodeurs et des périphériques encodeurs=8 peripheriques=9
```

`peripheriques=9` : neuf périphériques D3D11 **distincts et vivants** ont bien
été créés, un par tentative, y compris pour celle qui est refusée. Le mode a
fait ce qu'il prétend faire.

Configuration unique : 1280×720, 60 i/s, 8 Mb/s, profil Baseline, CBR, entrée
NV12 GPU.

### Ce que le relevé autorise à conclure

**La moitié négative, et elle seule : le partage du périphérique D3D11 n'était
pas la contrainte.** Donner à chaque encodeur son propre périphérique ne fait
gagner aucune session. Le témoin `partage` redonne bien **8**, le chiffre de la
sonde précédente : la comparaison est valide.

Deux conséquences produit qui en découlent directement :

1. **Huit fenêtres encodées de front, au plus**, sur cette RTX 4070, ce pilote
   et cette configuration. Le chantier D ne gagne aucune fenêtre en séparant les
   périphériques.
2. **Le coût de la séparation n'a pas à être payé.** L'issue inverse aurait
   obligé à partager des textures entre le périphérique de capture et ceux des
   encodeurs. Cette dette n'a plus lieu d'être ouverte : capture et encodeurs
   peuvent rester sur un périphérique unique, comme aujourd'hui.

### La correction d'attribution — ce n'est pas ce que le plan désignait

La sonde précédente écrivait que la 9ᵉ construction « échoue à la **liaison du
type d'entrée** », et hésitait entre deux `SetInputType` : celui de la MFT
H.264, et celui du Video Processor MFT du convertisseur de couleur. Le plan de
ce chantier reprenait cette formulation et posait la prémisse que **le composant
fautif se nommerait de lui-même**, les deux appels ayant reçu un `.context()`
distinct à la fin de la sonde.

**La prémisse était fausse, et l'attribution avec elle.** Comment c'est établi —
par élimination, sur le relevé :

- la **première** exécution de ce chantier tourne avec ces deux `.context()`
  déjà en place, et rend pourtant une chaîne de causes **nue**, réduite au seul
  HRESULT : `causes=Le type d'entrée n'est pas pris en charge pour le
  périphérique D3D. (0xC00D6D76)` — `nvenc-partage-temoin-sans-contexte.log` ;
- si l'appel fautif avait été l'un des deux `SetInputType`, son contexte serait
  apparu. L'appel fautif était donc **un `?` nu, sans contexte** — ni l'un ni
  l'autre des deux appels désignés ;
- **dix** appels du chemin de construction ont alors reçu un contexte distinct
  (`agent/src/encode.rs`) : les deux `SetOutputType`, `ResetDevice`, les deux
  `ActivateObject`, cinq `ProcessMessage`. La seconde exécution du témoin nomme
  le composant sans ambiguïté — c'est la **seule** différence entre les deux
  journaux `partage`, qui rendent par ailleurs le même 8 au même rang.

**Pourquoi l'attribution antérieure s'était égarée** : le libellé Windows de
`MF_E_UNSUPPORTED_D3D_TYPE` parle du type d'**entrée** alors que l'appel refusé
règle le type de **sortie**. Le texte du HRESULT désignait un appel qui n'était
pas le sien. **Ne jamais se fier au texte d'un HRESULT pour désigner un appel** —
c'est la leçon transférable de cette correction, et elle a coûté dix
annotations pour être obtenue.

Ce que le refus **n'est pas** : le 9ᵉ transform matériel **s'instancie sans
peine** — `encodeur matériel retenu encodeur=NVIDIA H.264 Encoder MFT` paraît
**neuf** fois dans `nvenc-separe.log`, dont une pour la tentative refusée. Ce
n'est donc ni l'énumération ni l'activation de la MFT qui plafonne.

### Ce que la mesure n'isole pas

- **La réserve à deux variables confondues.** Entre `partage` et `separe`, deux
  choses changent : le nombre de périphériques D3D11 (1 contre 9) **et** la
  présence d'une duplication de sortie DXGI (`separe` n'en ouvre aucune — zéro
  ligne `duplication de sortie établie`, contre une en `partage`). L'énoncé « le
  partage n'était pas la contrainte » est donc tiré d'une comparaison à deux
  facteurs. La direction de la conclusion n'en est pas menacée — les deux modes
  rendent le même 8, et il faudrait une coïncidence pour que deux effets se
  compensent exactement — **mais le témoin propre, un mode `separe` ouvrant une
  duplication, n'a pas été exercé.** Cette réserve doit accompagner
  l'affirmation causale partout où elle est reprise, pas la suivre trois
  paragraphes plus loin.
- **La couche qui impose le plafond n'est pas identifiée.** Le refus est observé
  au niveau Media Foundation, rendu par la MFT NVIDIA. Rien ne distingue NVENC,
  le pilote NVIDIA, Media Foundation, ou une limitation de la carte en machine
  virtuelle.
- **8 n'est pas prouvé être « la limite de sessions NVENC de cette carte ».** Ce
  qui est mesuré, c'est le rang auquel `SetOutputType` refuse **pour cette
  configuration précise**. Aucune variation de résolution, de débit ou de format
  n'a été éprouvée.
- **Aucune image n'a été encodée.** La mesure porte sur la **création**. Que 8
  encodeurs se construisent ne prouve pas qu'ils tiennent la cadence ensemble.
  > ✅ **Note postérieure (31/07/2026)** : la seconde phrase est **répondue par
  > ailleurs** — 8 encodeurs alimentés ensemble tiennent 90,1 i/s par fenêtre
  > (`2026-07-31-duplications-paralleles-resultats.md` §3.2). Mais sur **huit
  > périphériques D3D11 distincts**, pas sur le périphérique unique partagé de
  > CETTE mesure : la première phrase, elle, reste exacte pour la mesure ②.
- **Que détruire un encodeur libère la place est une conjecture.** Aucune des
  trois exécutions n'exerce de destruction en cours de route. La mesure qui
  trancherait est courte : créer 8, en détruire un, tenter un 9ᵉ. Variante
  utile : cesser d'**alimenter** un encodeur sans le détruire, et tenter un 9ᵉ —
  c'est ce que ferait une mise en sommeil naïve.
- **Un seul GPU, une seule VM.** Le mode `partage` a tourné deux fois (avant et
  après instrumentation) et redonné 8 les deux fois ; `separe` une seule fois.
  La variabilité n'est pas caractérisée.
- **`D3D_DRIVER_TYPE_HARDWARE` ne garantit pas formellement l'adaptateur** en
  mode `separe` : que l'encodeur retenu soit `NVIDIA H.264 Encoder MFT` aux neuf
  tentatives est un fort indice, mais l'adaptateur n'est pas journalisé
  nommément.

---

## Mesure ③ — la capture réelle sur une sortie virtuelle : l'hypothèse fondatrice tient

*La sonde précédente recommandait la voie « un moniteur virtuel par fenêtre » en
avertissant que sa correction d'image était « argumentée par construction,
jamais mesurée » et en interdisant de la porter au crédit de la voie comme un
acquis. Cette mesure la porte enfin — pour partie.*

### La question

Windows compose-t-il réellement des fenêtres sur un moniteur **sans écran
physique branché**, et Desktop Duplication en rend-il autre chose que du noir ?

Rien ne le garantissait *a priori* : un compositeur pourrait légitimement ne rien
peindre sur une surface que personne ne regarde. **Deux sens du mot « attaché »
sont à distinguer** : la sortie est attachée **au bureau** (`attachee=true`, ce
que DXGI rapporte, et c'est vrai), mais **aucun écran physique** n'y est branché.
C'est le second sens qui faisait l'enjeu.

### Le relevé

`moniteurs-capture.log`, sortie virtuelle `\\.\DISPLAY5` créée par notre code à
1280×720, publiée à x = 2400 :

```
sortie virtuelle retenue pour la capture id=256 nom=\\.\DISPLAY5
  adaptateur=NVIDIA GeForce RTX 4070 index_adaptateur=0 index_sortie=1
  attachee=true x=2400 y=0 largeur_annoncee=1280 hauteur_annoncee=720

coordonnées de fenêtre et de texture
  bureau_x=2400 bureau_y=0 bureau_largeur=1280 bureau_hauteur=720
  texture_largeur=1280 texture_hauteur=720
  facteur_horizontal=1.0 facteur_vertical=1.0

disposition retenue voie="duplication" nombre=2
  places        =[Rect { x: 2400, … }, Rect { x: 3040, … }]
  places_texture=[Rect { x: 0,    … }, Rect { x: 640,  … }]

passe terminée passe="capture" voie="duplication" nombre=2
  cadences=[90.0, 90.0] verdicts_faux=450
  mire0_avant_recouvrement =Verdicts { justes: 450, voisines: 0, noires: 0, inconnues: 0 }
  mire0_apres_recouvrement =Verdicts { justes: 0, voisines: 450, noires: 0, inconnues: 0 }

la sortie virtuelle a survécu au banc — le verdict porte bien sur elle
```

Et `moniteurs-capture-n1-encodage.log`, N=1, passe de capture entière :
`cadences=[90.0]`, `justes: 900, voisines: 0, noires: 0, inconnues: 0`.

### Ce que le relevé autorise à conclure

- **Oui : le pixel jugé porte la bonne mire, 900 fois sur 900, et zéro image
  noire.** Windows compose bel et bien la sortie virtuelle, et Desktop
  Duplication la rend.
- **Sous recouvrement, la sortie virtuelle se comporte exactement comme le
  bureau physique** : 450 verdicts « Voisine », **0 « Noire »** — la mire du
  **dessus**, pas du noir. Ce n'est pas un défaut du moniteur virtuel, c'est la
  duplication de bureau qui ne sait pas défaire un recouvrement. **Cela ne
  condamne pas la voie 2**, dont la promesse est *une* fenêtre par moniteur : ce
  banc a délibérément posé **deux** fenêtres sur **un seul** moniteur virtuel,
  c'est-à-dire l'arrangement que la voie 2 exclut par construction.
- **La distinction « Voisine » contre « Noire » est tout l'intérêt de cette
  mesure**, et elle a exigé d'ajouter un compteur au banc : celui-ci ne comptait
  que des « verdicts faux » en bloc, et seulement **après** recouvrement. Une
  image noire et une image portant la mire du dessus tombaient dans le même
  compteur — deux résultats opposés, l'un condamnant la voie 2, l'autre la
  laissant intacte. La première ronde, conservée
  (`moniteurs-capture-ronde1.log`), n'affichait que `verdicts_faux=450` :
  indiscernable d'une sortie qui n'aurait rendu que du noir.
- **Le décalage d'origine est load-bearing, l'échelle ne l'était pas ici.** La
  sortie est publiée à x = 2400 : sans la conversion vers coordonnées de texture,
  le recadrage serait tombé à x = 2400 dans une texture large de 1280,
  **entièrement hors bornes**. Le piège de conversion existe donc bien ; il s'est
  manifesté par le **décalage**, pas par l'**échelle**.
- **Le contrôle de survie n'est pas décoratif** : `la sortie virtuelle a survécu
  au banc` — le banc tourne trente secondes sans pinguer le chien de garde. Si
  la sortie avait été retirée en route, la capture aurait rendu du vide et l'on
  aurait imputé à Windows un défaut du protocole de mesure.

### Le facteur d'échelle — une borne à tenir étroite

Le facteur vaut **1,0**, `DesktopCoordinates` et dimensions de mode coïncidant.
**Écrire « le piège DPI ne se présente pas sur cette VM » serait faux** : il s'y
est présenté, ailleurs — le relevé Apollo de la sonde précédente donnait
3413×960 contre 5120×1440 sur cette même machine.

> ⚠️ **Ce relevé Apollo n'a pas de journal joint** — la sonde précédente le
> signalait déjà comme son seul chiffre sans pièce, et sa session n'est pas
> reproductible sans le propriétaire du poste. C'est le **seul** contre-exemple
> dont on dispose ici, et il est donc à la fois nécessaire à la borne ci-dessous
> et moins bien étayé que tout le reste de ce document.

Les deux mesures sont commensurables et ne se contredisent pas : elles portent
sur **deux sorties différentes**. L'énoncé juste est donc : *le piège ne se
présente pas sur une sortie créée par CE pilote à 1280×720*. Rien n'est dit
d'une sortie créée à 5120×1440, résolution à laquelle Windows applique
volontiers 150 %. Le chemin de conversion reste nécessaire ; il n'a simplement
pas eu de travail d'échelle à faire ici — et il aurait tort d'être retiré au
motif que ce chantier n'a mesuré que des facteurs unitaires.

### Le défaut ouvert : la passe d'encodage tue le processus

> ✅ **Ce défaut a depuis été diagnostiqué et corrigé (31/07/2026).** Toute cette
> section est conservée telle qu'écrite ; sa conclusion porte la note détaillée,
> et l'un de ses énoncés — le caractère déterministe suggéré par « les deux
> exécutions » — est **faux**. Lire la note avant de reprendre quoi que ce soit
> d'ici.

**Bornage exact, à ne pas élargir.** À N=1 il n'y a pas de recouvrement, la
porte de correction laisse passer, et la passe d'**encodage** s'exécute — pour
la première fois de l'histoire de ce banc sur la voie `duplication`, la porte
l'ayant toujours coupée avant.

Elle tue le processus **à la sortie de la boucle, pas pendant** :

| | N=1 sur sortie virtuelle | Témoin, bureau physique |
| --- | --- | --- |
| Fin de la passe de capture | `11:05:03,590` | `11:12:20,882` |
| **Dernière** ligne `banc en cours` | `11:05:13,717` (`unites=[450]`) | `11:12:30,970` (`unites=[373]`) |
| Écart depuis le début de la passe | **+10,00 s** | **+10,00 s** |
| `passe terminée` | *jamais* | *jamais* |

Le journal périodique tombe à début + 1 s, + 2 s, … Les deux exécutions écrivent
leur **dixième et dernière** ligne à début + 10,00 s, c'est-à-dire au moment
exact où la condition de boucle cesse d'être vraie. **La boucle a donc tourné
entière** ; la mort survient entre sa sortie et l'écriture du bilan. Ce que le
processus traverse dans cet intervalle est court et nommable : la **destruction**
du `Vec<H264Encoder>` local à la passe, et celle de la duplication. Dire « pendant
la passe » aurait orienté le diagnostic vers `submit` / `poll_output` ; le relevé
désigne la **libération**.

Ce que les témoins bornent :

- **Ce n'est pas imputable à la sortie virtuelle.** Même banc, même voie, N=1,
  sur le **bureau physique**, sans aucune sortie virtuelle : même mort au même
  endroit (`moniteurs-capture-temoin-bureau-physique.log`).
- **Ce n'est pas l'encodeur seul.** `printwindow-n4.log` et `printwindow-n8.log`
  portent tous deux leur ligne `passe terminée passe="capture+encodage"` : sur la
  voie `printwindow` la passe va à son terme et le processus survit.
- **Un encodage doit probablement avoir réellement eu lieu.** La mesure ② forme
  le couple « duplication + encodeurs » en mode `partage` et **survit** à la
  destruction de ses 8 encodeurs (`relâchement terminé — le processus a survécu à
  la destruction`) — mais sans avoir jamais soumis la moindre image. La
  différence de nature est trop grande pour conclure, et elle resserre l'énoncé
  plutôt qu'elle ne le contredit.

**Le défaut n'est donc pas diagnostiqué, seulement localisé** : à la sortie de
boucle, sur la voie `duplication`, quelle que soit la sortie capturée, après un
encodage réel. Quel appel exactement le provoque — libération des encodeurs, de
la duplication, ou leur ordre relatif — reste entièrement ouvert.

> ✅ **Note postérieure (31/07/2026) — cette conclusion est dépassée : le défaut
> A depuis été diagnostiqué ET corrigé.**
> Voir `2026-07-31-duplications-paralleles-resultats.md` §7. En trois points, et
> chacun corrige quelque chose que la section ci-dessus laisse croire :
>
> - **il est intermittent, pas déterministe** — 2 plantages sur 6 exécutions du
>   cas comparable ; le bornage ci-dessus dit « les deux exécutions » sans dire
>   combien avaient passé, et un rapport antérieur avait enchaîné **quatre
>   exécutions propres sur un binaire non corrigé** ;
> - **l'appel qui provoque la faute est désigné**, avec sa pile symbolisée deux
>   fois identique : la MFT NVIDIA a un élément de travail encore en vol quand on
>   relâche l'encodeur, et il entre dans un verrou qui n'existe plus
>   (`RtlEnterCriticalSection`, chemin contendu, `DebugInfo` nul, **sur un fil de
>   pool et non sur le fil principal**). La question « libération des encodeurs,
>   de la duplication, ou leur ordre relatif » était donc mal posée : aucun
>   ordonnancement du fil principal ne pouvait la trancher ;
> - **`MFShutdown`, qu'un premier diagnostic a accusé, a été réfuté par la
>   mesure** — retiré entièrement du chemin, la faute revient (1 sur 5).
>
> Correctif : une file de travail Media Foundation **sérialisée par encodeur**
> imposée à la MFT, avec dépôt d'une sentinelle avant tout relâchement —
> **0 récidive sur 20 exécutions contre 2 sur 6**, ce qui **n'est pas une preuve
> d'absence**. Deux risques restent ouverts et assumés (§7.5 du document cité).

**Conséquence opérationnelle** : la garde ne court pas, donc la sortie virtuelle
**survit au processus**. C'est ce qui a produit l'orpheline réelle du chantier,
constatée depuis un processus neuf (`topologie relevée moment="avant purge"
nombre=2`) et retirée par la purge (`retirees=1 avant=2 apres=1`,
`moniteurs-capture-purge-orpheline.log`).

> ✅ **Note postérieure (31/07/2026)** : au passé — le plantage qui empêchait la
> garde de courir est corrigé (§7 de
> `2026-07-31-duplications-paralleles-resultats.md`). Ce qui **reste vrai en
> propre**, et pourquoi la purge n'est pas devenue inutile : une sortie
> virtuelle survit toujours à un processus qui meurt, quelle qu'en soit la
> cause — la garde `Sorties` est un RAII, et aucun RAII ne court sur un
> `0xc0000005`.

### Ce que la mesure n'isole pas

- **L'arrangement que la voie 2 recommande réellement.** Ce banc a posé N
  fenêtres sur **UNE** sortie virtuelle. La voie 2 promet **une** fenêtre par
  sortie, donc **N sorties et N duplications DXGI ouvertes de front**. Rien ici
  ne l'établit — et la sonde précédente a documenté que **DXGI n'autorise qu'une
  seule duplication par sortie** : la question porte donc sur N duplications sur
  N sorties distinctes, un montage jamais exercé. **C'est la mesure suivante, et
  elle est bloquante pour dimensionner la voie recommandée.**
  > ✅ **Note postérieure (31/07/2026)** : ce montage a depuis été exercé et la
  > voie est reçue — 90,1 i/s par fenêtre en capture+encodage à N=8, zéro
  > verdict faux (`2026-07-31-duplications-paralleles-resultats.md`). Une
  > exécution par rang, donc aucun taux ; rien au-delà de 8 sorties.
  > ⚠️ **Portée resserrée le 1ᵉʳ août 2026 (sous-bloc D1)** : le banc créait ses
  > N sorties **avant** d'ouvrir la moindre duplication ; ce n'est pas l'ordre du
  > produit, et cet ordre-là échoue — la création d'une sortie pendant que
  > d'autres capturent tue toutes les sessions (`0x887A0026`).
  > `2026-08-01-multifenetres-tranche-verticale-resultats.md`.
  > ✅ **Cet ordre-là passe depuis le sous-bloc D2** (1ᵉʳ août 2026,
  > `2026-08-01-multifenetres-arrangement-dynamique-resultats.md`) : le mutex est
  > toujours abandonné, mais la reprise l'encaisse et **aucune session n'en
  > meurt**. ⚠️ En **processus distincts**, la **5ᵉ** duplication est en revanche
  > refusée (`0x887A0022`) — plafond observé à **4**, dont **la couche n'est pas
  > identifiée**. **Ne pas transposer le 8 au multi-processus.**
- **La cadence de 90,0 i/s n'est pas expliquée.** Identique à N=1 et N=2,
  parfaitement régulière, alors que la sortie a été créée à 60 Hz. Le témoin qui
  vaut n'est pas celui de la sonde précédente (autre jour, autre code) mais celui
  de la **même heure, même banc, même voie, même N=1** : `cadences=[79.8]` sur le
  bureau physique contre `[90.0]` sur la sortie virtuelle — la sortie virtuelle
  est la plus rapide des deux, de ~13 %. D'où vient ce plateau n'est pas établi.
- **Le contenu jugé est UN pixel, au centre de la région.** Une capture juste au
  centre et fausse aux bords passerait la porte. Protocole du banc depuis
  l'origine, inchangé.
- **Aucune mesure d'encodage sur sortie virtuelle n'a survécu.** Les 450 unités
  H.264 du run N=1 ont bien été produites depuis des textures capturées sur
  `\\.\DISPLAY5` — ce qui *indique* que NVENC accepte de telles textures — mais
  la passe n'est pas allée à son terme, et aucun chiffre d'encodage n'est publié.
  > ✅ **Note postérieure (31/07/2026) — RÉFUTÉ, et c'était le point parqué
  > « pour la vague finale »** : le défaut qui coupait ces passes est corrigé, et
  > des chiffres d'encodage sur sortie virtuelle sont **publiés aux quatre
  > rangs** — 450 unités H.264 par encodeur à N = 1, 2, 4 et 8, et 90,1 i/s par
  > fenêtre en capture+encodage
  > (`2026-07-31-duplications-paralleles-resultats.md` §3.2 ;
  > `journaux-duplications-paralleles/paralleles-n8.log:146`). Ce qui reste vrai
  > de l'énoncé d'origine : les unités sont **comptées**, jamais décodées ni
  > regardées.

---

## Mesure ④ — `PrintWindow` à N=4 et N=8 : le repli ne tient pas l'échelle

### Le relevé

`printwindow-n4.log`, tuiles de 1200×540 couvrant le bureau entier :

```
passe terminée passe="capture"           voie="printwindow" nombre=4
  cadences=[17.6, 17.6, 17.6, 17.6] unites=[0,0,0,0]        verdicts_faux=0
passe terminée passe="capture+encodage"  voie="printwindow" nombre=4
  cadences=[17.2, 17.2, 17.2, 17.2] unites=[86,86,86,86]    verdicts_faux=0
```

`printwindow-n8.log`, tuiles de 800×360 (grille 3×3 dont une case inutilisée) :

```
passe terminée passe="capture"           voie="printwindow" nombre=8
  cadences=[8.8 ×8] unites=[0 ×8]   verdicts_faux=0
passe terminée passe="capture+encodage"  voie="printwindow" nombre=8
  cadences=[8.8 ×8] unites=[44 ×8]  verdicts_faux=0
```

`verdicts_faux = 0` aux deux rangs : **la passe d'encodage n'a pas été sautée**,
et le pipeline complet a donc bien tourné — contrairement à N=2 chez la sonde
précédente, où cette passe n'avait pas été conclue.

Conversion en débit de pixels — cadence par fenêtre × surface de la place × N.
**Les quatre rangs ont un journal**, ceux de N=1 et N=2 venant de la sonde
précédente (`journaux-sonde-multifenetre/` ; les `banc-*.log` y sont en UTF-8 et
`grep`-ables sans conversion — mais **pas** les autres fichiers du répertoire,
dont `dxgi.log`, `wgc.log`, `replis.log` et `nvenc.log`, en UTF-16LE) :

| N | i/s, passe `capture` | i/s, passe `capture+encodage` | Place | **MP/s** (passe `capture`) | Journal |
| --- | --- | --- | --- | --- | --- |
| 1 | 45,0 | 44,9 (224 unités) | 2400×1080 | **116,64** | `banc-printwindow-1.log` |
| 2 | 29,1 | *non conclue* | 1200×1080 | **75,43** | `banc-printwindow-2.log` |
| 4 | 17,6 | 17,2 (86 unités ×4) | 1200×540 | **45,62** | `printwindow-n4.log` |
| 8 | 8,8 | 8,8 (44 unités ×8) | 800×360 | **20,28** | `printwindow-n8.log` |
| — | — | — | — | 208–258 | voie `duplication`, sonde précédente : `journaux-sonde-multifenetre/banc-duplication-{1,2,4,8}.log` |

**Le MP/s est calculé sur la passe `capture`**, seule passe conclue aux quatre
rangs — la passe `capture+encodage` n'a pas été menée à son terme à N=2, et une
série qui mêlerait les deux ne serait pas comparable. Sur la passe
`capture+encodage`, là où elle est conclue, les chiffres sont **44,58 MP/s à
N=4** et **20,28 à N=8** : ce sont ceux qu'avait publiés la mesure ④, et ils ne
changent ni la forme de la courbe ni la conclusion.

### Ce que le relevé autorise à conclure

- **La cadence par fenêtre décroît de façon monotone sur quatre rangs** :
  45,0 → 29,1 → 17,6 → 8,8 (passe `capture`). À N=8 le repli délivre moins de
  9 images par seconde et par fenêtre.
- **Le débit de pixels décroît lui aussi, sur les quatre rangs et sans palier** :
  **116,64 → 75,43 → 45,62 → 20,28 MP/s**, soit une division par 5,8 entre N=1
  et N=8. **C'est le fait qui distingue cette voie de la voie `duplication`**,
  dont le débit de pixels restait quasi constant (208–258 MP/s) sur la même
  plage : chez `duplication`, partager une acquisition entre N recadrages d'aire
  totale fixe ne coûtait rien de plus ; chez `printwindow`, chaque fenêtre coûte
  sa propre traversée du chemin CPU. **Aucun signe de palier.**
  *(La surface totale des places est constante à 2 592 000 px aux rangs 1, 2 et
  4 — le bureau entier — et de 2 304 000 px à N=8, la grille 3×3 laissant une
  case inutilisée. Le débit de pixels et la cadence par fenêtre chutent donc
  ensemble, ce qui n'aurait pas été le cas si l'aire totale avait varié.)*
  *(Et la comparaison avec `duplication` est **à protocole identique**, pas
  seulement à aire comparable : ses 208–258 MP/s sortent **eux aussi de la passe
  `capture`**, ses passes d'encodage ayant été coupées par `verdicts_faux > 0`
  aux rangs 2, 4 et 8 — `banc-duplication-{2,4,8}.log` portent le verdict
  « ÉLIMINÉE sous recouvrement — la passe d'encodage est sautée ». Les deux
  séries mises en regard mesurent donc la même chose, ce qui est un argument
  plus fort que celui d'abord publié ici.)*

### Ce que le relevé n'autorise pas

- **Le goulot n'est pas isolé** par ce banc : rien ne départage le rendu
  `PrintWindow` lui-même, l'assemblage des tuiles, et l'encodage.
- **Rien au-delà de N=8**, plafond du banc (`mire::MIRES_MAX`).
- **Le plancher de l'implémentation reste celui que la sonde précédente avait
  relevé et non corrigé** : la voie alloue et détruit à chaque image un DC
  compatible, un bitmap compatible et un tampon remis à zéro. Les chiffres
  ci-dessus sont donc un **plancher de cette implémentation**, pas du chemin CPU
  en général — le vrai chiffre est meilleur. Cela ne change pas la **forme** de
  la courbe, qui est le résultat utile.
- **Le banc a changé entre ces mesures et la fin du chantier.** La lecture de
  pixel du banc, autrefois conditionnée au recouvrement, court désormais sur
  toute la passe : les cadences ci-dessus viennent d'une version qui relisait
  **deux fois moins**. Le risque est **borné mais non nul** :
  `moniteurs-capture-ronde1.log` (ancien décompte) et `moniteurs-capture.log`
  (nouveau) portent tous deux `cadences=[90.0, 90.0]` sur `duplication` à N=2 —
  à ~11 ms par tour la relecture ajoutée ne coûte rien de mesurable, et à 57 ms
  par tour (`printwindow` à N=4) la marge est plus large encore. Mais « faible »
  n'est pas « nul et démontré » : **aucune mesure `printwindow` n'a été rejouée
  sur le banc d'aujourd'hui.** Toute reprise de ces chiffres doit porter cette
  note ou les rejouer.

---

## Le canal de contrôle du pilote — ce qui rend tout le reste possible

*Sans cette pièce, la mesure ① aurait été impossible : la sonde précédente ne
savait faire paraître une sortie virtuelle qu'en demandant au propriétaire du
poste de lancer une session Apollo, et n'a jamais pu en obtenir deux.*

**Forme retenue : canal IOCTL.** Document complet :
`docs/superpowers/plans/journaux-mesures-prealables/canal-de-controle.md`.

L'hypothèse concurrente — charger `SudoVDA.dll` et appeler une fonction
exportée — est **écartée, pas seulement non confirmée** : cette DLL n'exporte que
`FxDriverEntryUm`, le point d'entrée générique UMDF injecté par WDF.

Ce qui est **confirmé par octets** dans le binaire installé sur cette VM
(md5 `200ec71b297ca42469652256c4fa896b`, deux copies identiques) :

| Constante | Statut | Offset |
| --- | --- | --- |
| GUID d'interface SUVDA `{e5bcc234-1e0c-418a-a0d4-ef8b7501414d}` | **fait local** | 53656 |
| `IOCTL_ADD_VIRTUAL_DISPLAY` `0x00222000` | **fait local** | 16316 |
| `IOCTL_GET_WATCHDOG` `0x0022200C` | **fait local** | 16284 |
| `IOCTL_REMOVE_VIRTUAL_DISPLAY`, `SET_RENDER_ADAPTER`, `DRIVER_PING`, `GET_PROTOCOL_VERSION` | lus en amont | — |
| Les sept dispositions de structures | **lues en amont, non confirmées** | — |

L'écart amont/local est de **onze mois** (en-tête du 2024-09-08 contre pilote
`DriverVer 07/14/2025`). Ce qui a ensuite été éprouvé **empiriquement**, par
notre code :

- `IOCTL_GET_PROTOCOL_VERSION` et `IOCTL_GET_WATCHDOG` répondent, avec le nombre
  d'octets attendu — la formule `CTL_CODE` est donc la bonne pour les codes non
  confirmés par octets. Version rendue : `{0, 2, 1, version de test}`,
  identique à la constante amont malgré les onze mois. **Ce relevé n'a pas de
  journal versé** (cité depuis `/media/vm/dev/agent.log`) ; les valeurs de
  watchdog qu'il porte sont en revanche reproduites dans
  `moniteurs-chien-de-garde.log` ;
- `IOCTL_ADD_VIRTUAL_DISPLAY` (56 octets d'entrée) et
  `IOCTL_REMOVE_VIRTUAL_DISPLAY` (16 octets) sont éprouvés **dix fois chacun**
  par la mesure ① (`moniteurs-montee-en-n.log`) ;
- `IOCTL_DRIVER_PING` est **accepté** (`moniteurs-chien-de-garde.log`).

**Ce que cela n'établit pas** : l'**ordre des champs** des structures. Un test
d'aller-retour ne peut pas le trancher pour la structure de veille (`delai` et
`decompte` valent tous deux 3) et ne le tranche que partiellement pour la
version de protocole. Un pilote ayant gagné un champ depuis l'en-tête amont
réussirait l'appel tout en rendant un compte d'octets différent — c'est
précisément pourquoi chaque appelant compare le compte rendu à la taille
supposée, et pourquoi un bloc d'assertions de compilation fige les six tailles.

### La purge autonome

Une sortie virtuelle **survit au processus qui l'a créée**. Un agent qui plante
à la cinquième création laisse cinq moniteurs derrière lui — et c'est arrivé
pour de bon pendant ce chantier (§Mesure ③).

**Stratégie** : régénérer la suite **déterministe** de GUID par la même fonction
que la création — fidèle par construction, pas par ressemblance — et tenter un
retrait sur `1..=16`. Un refus est l'issue **normale**, pas une erreur.

**Ce qu'elle ne rattrape pas**, à dire explicitement : les sorties d'un autre
logiciel (Apollo en attribue sur le même pilote), celles d'un gabarit de GUID
antérieur, un compteur ayant dépassé 16, et deux instances du pilote ouvertes en
parallèle (leurs compteurs repartant tous deux à 1, elles attribueraient les
mêmes GUID à des sorties distinctes).

---

## Recommandation pour le chantier D

*La sonde précédente recommandait la voie 2 (« un moniteur virtuel par fenêtre »)
comme cible et la voie 4 (`PrintWindow`) comme repli, **en conditionnant tout à
deux mesures**. Les deux sont prises. Voici où en est l'arbitrage.*

### La cible : la voie 2 est confirmée, et son prix est connu

**Je maintiens la voie 2 comme cible, et l'arbitrage est désormais fondé sur des
mesures et non sur une construction.** Trois choses ont changé :

1. **Son hypothèse fondatrice est vérifiée.** Windows compose bien une fenêtre
   sur un moniteur sans écran physique, et la duplication en rend l'image
   exacte — 900/900, zéro noire (mesure ③). C'était la faiblesse que la sonde
   signalait explicitement (« ne pas la porter au crédit de la voie comme un
   acquis »). Elle est levée.
2. **Son plafond est mesuré : 10, pour une cible de 8** (mesure ①). La sonde
   avertissait que « si le plafond est 1, tout l'arbitrage bascule vers la voie 4
   et le chantier D change de nature ». Le plafond n'est pas 1. **La marge est
   toutefois de 2, et le vivier est peut-être partagé avec Apollo** : c'est un
   fait à porter à la spécification, pas une alerte.
3. **Sa dépendance à Apollo est levée.** Le produit commande le pilote
   lui-même, crée, détruit et purge. La dépendance résiduelle est au pilote
   SudoVDA — un pilote d'affichage indirect tiers, à installer et à maintenir.

**Le prix de cette voie**, révisé à la lumière de ce chantier :

- **Un pilote d'affichage indirect tiers**, dont le contrat de programmation
  n'est confirmé qu'en partie (§Le canal de contrôle) et dont le chien de garde
  a une **unité inconnue**. Une exploitation durable devra pinguer — et la
  question « que se passe-t-il pour un client seul et muet ? » n'a pas de
  réponse mesurée.
- **Une topologie d'affichage à gérer**, c'est-à-dire un état **global du
  système d'exploitation** : créer et détruire des moniteurs au rythme des
  fenêtres, avec les reconfigurations que Windows en tire, et un retour à l'état
  initial à garantir en cas de plantage. La purge autonome couvre ce dernier
  point et **elle a déjà servi en conditions réelles** ; ses limites sont
  énumérées plus haut.
- **Un plafond de 8 encodeurs**, dont la couche fautive n'est pas identifiée et
  qu'aucune séparation de périphériques ne relève. Suspendre l'encodage des
  fenêtres masquées passe donc du statut d'optimisation souhaitable à celui de
  **condition de viabilité** au-delà de 8 fenêtres — et le mécanisme reste à
  choisir, puisque rien ne dit encore que détruire un encodeur libère la place.
- **Une mesure encore due avant de dimensionner**, et une seule : N duplications
  DXGI de front sur N sorties virtuelles (voir ci-dessous).
  > ✅ **Prise le 31 juillet 2026, et la voie est reçue** — 90,1 i/s par fenêtre
  > en capture+encodage à N=8, zéro verdict faux, aire totale 7,37 Mpx :
  > `2026-07-31-duplications-paralleles-resultats.md`. Le défaut de libération
  > ⚠️ **Portée resserrée le 1ᵉʳ août 2026 (sous-bloc D1)** : le banc créait ses
  > N sorties **avant** d'ouvrir la moindre duplication ; ce n'est pas l'ordre du
  > produit, et cet ordre-là échoue — la création d'une sortie pendant que
  > d'autres capturent tue toutes les sessions (`0x887A0026`).
  > `2026-08-01-multifenetres-tranche-verticale-resultats.md`.
  > ✅ **Cet ordre-là passe depuis le sous-bloc D2** (1ᵉʳ août 2026,
  > `2026-08-01-multifenetres-arrangement-dynamique-resultats.md`) : le mutex est
  > toujours abandonné, mais la reprise l'encaisse et **aucune session n'en
  > meurt**. ⚠️ En **processus distincts**, la **5ᵉ** duplication est en revanche
  > refusée (`0x887A0022`) — plafond observé à **4**, dont **la couche n'est pas
  > identifiée**. **Ne pas transposer le 8 au multi-processus.**
  > des encodeurs (point suivant de la liste ci-dessous) y est diagnostiqué et
  > corrigé — il était **intermittent**, non déterministe, et `MFShutdown` n'en
  > était pas la cause.

### Le repli : `PrintWindow` recule de « repli du produit » à « repli des petites configurations »

La sonde le retenait pour les fenêtres non-jeu, sur la foi de 29,1 i/s par
fenêtre à N=2, en signalant que N=4 et N=8 n'étaient pas mesurés et en
interdisant d'extrapoler. **La mesure ④ dit que l'extrapolation aurait été
fausse** : 17,6 i/s à N=4 et **8,8 i/s à N=8**. Et une fois les quatre rangs
recollés, l'argument ne repose plus sur deux points mais sur une série
complète : **116,64 → 75,43 → 45,62 → 20,28 MP/s**, décroissance monotone sans
palier, à surface totale pourtant constante sur les trois premiers rangs.

**Ce que cela fait au statut du repli** : à la cible de 8 fenêtres du chantier D,
`PrintWindow` délivre moins de 9 images par seconde et par fenêtre. C'est en deçà
de ce qu'on peut présenter comme un bureau distant, y compris pour un traitement
de texte. Le repli **ne tient pas à l'échelle visée**.

Il n'est pas pour autant à jeter, et il faut être juste avec le relevé :

- **il reste la seule voie dont la correction d'image sous recouvrement soit
  mesurée directement** (résultat contre-intuitif de la sonde, obtenu sur une
  mire peinte en D3D11) ;
- **à N=2 il reste parfaitement utilisable** (29,1 i/s), et à N=4 il reste
  discutable (17,6 i/s, 17,2 avec l'encodage) pour une fenêtre statique ;
- **le chiffre mesuré est un plancher de l'implémentation**, qui alloue et
  détruit ses ressources GDI à chaque image. Hisser ces allocations hors de la
  boucle est un travail d'une heure, jamais fait, et il précéderait toute
  décision définitive.

**Énoncé retenu** : `PrintWindow` est un repli **pour deux à quatre fenêtres**,
et pour les cas où la voie 2 ne peut pas s'appliquer (plafond de sorties atteint,
pilote absent, fenêtre qu'on ne veut pas isoler sur un moniteur). Ce n'est plus
un repli général du chantier D. Et l'écart entre son coût mesuré et son coût
possible doit être fermé avant qu'on le retienne ou l'écarte pour de bon.

### Ce qui reste à lever, par ordre d'utilité

1. **N duplications DXGI de front sur N sorties virtuelles.** *La seule mesure
   encore bloquante pour dimensionner la voie 2.* Tout est en place : la
   création de N sorties est éprouvée (mesure ①), la capture d'une sortie
   virtuelle l'est aussi (mesure ③) ; il reste à les composer. La règle « une
   seule duplication ouverte par sortie » rend la question réelle et non
   formelle. Coût : une demi-journée avec le banc existant.
   > ✅ **Note postérieure (31/07/2026)** : **levée, voie reçue à N=8** —
   > `2026-07-31-duplications-paralleles-resultats.md`.
   > ⚠️ **Portée resserrée le 1ᵉʳ août 2026 (sous-bloc D1)** : le banc créait ses
   > N sorties **avant** d'ouvrir la moindre duplication ; ce n'est pas l'ordre du
   > produit, et cet ordre-là échoue — la création d'une sortie pendant que
   > d'autres capturent tue toutes les sessions (`0x887A0026`).
   > `2026-08-01-multifenetres-tranche-verticale-resultats.md`.
   > ✅ **Cet ordre-là passe depuis le sous-bloc D2** (1ᵉʳ août 2026,
   > `2026-08-01-multifenetres-arrangement-dynamique-resultats.md`) : le mutex est
   > toujours abandonné, mais la reprise l'encaisse et **aucune session n'en
   > meurt**. ⚠️ En **processus distincts**, la **5ᵉ** duplication est en revanche
   > refusée (`0x887A0022`) — plafond observé à **4**, dont **la couche n'est pas
   > identifiée**. **Ne pas transposer le 8 au multi-processus.**
2. **Le plantage de la passe d'encodage sur la voie `duplication`.** Localisé,
   non diagnostiqué, et il **laisse des sorties orphelines** — donc il gêne les
   mesures autant qu'il menacerait le produit. À reprendre avec le bornage du
   §Mesure ③ comme point de départ : à la sortie de boucle, sur la libération,
   après un encodage réel.
   > ✅ **Note postérieure (31/07/2026)** : **diagnostiqué et corrigé** (même
   > document, §7). Il était **intermittent** (2 plantages sur 6 exécutions) et
   > non déterministe, et `MFShutdown` — que le diagnostic a d'abord accusé — a
   > été **réfuté par la mesure**.
3. **La séquence « créer 8 → en détruire 1 → tenter un 9ᵉ ».** Une heure, et
   elle décide du mécanisme de mise en sommeil des fenêtres masquées. Variante
   au passage : cesser d'alimenter un encodeur sans le détruire.
4. **Le témoin propre de la mesure ②** : mode `separe` **avec** une duplication
   ouverte, pour fermer la comparaison à deux variables.
5. **8 encodeurs alimentés ensemble**, pour savoir s'ils tiennent la cadence —
   la mesure ② ne porte que sur la création.
   > ✅ **Note postérieure (31/07/2026)** : **fait, ils tiennent** — 90,1 i/s
   > par fenêtre en capture+encodage à N=8, zéro verdict faux
   > (`2026-07-31-duplications-paralleles-resultats.md` §3.2). Sur **huit
   > périphériques D3D11 distincts**, pas sur le périphérique unique partagé de
   > la mesure ② : ce montage-là reste, lui, jamais alimenté.
6. **Le chien de garde en isolement** (`ApolloService` arrêté), si une
   exploitation durable des sorties virtuelles est engagée.
7. **Un créneau borné sur `0x800706BE`** (voie `Windows.Graphics.Capture`), et
   `DwmGetDxSharedSurface`, cinquième voie jamais sondée — inchangé depuis la
   sonde précédente, toujours hors chemin critique.

### Ce que le chantier D hérite, et doit ne pas défaire

- **Le trait `VoieDeCapture` corrigé** : acquisition et recadrage **séparés**,
  une texture de destination **par voie**. Refondre les deux réintroduit la
  famine et l'écrasement mutuel de textures (piège de la sonde précédente,
  toujours valable).
- **Le pilote de moniteurs virtuels** (`agent/src/diagnostics/multifenetre/`),
  sa garde de destruction RAII testée panique comprise, et **sa purge autonome**.
  La garde ne suffit pas — le plantage de la mesure ③ le prouve — et c'est
  pourquoi la purge existe.
- **Le décompte des verdicts par nature** ajouté au banc, et la vérification
  **avant** recouvrement. Sans eux, la mesure ③ n'aurait pas pu distinguer ses
  propres issues.
- **Le protocole des points reportés** ci-dessous : huit d'entre eux sont des
  dettes de code réelles.

---

## Points reportés — triés

Le chantier a laissé **dix-neuf points reportés** : ceux marqués
`minor (deferred)` à son journal de bord, plus ceux relevés en revue et non
traités sur-le-champ. Les taire serait contraire à sa règle ; les traiter tous
relèverait d'un autre chantier. Voici le tri — huit d'un côté, onze de l'autre.

### À traiter par le chantier D — ce sont des dettes de code réelles

| Point | Où | Pourquoi il ne peut pas rester |
| --- | --- | --- |
| `a_purger` n'est relue par personne depuis la purge inter-processus | `moniteurs.rs` / `purge.rs` | Un `detruire` échoué est irrécupérable dans le processus alors même que son GUID est connu. La demi-mesure existe déjà (`rejouer_purge_due`, appelée depuis la montée) ; il manque le chemin général |
| Le compteur de GUID repart de zéro **par instance** de pilote, pas par processus | `moniteurs.rs` | Deux instances ouvertes en parallèle attribuent les **mêmes** GUID à des sorties distinctes, et la purge n'en retire qu'une. Le chantier D ouvrira des sorties depuis un service durable : ce cas cesse d'être théorique |
| L'unicité des GUID n'est garantie qu'en deçà de 65 536 créations | `moniteurs.rs` | Énoncé en commentaire, non résolu. Un service à longue durée de vie qui crée et détruit au rythme des fenêtres y arrive |
| `noms_attaches` / `manquants` sont pures et testables mais vivent sous `cfg(windows)` | `montee.rs` | Aucun test ne les couvre depuis Linux, alors que ce sont exactement les fonctions qui décident si une sortie a paru ou disparu. Le chantier D s'appuiera dessus |
| `vers_texture` soustrait deux `i32` bruts avant conversion | `moniteurs_virtuels.rs` | `geometry.rs` documente pourquoi ce calcul passe par `i64` dans ce projet. Risque réel quasi nul aux résolutions actuelles, mais c'est une incohérence de convention dans du code que le chantier D va reprendre |
| Angle mort du contrôle par rang : si notre sortie ne paraît pas **et** qu'une sortie externe paraît au même tour, le contrôle passe en attribuant au pilote un nom qui n'est pas le nôtre | `montee.rs:356-366` | Rien ne relie l'identifiant rendu par l'IOCTL au nom DXGI. Sur une VM où Apollo peut ajouter une sortie à tout instant, c'est un faux positif possible |
| `presentes` journalise la liste mémorisée au relevé précédent, pas une relecture fraîche — et son libellé suggère le contraire | `montee.rs:346-353` | L'écart mesuré n'est que de 0,673 ms et la conclusion tient (§Mesure ①), mais c'est **le champ sur lequel repose la preuve par identité du chiffre pivot**. Un libellé qui promet plus qu'il ne porte se recopie : la première rédaction de ce document l'a fait. Correction d'une ligne |
| Le banc est aiguillé **avant** la purge | `multifenetre.rs:74` (banc) devant `:103` (purge) | Poser `MULTIFENETRE_BANC=…` **et** `MULTIFENETRE_VDD_PURGE=1` fait tourner le banc **sans purger**, donc sur une topologie éventuellement polluée, et rien au journal ne dit qu'une purge avait été demandée. Le commentaire de la branche de purge se croit pourtant « placée avant TOUTE sonde qui crée des sorties » : vrai des sondes `VDD*`, faux du banc. Purger puis mesurer est l'enchaînement naturel après le plantage documenté de la mesure ③, et le chantier D réutilisera ces sondes |

### Peuvent rester

| Point | Pourquoi |
| --- | --- |
| Aucun test n'exerce le chemin où `detruire()` échoue dans la boucle de `Drop` | Vérifiable par lecture, trou hérité du plan (tests recopiés verbatim). À combler si la garde évolue |
| Le `debug!` des refus de purge n'apparaît dans aucun journal versé | Niveau non atteint par défaut, à dessein : un `info` par essai noierait le signal sur seize tentatives dont la majorité échoue normalement |
| `avant` / `apres` de la purge comptent toutes les sorties DXGI, pas les seules attachées | Hérité de `relever_topologie`. Sans incidence sur le verdict de purge |
| La trace de protection multi-fils des périphériques autonomes est en `debug!` | Sa pose n'est attestée par aucun relevé versé. Sans conséquence tant qu'aucune image n'est soumise sur ces périphériques ; à reprendre s'ils servent un jour |
| `MULTIFENETRE_SORTIE` n'est exercée par aucun journal versé | Variable de confort pour rejouer le banc à la main. `analyser_designation` est couverte par un test unitaire |
| Provenance du pointeur de chemin SetupAPI ; `commander` est une `fn` sûre qui déréférence des pointeurs bruts ; première erreur SetupAPI avalée ; justification des droits d'accès un peu appuyée | Points de forme sur du code `#[cfg(windows)]` éprouvé en conditions réelles (dix créations, dix destructions, une purge de huit) |
| Journalisation faite **avant** la vérification du compte d'octets rendus par le pilote | Une réponse de taille inattendue est donc tracée comme si elle était valide, puis rejetée. L'ordre est trompeur à la lecture d'un journal, mais la vérification a bien lieu et rien de faux n'est retenu — à corriger si ce chemin gagne des appels dont la sortie est exploitée sans contrôle |
| Journaux versés en mode `100755`, fins de ligne CRLF héritées de la VM | Cosmétique |
| Chaque rejeu de sonde écrase les journaux précédents sans archivage | Les journaux qui comptent sont versés au dépôt ; l'archivage automatique serait de l'outillage pour l'outillage |
| Le second commit de la tâche 1 dépasse la portée littérale du brief | Autorisation tracée dans le rapport et le corps du commit — informatif, pas un défaut |
| Un rapport résume un échec de compilation attendu sans en coller la sortie brute | Le fait attendu est vérifié, seule la pièce manque |

---

## Pièges rencontrés

Écrit pour qui reprendra ce terrain. Chaque piège a coûté du temps réel dans ce
chantier. Ils s'ajoutent à ceux de la sonde précédente, qui restent tous
valables.

1. **Rendre un journal PowerShell lisible demande DEUX réglages, pas un.**
   `Tee-Object` sous PowerShell 5.1 écrit en UTF-16LE et n'a pas de paramètre
   d'encodage — c'est la cause documentée jusqu'ici. Mais la corriger par un
   `StreamWriter` en UTF-8 sans BOM **ne suffit pas** : les accents restaient
   corrompus, parce que `[Console]::OutputEncoding` valait encore la page de code
   OEM et abîmait les caractères **en lisant** la sortie du processus enfant,
   avant même que quoi que ce soit ne soit écrit. Le `StreamWriter` règle
   l'écriture, `[Console]::OutputEncoding` la lecture. **Les deux sont
   nécessaires.** Symptôme qui met sur la piste : un `grep` sur un mot accentué
   rend 0 alors que le même `grep` sur sa partie ASCII rend 1.
   *(Ne pas non plus remplacer `Tee-Object` par `Out-File -Encoding utf8` : il
   corrige l'encodage mais replie les lignes à la largeur de console.)*

2. **Ne jamais se fier au texte d'un HRESULT pour désigner un appel.**
   `MF_E_UNSUPPORTED_D3D_TYPE` se traduit par « Le type d'**entrée** n'est pas
   pris en charge pour le périphérique D3D » — et l'appel qui le rend configure
   le type de **sortie**. Ce seul libellé a fait attribuer le plafond d'encodage
   à un mauvais composant dans le document précédent, et la prémisse du plan de
   ce chantier (« le composant se nommera de lui-même ») en a hérité. **Dix
   annotations de contexte** ont été nécessaires pour trancher.

3. **Un compteur ne suffit pas quand un tiers peut agir sur le système.** Sur
   une VM où Apollo pilote la configuration d'affichage, une addition externe
   compense exactement un retrait : un contrôle par cardinal passe alors qu'une
   sortie a disparu. **Comparer des ensembles de noms, jamais des nombres** —
   c'est ce qui distingue, dans la mesure ①, « le pilote refuse » de « la sortie
   a été remplacée ». Le même durcissement vaut pour les contrôles de retour à
   l'état initial.

4. **Le contrôle qui vaut se fait depuis un processus neuf.** Le processus
   mesureur est juge et partie sur le retour à l'état initial, et une sortie
   virtuelle lui survit. Toutes les affirmations de propreté de ce chantier sont
   adossées à un relevé indépendant — `moniteurs-etat-initial.log`,
   `moniteurs-capture-etat-final.log`, `moniteurs-purge-etat-restaure.log`.

5. **Un plantage à la destruction se lit comme un plafond.** Dans la mesure ②,
   deux traces encadrant le relâchement (`relâchement des encodeurs…` puis
   `relâchement terminé — le processus a survécu à la destruction`) distinguent
   par construction deux modes d'échec que rien d'autre ne séparait. Sans elles,
   le plantage découvert à la mesure ③ aurait pu être compté comme une limite de
   ressource. **Instrumenter la sortie autant que l'entrée.**

6. **Localiser une mort de processus se fait à l'horodatage, pas à
   l'appréciation.** « Une ou deux secondes avant la fin de la passe » aurait
   envoyé le diagnostic fouiller la soumission d'images ; le comptage exact
   (dixième et dernière ligne périodique à début + 10,00 s, dans **deux**
   exécutions) désigne la **libération**. Un journal périodique à la seconde suffit
   à trancher, à condition de le lire.

7. **Modifier le banc rend les mesures antérieures non comparables — et il faut
   le dire.** La lecture de pixel a changé de portée en cours de chantier ; les
   cadences `printwindow` publiées avant viennent d'un banc qui relisait deux
   fois moins. Le risque est borné par un témoin (deux versions du banc donnant
   `[90.0, 90.0]` sur le même montage), mais **borné n'est pas nul**. Toute
   modification d'un instrument doit être signalée avec les mesures qu'elle
   invalide potentiellement.

8. **Un état « sale » ne se présume pas, il se constate depuis l'extérieur.**
   L'épreuve de la purge vaut parce qu'un processus **tiers** a compté 9 sorties
   là où il en attendait 1, et qu'un **quatrième** a confirmé le retour à 1.
   Une purge qui se juge elle-même ne prouve rien.

9. **Le nombre de créations qu'une épreuve obtient n'est pas une mesure du
   pilote.** L'épreuve de salissure a produit 8 sorties, pas 10 : le `sleep 15`
   du script mesure le temps côté hôte, quand le processus a vécu ~21 s (latence
   de démarrage de la tâche planifiée, puis latence de `Stop-Process` par WinRM).
   Huit tentatives à 3 s d'écart tiennent dans cette fenêtre, pas dix. **Un
   compte obtenu par minutage est un artefact de minutage** — le plafond reste
   celui de la mesure ①.

10. **Le mode de défaillance dominant reste l'énoncé, pas le code.** Comme pour
    la sonde précédente, quasiment toutes les corrections de revue de ce chantier
    ont porté sur des phrases qui affirmaient au-delà de leur relevé : une
    estimation présentée comme une observation, une borne élargie entre un titre
    et son corps, une exclusion d'hypothèse gravée dans un commentaire commité
    alors que le relevé ne l'excluait pas, un effet de bord passé sous silence.
    **Un seul point de tout le chantier a vu le code contredire son rapport** (une
    fonction structurellement morte présentée comme « peu utile en pratique »).
    Le coût d'un chantier de mesure est dans la discipline de l'énoncé.

---

## Annexe — comment rejouer

Le banc et les sondes vivent dans `agent/src/diagnostics/multifenetre/`, pilotés
par variables d'environnement, **un processus par voie** (ces API échouent par
plantage du processus, pas par code d'erreur).

| Variable | Ce qu'elle fait |
| --- | --- |
| `MULTIFENETRE_DXGI=1` | Relève la topologie DXGI et sort — le contrôle d'état depuis un processus neuf |
| `MULTIFENETRE_WGC=1` | Sonde `Windows.Graphics.Capture` (voie éliminée) |
| `MULTIFENETRE_REPLIS=1` | Sonde les voies de repli |
| `MULTIFENETRE_BANC=<voie>` + `MULTIFENETRE_N=<n>` | Le banc de cadence : `duplication` ou `printwindow`, N de 1 à 8 |
| `MULTIFENETRE_SORTIE=<adaptateur:sortie>` | Force la sortie DXGI capturée par le banc |
| `MULTIFENETRE_CONTRAT=1` | Éprouve le contrat IOCTL du pilote (deux tampons simples, sans effet de bord) |
| `MULTIFENETRE_VDD=1` | **Mesure ①** — montée en N de sorties virtuelles jusqu'au refus |
| `MULTIFENETRE_VDD_VEILLE=<secondes>` | Épreuve du chien de garde : une sortie, aucun ping, relevé à 1 Hz |
| `MULTIFENETRE_VDD_PURGE=1` | Purge autonome des sorties orphelines |
| `MULTIFENETRE_VDD_CAPTURE=1` | **Mesure ③** — crée une sortie virtuelle et y lance le banc |
| `MULTIFENETRE_NVENC=partage\|separe` | **Mesure ②** — plafond d'encodeurs, périphérique D3D11 partagé ou un par encodeur |

La VM Windows n'est pas démarrée automatiquement : voir la section « Cycle de vie
de la VM Windows » de `CLAUDE.md` avant toute séquence qui en dépend.
