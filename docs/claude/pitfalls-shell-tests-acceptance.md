# Pièges transverses — shell de l'hôte, tests et types, limites du montage de recette

> Extrait de [`CLAUDE.md`](../../CLAUDE.md) (lignes 870-1018 de l'original), déplacé tel quel. Liens relatifs à ce dossier.

### Shell de l'hôte

- 🔴 **UN SERVICE LAISSÉ PAR UNE EXÉCUTION PRÉCÉDENTE RÉPOND À LA PLACE DU
  VÔTRE, ET LE SYMPTÔME EST UN `404` QU'ON LIT COMME UN DÉPÔT RATÉ.** Payé
  **deux fois** sur `python3 -m http.server` (lots 32Q et 32T). Cause prouvée
  par PID et ligne de commande : `(cd D && python3 … & echo $!)` met le
  `cd && python3` **entier** en arrière-plan, donc `$!` désigne le
  **sous-shell** — le `kill` le tue, le serveur survit, le suivant ne peut plus
  se lier au port, et c'est **l'ancien** qui répond, depuis l'autre répertoire.
  **Ne jamais mettre un `cd` dans la commande qu'on met en arrière-plan**
  (`--directory`, `--chdir`, un chemin absolu), **libérer le port par PID
  relevé** avant de servir, et **juger sur ce qui est arrivé à destination** —
  ici la comparaison des deux sha256, qui a attrapé le défaut les deux fois là
  où aucun code de retour ne le pouvait.
- 🔴 **`pkill -f <motif>` DEPUIS UN SHELL DONT LA LIGNE DE COMMANDE CONTIENT LE
  MOTIF TUE LE SHELL** (exit 144, la suite de la chaîne ne s'exécute pas).
  Payé **au moins quatre fois**. **Tuer par PID relevé**, jamais par motif — et
  jamais par une heuristique de rang : l'ordre est superviseur, capteur, pont,
  **puis** les enfants.
- 🔴 **ZSH** : `"$var:suffixe"` applique un modificateur d'historique et **mange
  la valeur** (`$P:e2-v1` rend `2-v1`) ; zsh **ne découpe pas** les variables en
  mots, donc `git add $FICHIERS` passe la liste entière comme **un seul
  chemin**. **Toujours `${var}`, et des arguments littéraux.**
- 🔴 **DES BACKTICKS DANS UN `echo` DE JOURNAL EXÉCUTENT UNE COMMANDE**, et un
  `git commit -m` portant des accents graves **mutile les phrases**. **Quotes
  simples pour toute prose ; message long par un fichier (`-F`).**
- 🔴 **`git checkout -- <fichier>` RESTAURE HEAD, PAS L'ÉTAT D'AVANT LA
  MUTATION** : il a effacé du travail non commité. **Une rouge se restaure
  depuis une COPIE NOMMÉE**, et son garde « la mutation a-t-elle changé quelque
  chose ? » doit comparer **à cette copie**, jamais à HEAD (sinon il ne peut
  pas échouer).
- 🔴 **LA CHAÎNE DE REMPLACEMENT DE `-replace` TRAITE `$` COMME UN RENVOI DE
  GROUPE — ET ELLE A CORROMPU LE FICHIER DE L'INVITÉ.** Payé le 5 septembre
  2026 (item 8) : un `-replace` censé écrire `$env:SUPERVISEUR = '0'` a produit
  **`$$env:SUPERVISEUR   = 0`** — dollar doublé, guillemets perdus. L'agent
  relancé n'a plus rien journalisé (**segment d'UNE ligne**, 0 capteur, 0
  enrôlement), le bras a dû être **rejeté**, et `run-agent.ps1` **réparé sur la
  VM**. ⚠️ **Le symptôme ne ressemble pas à une corruption** : il ressemble à
  un agent qui ne fait rien. 🔵 **Le remède est `.Replace()`, la méthode de
  `String` — littérale, sans regex ni sémantique du `$`** — et le contrôle qui
  vaut est de **relire la ligne écrite**, jamais de supposer qu'elle l'est.
- 🔴 **MUTER PAR NUMÉRO DE LIGNE OU PAR UN MOTIF ANCRÉ SUR LA SYNTAXE, JAMAIS
  PAR UNE SOUS-CHAÎNE** : dans un dépôt qui commente ses invariants, la
  substitution frappe **le commentaire avant le code** — et une ligne peut
  exister cinq fois. `assert texte.count(ancre) == 1`.
- ⚠️ **LE HOOK `chpwd` DU SHELL HÔTE INJECTE UN `ls` DANS CHAQUE JOURNAL** dès
  qu'un `cd` court dans un sous-shell : `unset -f chpwd` avant toute collecte.
- ⚠️ **`grep -qa $'\000'` CHERCHE LA CHAÎNE VIDE ET MATCHE TOUT** : un contrôle
  d'octets NUL écrit ainsi ne peut pas échouer.
- 🔴 **`sed 's/[^ -~]//g'` SANS `LC_ALL=C` MANGE DES LETTRES ASCII** — la plage
  ` -~` est dépendante de la **collation** de la locale, pas des octets. Payé
  au lot 32O : un `.ps1` « nettoyé » de son unique caractère non-ASCII est
  ressorti avec `[System.Diagnostics.Process]::GetCurrentProcess()` réduit à
  `[..]::()`, et la sonde a échoué à l'analyse — **sans que rien ne dise que
  le fichier avait été mutilé**. ⚠️ Le contrôle `LC_ALL=C grep -c '[^ -~]'`,
  lui, est juste : c'est le `sed` qui doit porter `LC_ALL=C`, pas seulement le
  `grep` qui le vérifie. **Et le symptôme se lit comme une erreur de syntaxe
  du script**, jamais comme une corruption — c'est en LISANT la sortie plutôt
  qu'en la supposant qu'on le voit.
- ⚠️ **UN FICHIER DE CONTRÔLE PEUT SE POLLUER LUI-MÊME** : `echo` interprète
  `\x1b` et `\r`, si bien qu'un fichier qui **décrit** ses motifs les
  **contient**. Heredoc **cité**.

### Tests et types

- 🔴 **`cd client && npx vitest run` NE COUVRE PAS `proto/ts/`** — la racine
  Vitest est `client/`. **Deux commandes, jamais une**, et un test posé hors de
  `client/src/` ne tournerait pas sans que personne ne le voie.
- 🔴 **VITEST TRANSPILE SANS VÉRIFIER LES TYPES** (esbuild) : `tsc --noEmit` est
  une étape **distincte et obligatoire**. Elle seule attrape `Buffer` sans
  `@types/node`, ou un `Uint8Array` passé comme `BlobPart`.
- 🔴 **`expect(x).toBe(y, 'message')` EST SILENCIEUSEMENT IGNORÉ** par Vitest —
  attrapé par `tsc`, et par lui seul.
- 🔴 **TROIS ASSERTIONS DANS UN TEST NE PROUVENT QUE LA PREMIÈRE** : `expect`
  s'arrête au premier échec. Une assertion de **perte de données** placée en
  seconde position n'était éprouvée par **rien**.
- 🔴 **UNE SOURCE FACTICE QUI IMPLÉMENTE UN EFFET DE BORD EN NO-OP REND UNE
  FAMILLE ENTIÈRE DE DÉFAUTS INVISIBLE** : 456 tests verts sur un produit muet.
  De même, un test qui **normalise** ce qu'il éprouve (un `Number(...)`
  défensif) n'éprouve plus rien.
- 🔴 **UN `match` CATCH-ALL TUE UN FIL EN SILENCE.** `capteur/pont_media.rs`
  porte un `Ok(autre) => return` qui **tue `lire_le_media` sans panne
  apparente** : payé **SIX fois** (`Sommeil`, `Part`, `Audio`, `PleinEcran`,
  `PressePapier`, `Accent`). **À vérifier pour tout message neuf du capteur.**
  ⚠️ Son jumeau `transport/controle.rs` porte un `match` **exhaustif** : il se
  signale au compilateur, mais **le bras part dans le MÊME commit que la
  variante**, sinon l'arbre ne compile pas — un voisin qui bâtit depuis l'arbre
  partagé y perd sa mesure.
- 🔴 **UN COMPTE DE TESTS N'EST ATTRIBUABLE QU'ASSORTI DE SON COMMIT** quand
  plusieurs chantiers partagent l'arbre — mesuré : 107 → 120 en vingt-deux
  minutes, sans qu'aucune ligne du chantier ne bouge. **Mesurer avec
  `git show HEAD:` ou depuis un `git worktree`, et dater tout compte.**
- ⚠️ **NE JAMAIS `git add -A` DANS UN ARBRE PARTAGÉ** : un `git add -A agent/src`
  a emporté le travail concurrent d'une autre tâche, sans sa déclaration de
  module. **Nommer les fichiers** — et ⚠️ un pathspec de **répertoire** ne prend
  pas le module **homonyme** (`agent/src/pont` laisse `agent/src/pont.rs`).
- ⚠️ **UN `||` DE REPLI TRANSFORME « FICHIER ABSENT » EN « CONTRÔLE VERT ».**
- ⚠️ **LE NOM D'UN `grep` DE RECETTE SE VÉRIFIE CONTRE LE CODE, JAMAIS CONTRE LA
  SPEC** — et **sur le VERT**, avant de conclure quoi que ce soit du rouge.

### Ce que le montage de recette ne peut pas voir

- 🔴 **CE QU'UN NAVIGATEUR EXIGE, AUCUN TEST DE NODE NE LE VOIT** : deux défauts
  CORS rendaient deux routes inatteignables avec une suite entièrement verte.
  **Cette classe n'a AUCUN garde automatique dans ce dépôt.**
- 🔴 **UN CHROME SANS INTERFACE RAPPORTE `document.hidden = true` POUR TOUTE
  FENÊTRE D'ARRIÈRE-PLAN**, n'entre **pas** réellement en plein écran, n'expose
  **pas** `navigator.keyboard`, et n'a **aucun chemin acoustique**. **La
  visibilité et le focus sont donc IMPOSÉS par le pilote de recette, page par
  page** — limite héritée de D5, **qu'aucun sous-bloc n'a levée**.
  ⚠️ `window.open(url, nom)` ouvre un **ONGLET** ; avec une chaîne de
  caractéristiques, une **POPUP** — et cela change le comportement du focus.
  **Une sonde qui croit reproduire un geste doit le RELIRE, pas s'en souvenir.**
- 🔴 **`Page.addScriptToEvaluateOnNewDocument` NE COURT PAS sur une page ouverte
  par `window.open`** : poser l'amorce explicitement, page par page.
- 🔴 **UNE BOUCLE D'ATTENTE QUI NE DIT PAS CE QU'ELLE A VU REND UN ÉCHEC
  INDISCERNABLE D'UN PRODUIT EN PANNE.** Une boucle « aucune page de session »
  a coûté **deux créneaux d'interruption du propriétaire** avant qu'on ne lui
  fasse journaliser les cibles CDP **et l'état de la page** — laquelle portait
  la réponse en toutes lettres : *« Bureau refusé : un client est déjà connecté
  à la session … »*. **Faire dire à l'attente ce qu'elle observe**, pas
  seulement qu'elle a renoncé.
- 🔴 **LE RÔLE `client` EST EXCLUSIF PAR SESSION (lot 22) : AUCUNE RECETTE
  NAVIGATEUR NE PEUT ÊTRE JOUÉE PENDANT QUE LE PROPRIÉTAIRE EST CONNECTÉ**, et
  réciproquement un pilote qui se connecte **lui prend sa place**. Toute
  campagne visant cette VM doit donc être **annoncée et bornée dans le temps**,
  et son échec le plus probable n'est pas technique : c'est un humain déjà là.
  🔵 **LE REMÈDE, trouvé au lot 32C : `Target.setAutoAttach` avec
  `waitForDebuggerOnStart` au niveau NAVIGATEUR.** Chaque cible neuve pause
  avant son premier script ; on y pose l'amorce, puis `Runtime.
  runIfWaitingForDebugger` la relâche. ⚠️ **L'URL portée par
  `Target.attachedToTarget` est celle d'AVANT navigation (`about:blank`)** :
  apparier dessus ne marche jamais — **demander** son adresse à chaque session.
  🔴 **ET SURTOUT : NE PAS FERMER LA POP-UP POUR LA ROUVRIR.** L'agent voit
  alors partir son pair, démonte la session, et la page rouverte reste à
  `ice=new` — mesuré. L'amorce se pose **sans rien fermer**.
- 🔴 **TOUTE ÉVALUATION CDP SUR UNE PAGE PORTANT UN FLUX WebRTC ACTIF DOIT ÊTRE
  BORNÉE** : `Page.captureScreenshot` peut ne **jamais** rendre. Et
  `execFileSync` **bloque la boucle d'événements de Node**, donc le pilote cesse
  de lire son WebSocket CDP — **l'instrument détruit ce qu'il mesure**.
- 🔴 **UNE MIRE IMMOBILE NE PRODUIT AUCUNE IMAGE** : Desktop Duplication n'émet
  qu'au changement du bureau. **Animer la source, à une cadence CONNUE et
  affichée par la source elle-même.**
- ⚠️ **NE JAMAIS TRACER PAR PAQUET DANS LA BOUCLE DE TRANSPORT** : 18 619 lignes
  en quelques secondes sur un partage CIFS ont empêché une session de
  s'établir. **Compter ou échantillonner.**

---
