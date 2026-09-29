# Pièges transverses — documentation et dérive, taille des fichiers, outillage VM et Windows

> Extrait de [`CLAUDE.md`](../../CLAUDE.md) (lignes 729-869 de l'original), déplacé tel quel. Liens relatifs à ce dossier.

### Documentation et dérive — « le naufrage du 487 »

- 🔴 **CORRIGER UNE AFFIRMATION EXIGE DE LA CHERCHER, PAS DE LA CORRIGER LÀ OÙ
  ON NOUS L'A MONTRÉE.** Payé **neuf fois**, dont une **à l'intérieur du commit
  qui le dénonçait**. « Corrigé à sa place » est une affirmation de
  **complétude** : énumérer les places par `grep -n` **AVANT** d'écrire, **et
  les relire une par une APRÈS**. Une substitution qui ne dit pas combien
  d'occurrences elle a touchées est une affirmation non vérifiée.
- 🔴 **CHERCHER PAR LE SENS, PAS PAR LA FORMULE** : une négation se dit de
  plusieurs façons, et c'est celle qu'on n'a pas listée qui survit.
- 🔴 **TOUCHER UNE LIGNE D'UN TABLEAU DE COMPTES OBLIGE À REMESURER SON
  COMPTE**, même quand ce n'est pas l'objet de l'édition — deux fois, la main
  était **sur la ligne même** qui portait le chiffre faux. **Mesurer chaque
  ligne d'une table au moment où on l'écrit**, jamais de mémoire.
- 🔴 **RELEVER LES TAILLES APRÈS LA DERNIÈRE ÉDITION DE LA RONDE**, revue
  transverse comprise : une table mesurée en début de ronde est fausse à la fin
  de la même ronde.
- 🔴 **LA DURÉE DE VIE D'UN « CELA RESTE VRAI » EST D'UN SOUS-BLOC.** Mesuré :
  le même en-tête a été corrigé **trois fois, une par sous-bloc**, chaque
  correction laissant une « moitié qui reste vraie » que la suivante a dû
  reprendre.
- 🔴 **UN NUMÉRO DE LIGNE DANS `CLAUDE.md` EST FAUX DÈS QU'ON ÉCRIT AU-DESSUS —
  et on écrit toujours au-dessus. NOMMER LA CHOSE, jamais compter les lignes
  qui l'en séparent.** Idem dans le code quand le chantier **déplace** la ligne
  citée : relire la citation **après** avoir exécuté ce qui la déplace.
- 🔴 **UN MESSAGE DE COMMIT EST UNE PIÈCE DU DÉPÔT, ET PERSONNE NE LE RELIT.**
  Sept écarts sur neuf n'existaient que là. **Relire les journaux contre le
  message, jamais l'inverse.**
- 🔴 **UNE REVUE PAR TÂCHE NE PEUT PAS VOIR UN DÉFAUT QUI FRANCHIT UNE
  FRONTIÈRE DE TÂCHE** — chacun est correct des deux côtés pris séparément.
  Ce n'est pas un défaut de rigueur, c'est une propriété du découpage, et c'est
  ce qui justifie la **revue transverse de fin de branche** (elle a trouvé
  jusqu'à **vingt-sept** affirmations devenues fausses en une branche).
- ⚠️ **CORRIGER UNE AFFIRMATION FAUSSE PEUT EN PRODUIRE UNE AUTRE** : inscrire
  dans le commentaire les commandes qui l'établissent, pour que le prochain
  lecteur refasse le contrôle sans croire personne.
- 🔴 **UNE PREUVE NE DOIT JAMAIS VIVRE DANS UN RAPPORT GITIGNORÉ.** L'espace de
  travail d'un sous-bloc a disparu avec sa session, emportant **six constats de
  revue définitivement perdus**.
- 🔴 **NE JAMAIS FABRIQUER UNE PIÈCE.** Deux transcriptions ont été assemblées à
  la main et présentées comme des relevés : les faits rapportés étaient
  **vrais**, les preuves ne l'étaient pas — donc invérifiables par le suivant.
  Le mécanisme nommé : **réutiliser la sortie d'une commande pour répondre à la
  question d'une AUTRE, sans la relancer.**

### Taille des fichiers

- 🔴 **EXTRAIRE, JAMAIS COMPRIMER.** Le plafond a été franchi une douzaine de
  fois ; il n'a été rattrapé par une compression que **deux** fois, et ce
  fichier l'interdit désormais nommément. **La forme forte est l'extraction
  jouée dans une tâche DÉDIÉE, AVANT celle qui ajoute.**
- 🔴 **LA MARGE REGAGNÉE PAR UNE EXTRACTION SE REPERD SI ON LA TRAITE COMME
  ACQUISE** — payé **six fois**, dont une le jour même dans la branche qui
  l'avait gagnée.
- 🔴 **UNE ADDITION DE COMMENTAIRE PEUT ANNULER UNE EXTRACTION**, et **la revue
  transverse est elle-même une source de croissance** (jusqu'à +54 lignes).
- ⚠️ **UNE EXTRACTION N'EST JAMAIS RIGOUREUSEMENT VERBATIM** : elle laisse ses
  imports derrière elle (d'une famille **autre** que `dead_code` — un `TS6133`
  est un **échec** de `tsc`), déplace les visibilités (un `pub(super)` sans son
  type rend `private_interfaces`), casse les déictiques (« plus bas dans ce
  fichier »), et **déplace ce qu'un garde d'absence surveille**.

### Outillage VM et Windows

- 🔴 **LA VM S'ÉTEINT SEULE, PAR DEUX MÉCANISMES DISTINCTS — les confondre fait
  chercher du mauvais côté.** ① Une hibernation initiée **DANS l'invité**
  (`Kernel-Power` 187/42, QEMU se terminant ~5 s après) ; ② l'**HÔTE** qui tue
  QEMU — `terminating on signal 15 from pid <N>`, ce PID étant
  `libvirtd --timeout 120`, qui s'arrête sur inactivité et **emporte le
  domaine**. **Vérifier `virsh list --all` après toute séquence longue**, et le
  compteur d'extinctions avant/après.
- 🔴 **LA TAILLE DU BINAIRE NE PROUVE RIEN, DANS LES DEUX SENS** : deux
  compilations de la **même** source rendent deux tailles, et un binaire neuf
  peut peser **exactement** autant que celui qu'il remplace — voire **moins**
  en ajoutant du code. Ce qui tranche est **une chaîne qu'on a posée soi-même**,
  cherchée sur le chemin que `run-agent.sh` lance, **avec son témoin négatif et
  une chaîne préexistante**. ⚠️ Une chaîne **courte** peut être inlinée et
  coupée aux frontières de mot ; une constante **sans appelant** est éliminée.
- 🔴 **UN AGENT SURVIVANT TIENT `agent.log`**, et l'on relit alors le journal de
  la tentative **précédente** en croyant lire le sien. **`Get-Process agent`
  avant CHAQUE tentative, y compris échouée** — c'est aussi ce qui fait échouer
  `link.exe` en 1104 / `os error 5`.
  🔴 **ET IL EN EXISTE PLUSIEURS : le journal le plus récent n'est pas
  forcément celui qu'on cherche.** Un lot qui monte sa propre recette écrit
  ailleurs (`C:\nivuus\lot31\agent-lot31.log`), et la dernière occurrence
  d'une erreur dans `agent.log` peut être **la sienne propre**, vieille d'une
  heure. **Trancher sur une DONNÉE du relevé** — le viewport demandé, le
  numéro de session —, jamais sur la seule date.
  🔴 **UN MARQUEUR ÉCRIT PAR `Add-Content` DANS UN JOURNAL TENU PAR UN AGENT
  VIVANT NE S'ÉCRIT PAS.** ⚠️ **La rédaction précédente disait
  « SILENCIEUSEMENT PERDU … `Add-Content` rend la main sans erreur » : MESURÉ
  FAUX le 5 septembre 2026** — l'appel **LÈVE**, « The process cannot access
  the file 'C:\nivuus\agent.log' because it is being used by another
  process ». Le remède ne change pas ; la description du symptôme, si — on
  **voit** l'échec, on ne le devine pas. Poser un marqueur **quand l'agent est ARRÊTÉ** ; s'il tourne,
  **segmenter par HORODATAGE**. Le symptôme est un « bras » qui compte tout le
  journal.
- 🔴 **UN RELEVÉ WinRM EST CELUI DE LA SESSION 0**, jamais de la session
  interactive : identité, intégrité, presse-papier, audio et périphériques y
  diffèrent. **Tout ce qui dépend de la session passe par une tâche planifiée
  `/it`**, dont la sonde doit **imprimer sa session** plutôt que la supposer.
- 🔴 **`build-agent.sh` RSYNCHRONISE L'ARBRE ENTIER**, travail non commité des
  voisins compris : il a échoué sur un module déclaré dont le fichier n'était
  pas suivi. **Vérifier `git status --porcelain agent/ proto/` avant CHAQUE
  build, pas seulement le premier**, ou bâtir depuis un `git worktree` isolé —
  ⚠️ avec `node_modules` lié, sans quoi le script s'arrête en silence.
- 🔴 **UNE VARIABLE NEUVE DOIT ÊTRE AJOUTÉE À `scripts/run-agent.sh` PAR UNE
  TÂCHE DÉDIÉE** — payé en D1 (`SUPERVISEUR`), D2 (`MULTIFENETRE_REPRISE`),
  D7 (`AUDIO`) et **lot 32 (`SORTIE_DESIGNEE`)**, où implémenteur **et**
  relecteur avaient vérifié la propriété **en traçant le code** : le tracé
  était juste, la valeur ne pouvait pas atteindre le processus.
  🔴 **LIRE LA LIGNE DANS LE SCRIPT GÉNÉRÉ NE SUFFIT PAS — LE LOT 32 L'A
  MESURÉ.** Une ligne peut être présente dans le `.ps1` **et n'être jamais
  exécutée** : posée APRÈS l'invocation de l'agent, elle n'atteint rien, et le
  fichier semble parfaitement bon. **Ce qui compte est sa POSITION RELATIVE À
  L'INVOCATION** — et le seul contrôle qui ne peut pas mentir est **la trace
  émise par le processus lui-même** (un `warn!` de désarmement, un champ de
  journal), parce qu'elle n'existe que si la valeur est arrivée. ⚠️ **Une
  rouge de banc dont la variable n'atteint pas le processus est VACUEUSE et
  se lit exactement comme une bonne** : elle rougit, pour la mauvaise raison.
- ⚠️ **`nodejs-winrm` ENVELOPPE TOUJOURS LA COMMANDE** dans
  `powershell -Command "& { … }"` : un script en ligne portant guillemets ou
  parenthèses entre en collision, **et le symptôme est un script qui ne tourne
  jamais**. Écrire le script sur le partage et l'invoquer par `-File`.
  ⚠️ Il **rend la main dès la première ligne et tue le processus distant** :
  rediriger tous les flux vers un fichier, et le lire depuis l'hôte.
- ⚠️ **UN `.ps1` SANS BOM CONTENANT UN SEUL CARACTÈRE NON-ASCII NE S'ANALYSE
  PAS**, et l'erreur désigne une **autre** ligne. Contrôle en une ligne :
  `LC_ALL=C grep -c '[^ -~]' fichier.ps1` doit rendre **0**.
- ⚠️ **LE DÉFAUT À DEUX RÉGLAGES, TOUJOURS NON CORRIGÉ** dans
  `build-agent.sh` / `run-agent.sh` : un journal PowerShell lisible demande
  `[Console]::OutputEncoding` (**lecture**) **ET** un `StreamWriter` UTF-8
  (**écriture**). Symptôme : un `grep` sur un mot accentué rend 0 quand le même
  `grep` sur sa partie ASCII rend 1.
- ⚠️ **UNE SORTIE VIRTUELLE ET UNE RACINE ProjFS SURVIVENT À UN ARRÊT BRUTAL** :
  le `Drop` ne court pas sur un `TerminateProcess`. **Purger
  (`MULTIFENETRE_VDD_PURGE=1`) entre deux exécutions.**
- ⚠️ **COMPTER LES FENÊTRES, JAMAIS LES LANCEMENTS** : Paint en ouvre deux,
  Bloc-notes fait avancer le compteur de sessions de deux, et Chrome rejoint
  son instance existante sans `--user-data-dir` distinct.
