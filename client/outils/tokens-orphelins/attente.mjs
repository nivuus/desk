// La LISTE D'ATTENTE du contrôle §7.6, et TOUTE la doctrine qui la justifie.
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
// LA LISTE D'ATTENTE — 10 tokens déclarés que le produit n'appelle pas ENCORE.
//
// ⚠️ CE NOMBRE EST TENU À JOUR PAR LA TÂCHE QUI LE REND FAUX, jamais par une
// tâche de ménage plus tard : elle en avait 28 à la fin de S1, et les six
// commits de S2 l'ont ramenée à 10 — chacun retirant, DANS SON PROPRE COMMIT,
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
// nommée dont chaque retrait est FORCÉ par le contrôle rétrécit toute seule,
// et le jour où elle est vide, tout ce bloc disparaît avec elle.
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
// ⚠️ PARTIELLE, et le mot est pesé : elle juge le SOUS-BLOC NOMMÉ, jamais le
// CONTENU de l'annotation — « S3 — la gouttière entre cartes » changé en
// « S3 — n'importe quoi » lui échapperait —, et elle exige que le dépôt sache
// quel sous-bloc est courant, ce qu'aucun fichier ne dit aujourd'hui. Elle n'est
// pas construite ici : hors du périmètre de S2, et LÉGUÉE.
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
// même commande, sur les dix entrées ci-dessous :
//
//   → paires totales : 52 | citant un token en attente : 0
//
// **ZÉRO — et ce zéro dit l'inverse de ce qu'on croirait y lire.** Il ne réfute
// pas S1 : il montre que la décision de S1 a TENU JUSQU'AU BOUT. Aucune des
// quatorze couleurs par thème n'est plus orpheline ; les dix entrées restantes
// sont typographiques, d'espacement, de rayon et de police, et les paires de
// contraste ne citent que des couleurs. Élaguer en S1 aurait retiré des
// couleurs que S2 emploie aujourd'hui.
// ⚠️ ET CELA RETIRE SON ARGUMENT À CE BLOC-CI : « §7.1 tomberait de 50 paires
// à 4 » NE PROTÈGE PLUS RIEN, puisqu'un élagage n'atteindrait plus aucune
// couleur. Ce qui protège les dix restants n'est plus qu'une chose — leur
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
const EN_ATTENTE_D_APPELANT = new Map([
    // ── Famille ÉTIQUETTE / PASTILLE — RE-TAGUÉE S2 → S3+ par S2 (tâche 8) ─
    ['--t-xs', 'S3 ou plus tard — la mention légale et les étiquettes'],
    ['--e-1', 'S3 ou plus tard — l’écart interne d’une étiquette'],
    ['--r-plein', 'S3 ou plus tard — les pastilles et les boutons ronds'],
    // ── Typographie, interligne et espacement — S3 ────────────────────────
    ['--t-2xl', 'S3 — le titre de l’écran de connexion'],
    ['--t-3xl', 'S3 — le titre du hub'],
    ['--lh-large', 'S3 — les paragraphes longs'],
    ['--e-5', 'S3 — la gouttière entre cartes'],
    ['--e-6', 'S3 — la marge des sections'],
    ['--e-7', 'S3 — la marge de tête des surfaces'],
    // ── Le cas particulier, et il est nommé ───────────────────────────────
    // 🔴 `--police-mono` N'A QU'UN SEUL APPELANT PRÉVU, `#stats`, et la spec
    // §4.3 laisse son sort ouvert : « si aucun appelant n'apparaît, le token
    // sort ». Il N'A PAS été câblé par S1, et pas par oubli : `#stats` hérite
    // aujourd'hui de `--police-ui`, si bien que lui poser la pile monospace
    // CHANGERAIT SON APPARENCE — ce que S1 s'interdit nommément (« visuellement
    // quasi neutre sur index.html »). C'est donc S4, le sous-bloc qui a le
    // droit de toucher la fenêtre de session, qui tranche : ou il le câble, ou
    // il le retire. Aucun autre sous-bloc n'a le droit de laisser cette ligne
    // en place sans décider. ⚠️ S2 NE L'A PAS ROUVERT, et pas par omission :
    // aucune de ses quatre familles n'a besoin d'une pile monospace.
    ['--police-mono', 'S4 — #stats, OU RETRAIT : le seul token dont le sort est encore ouvert'],
]);

export { EN_ATTENTE_D_APPELANT };
