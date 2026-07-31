# N duplications DXGI de front sur N sorties virtuelles — conception

> ✅ **Note du 31 juillet 2026, postérieure à ce document — CE CHANTIER EST
> EXÉCUTÉ.** Ce qui suit décrit l'état **d'avant**, et le reste : c'est une
> conception, pas un compte rendu. Les résultats sont dans
> `plans/2026-07-31-duplications-paralleles-resultats.md`. En deux lignes : la
> **voie est reçue** (90,1 i/s par fenêtre en capture+encodage à N=8, zéro
> verdict faux, huit duplications de front — une exécution par rang, donc aucun
> taux), et le défaut hérité du §5 est **diagnostiqué et corrigé**. Toute phrase
> de ce document qui dit « n'a jamais été mesuré », « reste dû » ou « n'est pas
> diagnostiqué » est à lire au passé.

> Troisième chantier de mesure de la série, dans la lignée de la sonde de
> capture multi-fenêtres (`plans/2026-07-30-sonde-capture-multifenetre-resultats.md`)
> et des mesures préalables (`plans/2026-07-31-mesures-prealables-chantier-d-resultats.md`).
> Il ne construit rien du **chantier D (multi-fenêtres)**, décrit dans
> `2026-07-28-support-jeux-design.md` §4 et §5 : il éprouve l'arrangement que
> la voie recommandée propose réellement, et que rien n'a encore exercé.

## 1. Question posée

Les mesures préalables ont levé les quatre inconnues qui bloquaient la
spécification du chantier D, et déclarent celui-ci spécifiable. Elles laissent
cependant, nommément, **une mesure due avant de dimensionner la voie** :

> N sorties virtuelles, **une fenêtre chacune**, donc **N duplications DXGI de
> front**. Le banc a posé N fenêtres sur **UNE** sortie. DXGI n'autorisant
> qu'une duplication par sortie, la question est réelle.

C'est l'arrangement que la voie recommandée propose *réellement*. Tout ce qui a
été mesuré jusqu'ici l'a été sur un montage qui n'est pas celui-là.

Une seconde réserve, distincte, tombe par le même montage. Les cadences de la
sonde — 80,2 / 99,4 / 98,8 / 107,5 i/s par fenêtre à N = 1/2/4/8 — ont été
prises **à aire totale fixe** : `disposition::tuiles` découpe un bureau, donc la
surface par fenêtre décroît quand N croît. Le débit de pixels y est quasi
constant *par construction*, et **le nombre de fenêtres n'y est pas prouvé
neutre en soi**. N sorties virtuelles de 1280×720 **chacune** donnent, elles,
une aire qui croît avec N : c'est le cas produit, et il n'a jamais été mesuré.

## 2. Décisions de cadrage

| Décision | Retenu |
| --- | --- |
| Nature du bloc | Chantier de **mesure**, aucun livrable produit — à une exception près (§5) |
| Périmètre | N sorties × 1 fenêtre × 1 duplication × 1 encodeur, **capture ET encodage** |
| Résolution | **1280×720@60 par sortie** — aire utile, non partagée |
| Rangs | N = **1, 2, 4, 8**, un processus par rang |
| Contrôle d'image | **Une voie par tour, en rotation** |
| Critère de réception | À N=8 : **≥ 60 i/s par fenêtre en capture+encodage, zéro verdict faux** |
| Défaut ouvert hérité | **Diagnostiqué et corrigé d'abord**, comme défaut produit |
| Emplacement du code | `agent/src/diagnostics/multifenetre/`, hors chemin de production |

**Pourquoi capture ET encodage, et non la capture seule.** La question ouverte
ne porte que sur les duplications, et s'en tenir là serait plus court. Mais
l'exigence du produit est de faire tourner huit fenêtres *encodées* ; une
mesure de capture nue laisserait le vrai dimensionnement à un troisième
chantier. Le coût de cette extension est connu et borné : elle rencontre le
défaut ouvert hérité, traité au §5.

**Pourquoi une résolution utile par sortie.** Voir §1 : c'est le seul montage
où le nombre de fenêtres varie sans que la surface par fenêtre bouge. Il lève
gratuitement une réserve que la sonde a explicitement laissée ouverte.

**Pourquoi la rotation du contrôle d'image.** Le montage supprime le
recouvrement — une fenêtre par sortie, rien ne peut en cacher une autre — donc
la porte éliminatoire du banc actuel n'a plus d'objet. Le risque devient
l'**appariement** : que la voie *i* capture en réalité la sortie *j*, ou du
noir. Contrôler les N voies à chaque tour le détecterait, mais ferait croître
le coût CPU du contrôle avec N : la cadence relevée à N=8 intégrerait huit fois
ce coût et ne serait comparable à rien. C'est exactement l'erreur déjà payée au
chantier précédent, où la portée de la lecture de pixel a changé en cours de
route et a rendu deux rangs non comparables aux deux autres. La rotation donne
une couverture complète pour **une lecture par tour quel que soit N**.

## 3. Protocole

**Sonde** : `MULTIFENETRE_VDD_PARALLELE=<N>`, N de 1 à 8. Un processus par rang
— ces API échouent par **plantage du processus**, pas par code d'erreur, et une
sonde monolithique perdrait les rangs déjà mesurés.

**Séquence, dans un seul processus** (la garde `moniteurs_virtuels::Sorties`
détruit les sorties à la fin du processus : un banc lancé après coup ne
trouverait plus rien à capturer) :

1. Relevé de topologie **avant**, ensemble de noms mémorisé.
2. Création de N sorties virtuelles à 1280×720@60, en pinguant le chien de
   garde pendant l'attente de reconfiguration.
3. Relevé **après**, désignation des sorties neuves par **différence
   d'ensembles de noms** — pas par index : DXGI renumérote ses sorties à chaque
   reconfiguration. Refus si cet ensemble ne compte pas exactement N éléments :
   une addition externe rendrait la mesure inimputable. Le cardinal n'est
   éprouvé qu'**après** la différence de noms, jamais à sa place (§7).
4. Une mire **plein cadre** par sortie, une `SourceDuplication` par sortie, un
   encodeur par sortie.
5. Trois passes de 10 s : témoin, capture, capture+encodage.
6. Contrôle de survie **nommément** après chaque passe, purge des orphelines,
   relevé final comparé à l'ensemble initial.

**Trois passes, et le témoin n'est pas décoratif.** Huit swapchains peignant à
60 Hz consomment du GPU ; sans passe où les mires peignent seules, un
décrochage à N=8 serait indiscernable d'un décrochage des mires elles-mêmes.

**Le chien de garde court pendant la mesure.** Le banc actuel ne pingue pas :
il tenait trente secondes sur une seule sortie, et `capture_virtuelle.rs`
signale déjà ce point comme un risque assumé, rattrapé après coup par un
contrôle de survie. Ici, huit sorties sont exposées, sous un garde dont
**l'unité reste inconnue — aucune n'est exclue, pas même la seconde**. La
boucle de passes pingue donc à 1 Hz, et la survie est contrôlée après chaque
passe. Sans cela, une sortie retirée sous la mesure ferait imputer à Windows un
défaut du protocole.

**Contrôle d'image.** Au tour *t*, la voie `t mod N` est lue en son centre et
doit y trouver **sa** mire. Quatre compteurs par voie : `juste`, `voisine`
(l'image porte l'identité d'une *autre* mire — l'appariement croisé, risque
propre à ce montage), `noire`, `inconnue`. Le décompte par nature est repris du
banc actuel, où il a déjà prouvé son utilité : une image noire et une image
portant la mire du dessus sont deux résultats opposés, et les confondre rend la
mesure ininterprétable.

**Ce que la sonde ne doit pas traiter comme une panne.** Si la Kᵉ
`DuplicateOutput` échoue, **c'est le résultat** : il est journalisé avec le rang
K, le HRESULT nu et la sortie visée nommément, et la sonde s'arrête proprement.
Une duplication refusée n'est pas une erreur d'exécution, c'est la réponse à la
question posée.

**Aucune trace par trame** : compteurs agrégés, journalisés à la seconde. Une
trace par paquet écrite sur le partage CIFS a détruit la session qu'elle
mesurait au chantier NAT.

## 4. Critère de réception, et ce qu'il ne couvre pas

**Reçue** si, à N=8 : chaque fenêtre tient **≥ 60 i/s en capture+encodage** — la
cible du chantier 0, celle que le produit promet — **et** aucun verdict faux sur
aucune voie.

**Ce que la mesure n'établira pas**, à écrire d'avance pour que le rapport ne le
dépasse pas :

- rien au-delà de 8 sorties, ni d'autres résolutions ou débits ;
- rien de la **latence** — seule la cadence est relevée ;
- rien du comportement quand un tiers (Apollo) consomme des sorties du même
  vivier de 10 ;
- rien de la **couche** qui imposerait un éventuel plafond, si plafond il y a ;
- rien de la mise en sommeil des fenêtres masquées : que détruire un encodeur
  libère la place reste une conjecture non éprouvée, hors périmètre ici.

## 5. Le défaut ouvert hérité — traité en premier, comme défaut produit

> ✅ **Note du 31 juillet 2026 — DÉFAUT DIAGNOSTIQUÉ ET CORRIGÉ**, et
> **les trois hypothèses ci-dessous sont TOUTES FAUSSES** : ne pas les
> reprendre. Aucune n'était la bonne parce que la question était mal posée —
> elles cherchaient toutes un **ordre de destruction sur le fil principal**, or
> la faute s'exécute sur un **fil de pool** : la MFT NVIDIA gardait un élément
> de travail en vol quand on relâchait l'encodeur
> (`RtlEnterCriticalSection` sous `CSerialWorkQueue::QueueItem::ExecuteWorkItem`,
> `DebugInfo` nul). `MFShutdown`, que le diagnostic a d'abord accusé, a été
> **réfuté par la mesure**. Le défaut était en outre **intermittent** (2
> plantages sur 6 exécutions), pas déterministe comme ce paragraphe le laisse
> croire. Correctif : `agent/src/encode/arret.rs` — file de travail sérialisée
> par encodeur, sentinelle attendue avant relâchement, plus
> `IMFShutdown::Shutdown` ; **0 récidive sur 20 exécutions** du cas comparable,
> *ce qui n'est pas une preuve d'absence*. Détail en
> `plans/2026-07-31-duplications-paralleles-resultats.md` §7.

Les mesures préalables laissent un défaut **localisé, non diagnostiqué** : sur
la voie `duplication`, la passe d'encodage **tue le processus à la SORTIE de sa
boucle** — donc à la libération des encodeurs, pas à la soumission d'images —
après un encodage réel, quelle que soit la sortie capturée, et laisse une sortie
virtuelle orpheline.

Il bloque le présent chantier de front : le bilan ne serait jamais journalisé.
Mais il ne se contourne pas, parce qu'il **n'est pas un défaut de banc** : dans
le chantier D, fermer une fenêtre détruira son encodeur. C'est le même chemin,
en production.

**Méthode** — celle de systematic-debugging, avec la leçon du chantier
précédent : *instrumenter la sortie autant que l'entrée*. Le cas minimal est
déjà connu et n'exige aucune sortie virtuelle
(`MULTIFENETRE_BANC=duplication MULTIFENETRE_N=1` sur le bureau physique meurt
au même endroit). Une trace encadre chaque relâchement pris **séparément** —
chaque encodeur, puis la source de duplication — pour **désigner** le
responsable plutôt que le déduire.

**Trois hypothèses, aucune privilégiée avant les traces** : un ordre de
destruction où le `H264Encoder` survit au périphérique D3D11 que sa voie
possède ; un pipeline Media Foundation jamais drainé ni arrêté avant
relâchement ; un gestionnaire de périphérique DXGI relâché avant les transforms
qui s'en servent.

**Vérification** : la passe capture+encodage journalise sa ligne de bilan, le
processus rend la main, la garde court, et la topologie revient **nommément** à
son état initial.

C'est l'unique livrable produit de ce chantier ; le reste est de la mesure.

## 6. Structure du code

Le montage multi-sorties diverge réellement du banc actuel : pas de
recouvrement donc pas de porte éliminatoire, un verdict en rotation, une
`SourceDuplication` **par sortie** au lieu d'une partagée. Trois logements
étaient possibles ; le retenu est l'extraction du cœur commun.

- **`compteurs.rs` (neuf)** — la métrologie, extraite de `banc.rs` : `Verdicts`,
  `Compteurs`, `journaliser`, `passe_temoin`, le verdict sur pixel. Ce qui ne
  dépend pas du protocole.
- **`banc.rs`** — conserve le protocole mono-sortie à recouvrement, et redescend
  sous 300 lignes.
- **`paralleles.rs` (neuf)** — le pilote multi-sorties : création des N sorties,
  N mires, N sources, N encodeurs, rotation du verdict, ping du garde.

**Pourquoi extraire plutôt que dupliquer ou généraliser.** Généraliser `banc.rs`
y ferait cohabiter deux protocoles incompatibles et le pousserait vers les 500
lignes que la convention du projet interdit de franchir. Écrire un banc
autonome dupliquerait la boucle de passes : deux bancs divergeraient, et l'on ne
saurait plus si un écart de cadence vient de la voie ou du banc. L'extraction
est le seul montage où « 90,0 i/s à N=1 sur une sortie » et « X i/s à N=8 sur
huit sorties » se comparent sans réserve.

Si `paralleles.rs` approche les 500 lignes à l'écriture, le pilote des sorties
se sépare de la boucle de passes. Le découpage se décide sur le fichier réel.

**Testable sans Windows, donc écrit en TDD** : le choix de la voie contrôlée à
chaque tour (propriété : sur *k·N* tours, chaque voie est contrôlée exactement
*k* fois), l'appariement sortie ↔ mire ↔ voie, et la conversion des places en
coordonnées de texture **par sortie** — chaque sortie virtuelle porte son propre
facteur d'échelle, là où le banc actuel n'en connaît qu'un. Le reste est
`#[cfg(windows)]` sans filet, comme la dette assumée du projet.

## 7. Contrôles de validité — hérités, non négociables

- **Comparer des ensembles de noms, jamais des nombres.** Apollo peut ajouter
  une sortie à tout instant : une addition externe compense exactement un
  retrait, et un contrôle par cardinal passe alors qu'une sortie a disparu.
- **Le contrôle qui vaut se fait depuis un processus NEUF** (`MULTIFENETRE_DXGI=1`),
  après coup : le processus mesureur est juge et partie, et une sortie virtuelle
  lui survit.
- **Purge des orphelines entre les rangs** (`MULTIFENETRE_VDD_PURGE=1`).
- **Journaux en UTF-8 par les DEUX réglages PowerShell** : le `StreamWriter`
  règle l'écriture, `[Console]::OutputEncoding` la lecture de la sortie du
  processus enfant. L'un sans l'autre laisse du mojibake.
- **Ne pas se fier au texte d'un HRESULT pour désigner un appel** — chaque
  appel du chemin de construction porte son propre contexte.
- **Le mode de défaillance dominant de ces chantiers est l'énoncé, pas le
  code** : le rapport ne dit que ce que les journaux montrent.

## 8. Ordre des tâches

1. Diagnostic et correctif de la libération des encodeurs (§5).
2. Extraction de la métrologie vers `compteurs.rs` — remaniement pur, tests
   existants verts.
3. Pilote multi-sorties `paralleles.rs` et rotation du verdict.
4. Prises de mesure aux rangs 1, 2, 4, 8 ; journaux versés.
5. Document de résultats, puis mise à jour de `CLAUDE.md` et de
   `2026-07-28-support-jeux-design.md` §5 D.
