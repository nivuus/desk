# Le témoin de clôture de G5 — et pourquoi il sort en **1**

`env -u TURN_URL -u TURN_SECRET ./scripts/verify-all.sh`, relancé **après la
dernière édition de la ronde**, revue transverse comprise. Journal brut :
`22-verify-all-cloture.log`.

```
EXIT=1        ══ 2 étape(s) sur 10 en ÉCHEC ══
```

> 🔴 **PRÉSENTER CE FILET COMME VERT SERAIT FAUX ; LE PRÉSENTER COMME ROUGE LE
> SERAIT TOUT AUTANT.** C'est la leçon que le chantier E a payée. Le témoin
> nomme donc **quelle** étape échoue **et à qui elle appartient**.

## Les deux étapes en échec, et leur propriétaire

| Étape | Test | Propriétaire |
| --- | --- | --- |
| `cargo test --workspace` | `pont::ecriture::fil::tests::les_dues_sont_annoncees_avant_toute_poussee_au_demarrage`<br>`pont::ecriture::fil::tests::un_fichier_absent_au_redemarrage_sort_du_journal_en_le_nommant` | **F5** |
| `client : npm test` | `src/fichiers/protocole.annonces.test.ts` — « 🔴 ne répond RIEN, et appelle le rappel injecté » | **F5** |

**Attribution PAR LA COMMANDE, jamais par conviction** :

```
git log --oneline -1 -- agent/src/pont/ecriture/fil.rs
  -> a809341 pont(f5) : la QUATRIEME famille du protocole …
git log --oneline -1 -- client/src/fichiers/protocole.annonces.test.ts
  -> 018fb70 pont(f5) : extraction PREALABLE de protocole.annonces.test.ts
git status --porcelain agent/ client/
  -> DIX fichiers de `agent/src/pont/` modifiés et NON COMMITÉS
```

Les trois titres portent le marqueur **🔴** de la discipline rouge-d'abord du
dépôt, et F5 a du travail **en cours** dans les fichiers qu'ils éprouvent :
**ce sont ses rouges, pas une casse de G5.**

⚠️ **G5 N'Y TOUCHE PAS** — corriger la rouge d'un chantier actif la lui volerait.

## Ce qui est vert, et mesuré isolément

| Surface | Relevé |
| --- | --- |
| `cargo test -p proto` | **109** passed, 0 failed |
| `cargo test -p agent apps::` | **144** passed, 0 failed |
| `cargo test -p agent accent` | **19** passed, 0 failed |
| `client : npx vitest run src/hub/` | **74** passed |
| `client : npm run design:verifier` | **7/7 vert** |
| `proto : npm test` | **304** / 9 fichiers |
| `plateforme : test:sqlite` **et** `test:postgres` | **566** / 58 fichiers, les deux |
| les **trois** `typecheck` | sortie **0** |

⚠️ **`proto` est passé de 298 à 304** : **six** de ces tests sont **de F5**, pas
de moi — il a commité `a809341` pendant ma ronde. **Un compte de tests n'est
attribuable qu'assorti de son heure quand plusieurs chantiers partagent
l'arbre.**

## ⚠️ Un TROISIÈME rouge étranger, vu plus tôt et absent de celui-ci

`apps::surveillance::faute::une_famille_ne_consomme_pas_le_budget_d_une_autre`
(**G4**) a fait échouer une exécution antérieure de ce même filet, et **n'échoue
pas dans celle-ci**. Il est **INTERMITTENT** — mesuré **1 échec sur 12** en
parallèle, **0 sur 12** avec `--test-threads=1`. Voir
`14-flake-g4-surveillance-faute.log`.

🔴 **Son absence de ce journal-ci n'est donc PAS une réparation**, et il ne faut
pas la lire comme telle.
