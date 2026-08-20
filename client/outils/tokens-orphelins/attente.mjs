// La LISTE D'ATTENTE du contrôle §7.6, et TOUTE la doctrine qui la justifie.
//
// ⚠️ `SOUS_BLOCS_CLOS` N'EST PLUS ICI — extrait vers `sous-blocs-clos.mjs` par
// la tâche 6 de S4, AVEC toute sa doctrine, parce que retirer la dernière
// entrée de cette liste a fait GROSSIR ce fichier de 227 à 243 pour un seuil
// d'extraction à 240. Le fichier voisin porte la mesure et la raison.
//
// 🔴 EXTRAIT DE `tokens-orphelins.mjs` PAR LA TÂCHE 8 DE S2, ET LA DOCTRINE EST
// PARTIE AVEC SA DONNÉE — c'est le geste que `CLAUDE.md` exige nommément
// (« extraire, jamais compresser » ; `serveur/instances.rs` a emporté `TAMPON`
// avec le commentaire qui le justifie). Ce qui a forcé l'extraction est mesuré,
// et la mesure est écrite ici plutôt qu'ailleurs :
//
//   Le plan S2 attendait de la tâche 8 un `tokens-orphelins.mjs` PLUS COURT
//   qu'à `56b975a` (233 lignes), les 18 entrées retirées de cette liste devant
//   le faire maigrir. Relevé par la commande le 20 août 2026, AVANT extraction :
//
//     entrées de liste  28 → 10   (−18)
//     commentaires     104 → 153  (+49)
//     code              87 →  93  (+6 : la seconde exclusion et `--sans-exclusion`)
//     lignes vides      14 →  14
//     TOTAL            233 → 270  (+37)
//
//   **Les 18 entrées retirées ont été plus qu'annulées par du commentaire** —
//   en petit, la leçon que ce dépôt a payée en grand : « une addition de
//   commentaire peut annuler une extraction ». Et la première rédaction du
//   constat ci-dessus, écrite EN TÊTE DE `tokens-orphelins.mjs`, a porté le
//   fichier à **296**, marge 4 : un encadré qui dénonçait la dérive la
//   produisait. Aucun des trois blocs neufs n'est du remplissage — la raison
//   MESURÉE de la seconde exclusion (tâche 7), l'encadré des re-tags que rien
//   ne contrôle, et le relevé de S1 refait au lieu d'être effacé —, et les
//   raboter aurait échangé une vérité contre un nombre.
//
// ⚠️ CE FICHIER N'A PAS DE TEST, et il n'en a pas besoin : il ne porte AUCUNE
// logique, seulement une donnée et sa justification. Ce qui l'emploie est
// `tokens-orphelins.mjs`, dont le contrôle échoue dans les deux sens.
// ═══════════════════════════════════════════════════════════════════════════
// LA LISTE D'ATTENTE — ELLE EST VIDE DEPUIS LE SOUS-BLOC S4, TÂCHE 6.
//
// 🔴 ET ELLE NE DISPARAÎT PAS POUR AUTANT : L'ÉNONCÉ DE S1 QUI LE PROMETTAIT
// EST CORRIGÉ PLUTÔT QU'EXÉCUTÉ. « Le jour où elle est vide, tout ce bloc
// disparaît avec elle » — écrit plus bas par S1, et FAUX. Supprimer ce fichier
// supprimerait l'ÉGALITÉ elle-même : c'est elle qui fait rougir
// `NOUVEL ORPHELIN` pour tout token futur déclaré sans appelant, et S4 en
// déclare QUATRE de plus (`--sur-voile`, puis les trois tokens de contenant).
// Une `Map` vide est ce qui rend ce contrôle STRICT ; la supprimer le rendrait
// muet. La clause ③ (`SOUS_BLOCS_CLOS`) continue de mordre pour la même
// raison, et c'est la troisième rouge de la tâche 6.
//
// La doctrine part AVEC sa donnée — règle d'extraction que ce fichier porte
// déjà —, et le fichier MAIGRIT : 227 → voir le message du commit.
//
// ⚠️ CE NOMBRE EST TENU À JOUR PAR LA TÂCHE QUI LE REND FAUX, jamais par une
// tâche de ménage plus tard : elle en avait 28 à la fin de S1, et les QUATRE
// commits de famille de S2 l'ont ramenée à 10 — chacun retirant, DANS SON COMMIT,
// exactement les entrées que le contrôle venait de nommer « À RETIRER DE LA
// LISTE ». Un compte qui n'appartient à personne dérive — ce dépôt l'a payé
// assez souvent.
//
// 🔴 CE N'EST PAS UN ASSOUPLISSEMENT DU CONTRÔLE, ET LA DIFFÉRENCE TIENT À UN
// MOT : ÉGALITÉ, pas inclusion. Le contrôle exige que l'ensemble des orphelins
// soit EXACTEMENT cette liste. Il échoue donc dans LES DEUX SENS :
//
//   • un token orphelin absent de la liste  → « nouvel orphelin »   (rouge)
//   • un token de la liste qui a un appelant → « à retirer d'ici »  (rouge)
//
// La seconde moitié est celle qui compte : elle rend la liste AUTO-NETTOYANTE.
// Un seuil (« au plus N orphelins ») aurait pourri sur place ; une liste
// nommée dont chaque retrait est FORCÉ par le contrôle rétrécit toute seule.
// ❌ « ET LE JOUR OÙ ELLE EST VIDE, TOUT CE BLOC DISPARAÎT AVEC ELLE » — écrit
// ici par S1, RÉFUTÉ par S4 (tâche 6), le jour même où elle s'est vidée : voir
// l'encadré de tête. C'est l'égalité qui vaut, pas la liste, et l'égalité a
// besoin de ce fichier.
//
// ── 🔴 RE-TAGUER UNE ENTRÉE N'EST VU PAR AUCUN CONTRÔLE ────────────────────
// Le contrôle compare des ENSEMBLES DE NOMS. Changer « S2 » en « S3 » dans une
// annotation ne déclenche rien, dans aucun des deux sens, jamais. C'est le
// point le plus faible de ce dispositif, et la porte par laquelle on
// assouplirait la liste sans qu'aucune commande ne le dise.
// RÈGLE DE REVUE, faute de mieux : TOUTE ANNOTATION MODIFIÉE PORTE SA RAISON
// ET LE SOUS-BLOC QUI L'A MODIFIÉE. Trois l'ont été par S2 (tâche 8, 20 août) —
// `--t-xs`, `--e-1`, `--r-plein` —, et la raison est la même pour les trois :
// elles décrivent une famille ÉTIQUETTE / PASTILLE que le §6 de la spec ne
// confie PAS à S2. S2 est borné à quatre familles — bouton, champ, surface,
// message —, « ce dont un écran de connexion a besoin », et un écran de
// connexion n'a ni étiquette ni pastille. S1 avait prédit S2 ; la prédiction
// était fausse. Fabriquer une pastille dans le seul but de vider trois lignes
// aurait été vider un contrôle pour en verdir un autre — le geste même que
// l'encadré ci-dessus refuse.
// ❌ « AUCUNE MITIGATION TECHNIQUE N'EST POSSIBLE : un contrôle sur ces chaînes
// de prose ne pourrait pas échouer utilement » — le plan de S2 l'écrit trois
// fois (l. 369, l. 1379-1380, table des risques), et c'est FAUX. Une mitigation
// PARTIELLE existe, et elle POURRAIT échouer utilement : *aucune entrée ne doit
// nommer un sous-bloc déjà clos*. Elle serait passée au rouge à la fin de S2 sur
// les trois entrées annotées « S2 », FORÇANT la décision au lieu de la laisser à
// une règle de revue.
// ✅ ELLE EST CONSTRUITE — SOUS-BLOC S3, TÂCHE 7. `SOUS_BLOCS_CLOS` ci-dessous
// la nomme, et `tokens-orphelins.mjs` la fait échouer. Le sous-bloc qui la bâtit
// est celui qui s'apprêtait à en avoir besoin : S3 a re-étiqueté DEUX entrées
// (`--t-2xl` et `--t-3xl`, interverties par rapport à la spec, la seconde
// nommant un hub que ⑥ ne livre pas), et construire un garde-fou dans le
// sous-bloc qui va s'en servir est la seule façon de savoir qu'il mord.
// ⚠️ PARTIELLE, et le mot reste pesé : elle juge le SOUS-BLOC NOMMÉ, jamais le
// CONTENU de l'annotation — « S4 — la gouttière entre cartes » changé en
// « S4 — n'importe quoi » lui échappe —, et elle DÉPEND D'UNE LISTE TENUE À LA
// MAIN : un sous-bloc qui ne s'y déclare pas la neutralise. C'est une règle de
// revue de plus, et elle est déclarée plutôt que dissimulée.
// ⚠️ ET APRÈS S3 ELLE NE GARDE QU'UNE ENTRÉE — un mécanisme pour une ligne.
// C'est une objection, et voici la réponse : cette ligne-là est précisément
// celle dont la prose dit qu'« aucun sous-bloc n'a le droit de la laisser en
// place sans décider », une injonction que RIEN n'appliquait ; et une
// mitigation construite APRÈS la faute qu'elle devait empêcher n'aurait plus
// rien à empêcher.
//
// ── POURQUOI CETTE PALETTE N'EST PAS SIMPLEMENT RÉDUITE À CE QUI SERT ──────
// C'était la voie évidente, et elle est REFUSÉE SUR MESURE, prise le 19 août
// 2026 (relevé S1) : sur les 50 paires de contraste déclarées du §4.5 que le
// contrôle §7.1 vérifie, **46 citent au moins un token de cette liste**.
// Élaguer la palette pour verdir §7.6 ferait tomber §7.1 de 50 paires à 4 — on
// satisferait un contrôle en vidant l'autre, ce qui est exactement le geste que
// ce dépôt combat. Relevé par la commande :
//
//   node --input-type=module -e "import {PAIRES} from './src/design/contraste.ts'; …"
//   → paires totales : 50 | paires citant au moins un token sans appelant : 46
//
// 🔴 CE RELEVÉ EST DATÉ DE S1, ET IL EST FAUX AU PRÉSENT — il est laissé DATÉ
// plutôt qu'effacé, parce qu'un relevé daté reste vrai comme histoire et que
// c'est lui qui a fondé la décision. REFAIT PAR S2 (tâche 8) LE 20 AOÛT 2026,
// même commande, sur les DIX entrées d'alors (S3 les a ramenées à UNE) :
//
//   → paires totales : 52 | citant un token en attente : 0
//
// **ZÉRO — et ce zéro dit l'inverse de ce qu'on croirait y lire.** Il ne réfute
// pas S1 : il montre que la décision de S1 a TENU JUSQU'AU BOUT. Aucune des
// quatorze couleurs par thème n'est plus orpheline ; les dix entrées d'alors
// étaient typographiques, d'espacement, de rayon et de police, et les paires de
// contraste ne citent que des couleurs. Élaguer en S1 aurait retiré des
// couleurs que S2 emploie aujourd'hui. ⚠️ LE ZÉRO TIENT APRÈS S3, ET SANS ÊTRE
// REFAIT : `--police-mono`, seule entrée restante, n'est pas une couleur.
// ⚠️ ET CELA RETIRE SON ARGUMENT À CE BLOC-CI : « §7.1 tomberait de 50 paires
// à 4 » NE PROTÈGE PLUS RIEN, puisqu'un élagage n'atteindrait plus aucune
// couleur. Ce qui protège l'entrée restante n'est plus qu'une chose — son
// annotation, et le sous-bloc qui la porte. Voir l'encadré des re-tags.
//
// ⚠️ CE CONTRÔLE EST DONC ROUGE PAR CONSTRUCTION JUSQU'À S4 SI ON LE PREND
// COMME MESURE DE « la palette est-elle entièrement employée ? ». Ce n'est pas
// ce qu'il mesure. Ce qu'il mesure, à partir de S1, c'est que **l'écart entre
// la palette et son emploi soit CONNU, ÉNUMÉRÉ ET DÉCROISSANT** — et cela, il
// peut l'échouer dès aujourd'hui, dans les deux sens.
//
// Chaque entrée nomme le sous-bloc qui la consommera. Relevé le 20 août 2026,
// au commit de la tâche 8 du sous-bloc S2.
// ═══════════════════════════════════════════════════════════════════════════
//
// 🔴 SEPT ENTRÉES SONT SORTIES À LA TÂCHE 4 DU SOUS-BLOC S3, dans le commit
// même qui a écrit leurs appelants — `client/src/shell.css`, la feuille de la
// page-shell. Le contrôle exige l'ÉGALITÉ : les retirer sans écrire l'appelant
// aurait rendu `NOUVEL ORPHELIN`, l'écrire sans les retirer
// `À RETIRER DE LA LISTE`, et les deux sens ont été vus rouges en S1.
//
//   --t-2xl    le titre de la page-shell        (.bureau__titre)
//   --e-5      la gouttière entre cartes        (.bureau__fenetres)
//   --e-6      la marge des sections            (.bureau__section)
//   --e-7      la marge de tête de la surface   (.bureau)
//   --t-xs     l'étiquette de la pastille       (.bureau__pastille)
//   --e-1      son écart interne                (.bureau__pastille)
//   --r-plein  sa forme                         (.bureau__pastille)
//
// ⚠️ LA FAMILLE ÉTIQUETTE / PASTILLE ÉTAIT RE-TAGUÉE « S3 ou plus tard » PAR
// S2, faute de savoir si une pastille existerait. Elle existe : c'est l'état
// ouverte / fermée d'une fenêtre, dit par l'ENCRE et jamais par un fond, pour
// que son contraste reste dans les 52 paires mesurées du §7.1.
// ═══════════════════════════════════════════════════════════════════════════
//
// 🔴 DEUX ENTRÉES DE PLUS SONT SORTIES À LA TÂCHE 5 DE S3, avec leurs appelants
// dans `client/src/connexion.css` :
//
//   --t-3xl     le titre de l'écran de connexion  (.connexion__titre)
//   --lh-large  l'interligne de son bandeau       (.connexion__message)
//
// ⚠️ ET LEURS DEUX ANNOTATIONS ÉTAIENT FAUSSES, CHACUNE À SA FAÇON — corrigé
// ici plutôt que recopié, comme la règle de revue de ce fichier l'exige.
//
//   ① `--t-2xl` et `--t-3xl` ÉTAIENT INTERVERTIS par rapport à la spec. Le
//      §4.4 de la spec écrit « --t-2xl … titre de page » et « --t-3xl … titre
//      d'écran de connexion » ; cette liste disait l'inverse. LA SPEC L'EMPORTE
//      — le cran de 32 px va au titre que la page n'a qu'une fois, celui de
//      24 px au titre d'une page qui porte des sections sous lui.
//   ② `--t-3xl` NOMMAIT « le titre du hub », c'est-à-dire une surface que le
//      sous-projet ⑥ NE LIVRE PAS : sa spec §6 l'écrit en toutes lettres, « le
//      hub ne figure PAS dans ce découpage ». L'entrée attribuait donc à S3 un
//      appelant que S3 ne pouvait pas écrire, et elle serait restée en attente
//      pour toujours si la spec n'avait pas tranché.
//
// ⚠️ `--lh-large` ÉTAIT LE PLUS FRAGILE DES NEUF, et il n'a PAS été consommé
// pour vider une ligne : le bandeau de l'écran de connexion porte les plus
// longues proses du produit — motif de refus, état de la VM, aveu de
// non-redémarrage, cause réseau citée en entier. Le paragraphe long existait
// déjà ; en fabriquer un aurait été vider un contrôle pour en verdir un autre.
// ═══════════════════════════════════════════════════════════════════════════

/**
 * ⚠️ LE SOUS-BLOC EST UN CHAMP STRUCTURÉ, IL N'EST PLUS NOYÉ DANS UNE PHRASE.
 * C'est ce qui rend la mitigation possible : tant que « S4 » n'était qu'un
 * préfixe de prose, aucune commande ne pouvait le lire sans deviner. Le champ
 * `raison` porte le reste, et lui reste hors de toute portée automatique.
 */
/**
 * ⚠️ VIDE DEPUIS S4, TÂCHE 6, ET C'EST UN ÉTAT NORMAL — pas une invitation à
 * supprimer ce fichier. Voir l'encadré de tête : c'est l'ÉGALITÉ qui vaut.
 *
 * 🔴 LA DERNIÈRE ENTRÉE SORTIE EST `--police-mono`, et sa doctrine est partie
 * AVEC elle. Ce qu'il faut en garder tient en trois lignes, parce que le fait
 * vaut plus que la prose : trois sous-blocs se sont passé « le câbler ou le
 * retirer » faute d'avoir le droit de changer l'apparence de la fenêtre de
 * session ; S4 l'a, et il l'a CÂBLÉ sur `#stats` (`client/src/style.css`), que
 * la spec §4.3 désignait comme son unique appelant prévu depuis S1.
 */
const EN_ATTENTE_D_APPELANT = new Map([]);

export { EN_ATTENTE_D_APPELANT };
