# Contribuer aux projets Nivuus

## Workflow

La branche principale s'appelle `main` et n'accepte aucun push direct.
Toute modification passe par une pull request dont les checks
`policy / Coding rules` et `security / Secrets and dependencies` doivent être
verts. Le merge se fait en squash, et la branche est supprimée
automatiquement.

## Règles de codage

**Anglais dans le code.** Les identifiants, les commentaires, les docstrings,
les messages de commit et les noms de branches sont en anglais. Les chaînes
affichées à l'utilisateur, les README, les CHANGELOG et le contenu de `docs/`
restent en français.

Si une ligne déclenche à tort le contrôle — un nom propre, un terme métier
français sans équivalent, un identifiant de protocole — ajoutez
`policy: allow-fr` dans un commentaire. Un marqueur placé dans une chaîne de
caractères n'a aucun effet : il doit apparaître en dehors de toute donnée
pour rester une exemption visible et relisable.

Pour un fichier entier — typiquement un fichier qui ne contient que des
messages destinés à l'utilisateur, ou un texte multiligne que le marqueur de
ligne ne peut pas couvrir sans s'afficher — placez `policy: allow-fr-file`
n'importe où dans le fichier.

**500 lignes maximum par fichier source.** La règle vise le code de
production ; les fichiers de test et les fichiers générés en sont exemptés.
Un fichier qui dépasse la limite signale généralement une responsabilité mal
découpée.

Pour un fichier ancien que l'on ne fait que retoucher — corriger un
commentaire, ajouter une directive — sans vouloir le découper séance tenante,
placez `policy: allow-long-file` dans le fichier, suivi de la raison. La dette
reste visible et relisable en revue, au lieu d'être contournée en silence.

**Conventional Commits.** Format `<type>(<portée>)!: <sujet en anglais>`.
Types acceptés : `feat`, `fix`, `chore`, `docs`, `refactor`, `test`, `ci`,
`perf`, `build`, `style`, `revert`. Le format conditionne la génération du
CHANGELOG et le calcul de version, il est donc bloquant.

## Portée des contrôles

Les contrôles ne s'appliquent qu'aux fichiers ajoutés ou modifiés dans la
pull request. Le code existant qui ne respecte pas encore ces règles n'est
pas signalé tant qu'on n'y touche pas.

Cette portée n'est pas uniforme : la règle d'anglais regarde les lignes que
vous ajoutez ou modifiez, ligne par ligne, et laisse tranquille le reste d'un
fichier ancien qu'un commit ne fait qu'effleurer. La règle des 500 lignes
regarde le fichier entier, parce qu'une longueur ne se mesure pas ligne par
ligne : toucher une seule ligne d'un vieux fichier trop long l'expose donc à
la limite, avec `policy: allow-long-file` comme échappatoire si ce n'est pas
le moment de le découper.
