-- Sous-ensemble portable (spec §3.2) : identifiants TEXT/UUID v4, horodatages
-- INTEGER en millisecondes TOUJOURS écrites par l'application, booléens
-- INTEGER 0/1, aucune valeur littérale.

CREATE TABLE schema_migration (
    version    INTEGER PRIMARY KEY,
    applique_a INTEGER NOT NULL
);

-- `utilisateur` est créée par P1 et RESTE VIDE : P2 lui donne son
-- comportement, pas sa table.
--
-- Pourquoi elle ne peut pas attendre : `vm.utilisateur_id` la référence, et
-- SQLite ne sait pas ajouter une contrainte par ALTER TABLE -- mesuré le
-- 19 août 2026 sur SQLite 3.50.4 : near "CONSTRAINT": syntax error. Une clé
-- étrangère naît avec sa table ou n'existe jamais.
CREATE TABLE utilisateur (
    id            TEXT PRIMARY KEY,
    email         TEXT NOT NULL UNIQUE,
    empreinte_mdp TEXT NOT NULL,
    cree_a        INTEGER NOT NULL
);

CREATE TABLE vm (
    id             TEXT PRIMARY KEY,
    nom            TEXT NOT NULL,
    adresse        TEXT NOT NULL,
    utilisateur_id TEXT NULL REFERENCES utilisateur(id),
    vue_a          INTEGER NULL
);

-- Index unique PARTIEL : plusieurs VM non attribuées coexistent, une seconde
-- attribution est refusée. Éprouvé le 19 août 2026 sur SQLite 3.50.4 --
-- deux NULL tolérés : OK, double attribution : REFUSEE -> UNIQUE constraint
-- failed: vm.utilisateur_id. C'est de lui que dépendra le critère 2 de P4.
CREATE UNIQUE INDEX vm_un_utilisateur ON vm(utilisateur_id)
    WHERE utilisateur_id IS NOT NULL;

-- `nom_session` est ABSENTE du schéma de la spec (§5), et ajoutée ici : sans
-- elle, la ligne écrite à l'appariement ne désigne rien -- c'est le seul
-- identifiant qu'une session possède avant P2 et P3.
--
-- `utilisateur_id` et `vm_id` naissent NULL alors que la spec les veut
-- NOT NULL : en P1 il n'y a ni utilisateur ni VM, et il n'existe aucune valeur
-- honnête. Ils ne seront PAS resserrés plus tard -- voir le commentaire de
-- `utilisateur` : SQLite exige une reconstruction de table, que Postgres ne
-- fait pas de la même façon.
CREATE TABLE session (
    id             TEXT PRIMARY KEY,
    nom_session    TEXT NOT NULL,
    utilisateur_id TEXT NULL,
    vm_id          TEXT NULL,
    ouverte_a      INTEGER NOT NULL,
    fermee_a       INTEGER NULL,
    motif          TEXT NULL
);

CREATE INDEX session_ouvertes ON session(fermee_a) WHERE fermee_a IS NULL;
