# Pièges transverses — méthode de mesure

> Extrait de [`CLAUDE.md`](../../CLAUDE.md) (lignes 545-728 de l'original), déplacé tel quel. Liens relatifs à ce dossier.

## 🪤 Pièges transverses — ceux que ce dépôt a payés PLUSIEURS fois

⚠️ **Les pièges propres à un chantier sont restés dans son § « Pièges neufs »
du journal.** Ceux d'ici ont été payés dans au moins deux chantiers
indépendants : ce sont eux qui coûtent.

### Méthode de mesure

- 🔴 **UN CONTRÔLE QU'ON N'A JAMAIS VU ROUGE N'EST PAS UN CONTRÔLE.** Payé une
  dizaine de fois. **La règle complète est en deux temps :** exécuter le
  contrôle, **PUIS provoquer délibérément l'état qu'il doit dénoncer et
  vérifier qu'il le dénonce**. Un contrôle peut être exécuté, rendre un
  diagnostic juste, et rester **structurellement incapable de rendre l'autre
  valeur** — c'est ainsi qu'un `grep` de recette écrit pour révéler une panne
  muette a couru sans pouvoir échouer. **Un plan n'immunise pas contre ce
  patron : il en est une source.**
  🔴 **SA FORME LA PLUS TRAÎTRE : UN ATTENDU DÉRIVÉ DE LA MESURE ELLE-MÊME.**
  Payé au lot 32R, en citant la règle dans le même document. Trois points de
  curseur mesurés, le rectangle de mappage **déduit de ces trois points**, puis
  chaque point comparé **à ce rectangle-là** : « écart nul au pixel » ne disait
  rien d'autre que « les trois points sont alignés », et **tout mapping affine
  passait**. L'erreur réelle était de **432 px** au bord droit, et un humain
  l'a vue le lendemain. **L'attendu doit venir du PRODUIT — une dimension
  relevée dans son journal, une constante de sa configuration —, jamais d'un
  calcul sur les points qu'on juge.** ⚠️ Le symptôme est un contrôle qui
  « passe parfaitement » : plus l'accord est bon, plus il faut se demander
  d'où vient l'attendu.
- 🔴 **UN CONTRÔLE QUI NOMME UNE CHAÎNE QUE LE PRODUIT N'ÉMET PAS REND ZÉRO
  POUR TOUJOURS — ET C'EST INDISCERNABLE D'UN VRAI ZÉRO.** ⚠️ **C'est le
  jumeau lexical du piège précédent, et il mord plus souvent** : là-bas le
  contrôle ne pouvait pas rendre l'autre valeur pour une raison de
  *construction* ; ici, pour une raison de **VOCABULAIRE** — il cherche un mot
  que le produit n'écrit nulle part. **La règle qui les couvre tous les deux :
  un contrôle dont on n'a pas vu la valeur NON ATTENDUE au moins une fois n'est
  pas un contrôle.** Corollaire pratique : **le motif d'un `grep` de recette se
  vérifie contre le CODE QUI L'ÉMET** (`grep -n` sur le `tracing::`/`console.`
  lui-même), jamais contre la spec, le plan, ou le souvenir qu'on en a.
  🔴 **SIX INSTANCES DATÉES DU MÊME JOUR — le 5 septembre 2026, lot 3 —, les
  trois premières venues du PLAN et la dernière de la DOCUMENTATION, aucune de
  l'implémenteur :**
  - **une variante d'énumération n'est pas une chaîne de journal.**
    `grep -ac "NonMesuree" agent.log` rend **0** et ne peut rendre que 0 :
    `SourceMax::NonMesuree` est un identifiant Rust (34 occurrences dans
    `agent/` et `proto/`, **aucune** dans un `tracing::`), et sur le fil la
    sérialisation est `"source_max":"non-mesuree"`. Le chiffre existait — 34
    sur 41 — mais dans le catalogue servi par la plateforme, pas dans le
    journal. **Le legs qui en découlait annonçait 71.**
  - **un accent suffit, et c'était le TÉMOIN.** Le plan prescrivait
    `grep -ac "attache au capteur"` comme témoin destiné à rendre le zéro de
    `message REFUSE` interprétable. Le produit écrit `"attaché au capteur"`
    (`capteur/tube.rs:153`) : le motif du plan rend **0**, celui du code rend
    **457**. **Un témoin faux ne dégrade pas la mesure, il l'ANNULE** — « zéro
    refus, témoin à zéro » ne distingue pas un produit sain d'un produit mort.
  - **un commentaire dicté verbatim peut être faux.** Le plan faisait écrire à
    la sonde de latence que l'agent annonce l'instant de capture « par le
    sender report RTCP […] rendu dans `metadata.captureTime` ». La première
    moitié est vraie et vérifiée (204 SR sur le fil, 63 lus par Chrome) ; la
    seconde est **fausse** — `captureTime` vient de l'extension d'en-tête RTP
    `abs-capture-time`. La mesure a rendu **0 échantillon sur 9 084 trames**
    avec un produit qui marchait.
  - 🔴 **LA PLUS DÉMONSTRATIVE DES SIX FORMES : UN OUTIL QUI FABRIQUE UNE
    VALEUR PLAUSIBLE EN ÉCHOUANT.** `scripts/winrm.js` imprimait la **pile de
    son erreur sur STDOUT** et sortait avec le code **0**. Un appelant écrit
    `node scripts/winrm.js '(Get-Process agent …).Count' | tr -dc '0-9'`
    moissonnait alors **les numéros de ligne de la pile** — vu réellement sur
    la VM le 28 août 2026 : `🔴 22246232650828772271221761422508285591251033905
    agent(s) survivant(s)`. **Le contrôle ne pouvait pas échouer ET rendait un
    nombre**, ce qui est pire qu'un zéro : un zéro intrigue, un nombre rassure.
    🔵 **Le seul remède est de lire le CODE DE RETOUR**, qui sépare « la
    requête a réussi et rend un compte » de « la requête a échoué et n'a rien
    à dire » — un filtre de caractères ne le peut jamais.
  - 🔴 **LA PLUS DANGEREUSE DE LA SÉRIE : UN INSTRUMENT QUI FABRIQUE UNE
    DONNÉE QUE RIEN NE SIGNALE COMME ABSURDE.** Les autres formes rendent un
    contrôle incapable d'échouer ; celle-ci **rend un nombre**, dans un dépôt
    dont la raison d'être est de porter des chiffres datés. Mesuré le
    5 septembre 2026, item 2 : un banc de débit du pont a rendu
    **1 312 669 Kio/s** — 615 327 tours, 80,6 Go en 60 s, **quarante mille
    fois** le débit réel. Il relisait **le même fichier** en boucle, donc il
    mesurait le **cache de fichiers de Windows**, pas la traversée.
    ⚠️ **Le symptôme n'était pas une erreur : c'était un NOMBRE — et un nombre
    se lit comme une mesure.** Un `0` intrigue, un `1 312 669` en impose.
    🔵 **Le remède employé** : vingt fichiers **DISTINCTS** de 128 Kio, lus
    **une seule fois** chacun — 32,82 Kio/s, dans la fourchette de F4.
    🔵 **La règle générale** : *une mesure de débit qui ne relit jamais deux
    fois la même donnée est la seule qui mesure le transport.* Corollaire :
    **tout chiffre hors de l'ordre de grandeur attendu se traite comme un
    défaut d'instrument jusqu'à preuve du contraire**, jamais comme une bonne
    nouvelle.
  - 🔴 **HUITIÈME FORME, ET C'EST L'INSTRUMENT QUI CHOISIT SON SUJET SANS LE
    DIRE : UN DÉFAUT D'OPTION NON RENSEIGNÉE.** Mesuré le 5 septembre 2026
    (items 8 et 10). Le pilote de recette apparie l'application à lancer sur
    un motif par défaut — `chrome|edge|bloc.?notes|notepad` — qui retient
    **Microsoft Edge EN PREMIER**. Or **cette même campagne venait de mesurer
    qu'Edge n'est JAMAIS adopté** (règle d'appartenance : sa fenêtre est
    écartée 60 ms après le lancement). Quatre bras, sur deux items, ont donc
    rendu `fenetres SERVIES : 0` — **et j'ai failli attribuer ce zéro à un
    défaut du produit** (le `0x8000FFFF` du lot 31), dans un verdict et dans
    ce fichier. 🔴 **LE PIÈGE N'EST PAS LE DÉFAUT D'EDGE, C'EST QUE
    L'INSTRUMENT AIT CHOISI SON SUJET TOUT SEUL** : un paramètre non passé
    n'échoue pas, il prend une valeur — et cette valeur était le pire cas
    connu du jour. ⚠️ **Le symptôme est un zéro parfaitement plausible**, avec
    une cause produit toute prête à l'expliquer.
    🔵 **La règle : ce qu'un instrument SÉLECTIONNE — l'application, le
    périphérique, la sortie — doit être IMPOSÉ par l'appelant et RELU dans le
    relevé, jamais laissé à un défaut.** Le contrôle qui l'a attrapé est
    d'avoir cherché *quel* nom le pilote avait réellement lancé
    (`"nom":"Microsoft Edge"`) au lieu de croire au motif.
    🔵 **CE QU'IL FAUT RETIRER D'UNE ATTRIBUTION TROP RAPIDE** : les deux
    traces de désarmement que je croyais impossibles à produire (`presse-papier
    DESARME`, `accent de fenetre DESARME`) **sortent parfaitement** dès que le
    bras sert une fenêtre. **L'explication « le code ne peut pas les produire
    sur ce chemin » était FAUSSE, et c'est moi qui l'avais écrite.**
  🔵 **Ce qui a sauvé les quatre est le même geste** : un compteur ou un code
  de retour qui SÉPARE les causes d'un zéro (« aucune trame » contre « des
  trames sans le champ » ; « échec de transport » contre « compte nul »), et la
  relecture du code émetteur avant de conclure. Voir
  [les résultats partiels du lot 3](../superpowers/plans/2026-09-05-lot3-campagne-vm-resultats-partiels.md) § 3.
- 🔴 **UN CONTRÔLE D'EMPLOI PEUT COMPTER UN `var(--…)` QUE PERSONNE NE PEINT,
  OU UN ÉCRIVAIN, COMME UN « EMPLOI » — ET IL REND ALORS `0 orphelin(s)` SUR UN
  TOKEN QUI N'A PLUS AUCUN LECTEUR.** **Payé DEUX fois, ce qui le fait monter
  ici** : ① `--accent-fenetre` (lot 33) — le capteur la lit, l'agent l'annonce,
  `accent-dom.ts` la pose, et **aucune feuille ne la lit** ; le garde §7.6
  compte `poserToken(...)`, un **ÉCRIVAIN**, comme un appelant. ② `.bureau`
  (chantier `navigation-hub-unique`, 31 août 2026) — la classe n'est employée
  par **aucun balisage** depuis que le hub porte son propre conteneur, et elle
  est **conservée artificiellement** parce que sa déclaration est le **seul
  emploi de `--e-7` dans tout le dépôt** : la retirer ferait échouer §7.6.
  🔴 **Dans les deux cas le contrôle est VERT, et il est STRUCTURELLEMENT
  aveugle** — ce n'est pas un assouplissement, c'est sa construction. **La
  règle : « déclaré et employé » n'est pas « déclaré et LU ».** Ce qu'un
  contrôle d'orphelins établit est qu'un nom apparaît deux fois, jamais qu'il
  produit un pixel — et **on ne le voit qu'en regardant l'image**.
- 🔴 **UNE TRACE QUI NE PEUT SORTIR QU'EN CAS DE SUCCÈS NE PEUT PAS
  DIAGNOSTIQUER UN ÉCHEC.** Payé au lot 33, **sur une trace que le lot venait
  lui-même de retirer**. Le produit émettait `redimensionnement ignoré` à
  CHAQUE demande — c'est elle qui a rendu le diagnostic possible (34 demandes
  relevées, rapports d'aspect de 1,105 à 3,559). Le remède l'a remplacée par
  des traces placées **après un court-circuit** (« si la taille n'a pas
  changé, retourner »), si bien qu'un `0` au journal ne distinguait plus
  **« le message n'arrive jamais »** de **« il arrive et ne change rien »** —
  c'est-à-dire un défaut de la **limite déclarée du remède**. Le lot s'est
  ainsi rendu aveugle à son propre échec, et il a fallu un second envoi pour
  rouvrir les yeux. ⚠️ **Le symptôme est traître : le journal est SILENCIEUX,
  ce qui se lit comme « rien ne se passe » alors que le mécanisme tourne.**
  **La règle : sur un chemin qu'on instrumente pour diagnostiquer, la trace se
  pose AVANT le court-circuit, et elle nomme la BRANCHE prise** (un champ
  `decision`), jamais seulement le cas nominal. Corollaire : **remplacer une
  trace inconditionnelle par une trace conditionnelle est une PERTE
  d'observabilité, à traiter comme une régression** — et à mesurer avant, pas
  après.
- 🔴 **UNE ROUGE QUI ROUGIT POUR LA MAUVAISE RAISON NE PROUVE RIEN**, et elle
  est indiscernable d'une bonne si l'on ne lit que son code de sortie. **Lire
  QUELLE assertion a rougi.** Corollaire : **une rouge restée VERTE se
  DIAGNOSTIQUE, elle ne se classe pas** — deux fois, le diagnostic a révélé un
  trou de couverture réel.
- 🔴 **UN ZÉRO N'EST INTERPRÉTABLE QU'AVEC UN TÉMOIN NÉGATIF.** Un zéro rendu
  par une trace qu'on n'a pas allumée, par un chemin qui n'existe pas, ou par
  une chaîne que le produit n'émet nulle part, n'est pas une mesure. Vérifier
  dans le même relevé qu'une chose **connue pour exister** rend non-zéro.
- 🔴 **UNE SORTIE VIDE N'EST PAS UN ZÉRO** : `grep` sans `-a` classe « binaire »
  un journal portant un seul octet NUL et rend une sortie **vide**.
- 🔴 **JUGER SUR LA RELECTURE, JAMAIS SUR LE CODE DE RETOUR.** Une API peut
  rendre `0` sur une sortie qui n'a pas bougé d'un pixel ; trois appels
  `waveOut` peuvent tous réussir et ne rien jouer.
- 🔴 **ON JUGE UN SON À SA FRÉQUENCE DOMINANTE**, jamais à un compte d'octets
  ni à une crête : `bytesReceived` croît sur un spectre à −1000 dB, et une
  crête franche se lit sur un silence numérique (VB-Cable rejoue son tampon).
- ⚠️ **UN PALIER DE MESURE DOIT ÊTRE PLUSIEURS FOIS PLUS LONG QUE LA
  TEMPORISATION DU MÉCANISME QU'IL OBSERVE.** Lire les constantes de temps du
  code **avant** de dimensionner un palier. Et **attendre le FAIT, jamais une
  durée**.
- ⚠️ **ÉNONCER LA RÈGLE DE SÉLECTION AVANT DE COMPTER.** Un sous-ensemble sans
  règle énoncée est un sous-ensemble **choisi**, même quand on ne l'a pas
  choisi.
- ⚠️ **UN COMPTEUR DE JOURNAL PEUT COMPTER DES LIGNES ET NON DES ÉVÉNEMENTS**,
  et une « latence » lue dans un journal de pilote peut être une **période
  d'échantillonnage** (facteur 86 relevé une fois). **Prendre la latence du
  côté qui la SUBIT.**
- ⚠️ **UN BUDGET D'INJECTION DE FAUTE DOIT ÊTRE GLOBAL AU PROCESSUS**, jamais
  relu par fil : un budget qui se réarme rend le chiffre-juge **structurellement
  incapable de quitter zéro**, sur un produit pourtant corrigé.
- ⚠️ **DISCULPER UN MAILLON NE DÉSIGNE PAS LE COUPABLE SUIVANT** : il faut une
  mesure **par maillon**.
