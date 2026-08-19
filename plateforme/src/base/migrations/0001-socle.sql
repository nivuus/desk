-- Sous-ensemble portable (spec §3.2) : identifiants TEXT/UUID v4, horodatages
-- BIGINT en millisecondes TOUJOURS écrites par l'application, booléens
-- INTEGER 0/1, aucune valeur littérale.
--
-- Les horodatages sont BIGINT et non INTEGER, et c'est MESURÉ, pas prudentiel.
-- `INTEGER` vaut jusqu'à 8 octets sur SQLite et exactement 4 sur Postgres :
-- le 19 août 2026, sur PostgreSQL 16.15, écrire un `Date.now()` dans une
-- colonne INTEGER rendait
--     value "1787136773742" is out of range for type integer
-- et le service ne pouvait pas appliquer ses PROPRES migrations. SQLite
-- l'acceptait sans un mot -- c'est un quatrième angle mort du couple
-- lint / double passe, celui du CHOIX DES VALEURS.
--
-- Convention qui rend la règle contrôlable : toute colonne d'horodatage porte
-- un nom en `_a` (cree_a, vue_a, ouverte_a, fermee_a, applique_a), et le lint
-- de `sous-ensemble.test.ts` refuse un `_a INTEGER`.

CREATE TABLE schema_migration (
    version    INTEGER PRIMARY KEY,
    applique_a BIGINT NOT NULL
);

-- `utilisateur` est créée par P1 et RESTE VIDE : P2 lui donne son
-- comportement, pas sa table.
--
-- ✅ P2 L'A FAIT (19 août 2026) : la table n'est plus vide. `depot/utilisateur.ts`
-- l'écrit et la relit, `admin/creer-utilisateur.ts` y crée un compte par la
-- ligne de commande, et `identite/mot-de-passe.ts` remplit `empreinte_mdp` au
-- format `scrypt$N$r$p$sel$empreinte`. **La table elle-même n'a PAS bougé** —
-- c'est exactement ce que la phrase ci-dessus promettait, et c'est ce qui
-- rendait la contrainte de D3 payante. `0002-identite.sql` s'y adosse.
--

-- Pourquoi elle ne peut pas attendre : `vm.utilisateur_id` la référence, et
-- SQLite ne sait pas ajouter une contrainte par ALTER TABLE -- mesuré le
-- 19 août 2026 sur SQLite 3.50.4 : near "CONSTRAINT": syntax error. Une clé
-- étrangère naît avec sa table ou n'existe jamais.
CREATE TABLE utilisateur (
    id            TEXT PRIMARY KEY,
    email         TEXT NOT NULL UNIQUE,
    empreinte_mdp TEXT NOT NULL,
    cree_a        BIGINT NOT NULL
);

CREATE TABLE vm (
    id             TEXT PRIMARY KEY,
    nom            TEXT NOT NULL,
    adresse        TEXT NOT NULL,
    utilisateur_id TEXT NULL REFERENCES utilisateur(id),
    vue_a          BIGINT NULL
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
--
-- ✅ P2 RENSEIGNE `utilisateur_id` (19 août 2026), et la colonne reste NULLABLE
-- POUR UNE RAISON QUI N'EST PAS DE LA DETTE : une session appariée par un pair
-- `agent` seul -- la session de contrôle `bureau` au démarrage d'une VM -- n'a
-- personne à inscrire : l'agent ne REVENDIQUE rien, sa session devant rester
-- revendicable par le client humain qui la rejoindra (P3 lui a donné une
-- identité, pas une propriété). `NOT NULL`
-- serait donc FAUX, pas seulement coûteux. `vm_id`, lui, reste entièrement
-- vide : c'est P3.

CREATE TABLE session (
    id             TEXT PRIMARY KEY,
    nom_session    TEXT NOT NULL,
    utilisateur_id TEXT NULL,
    vm_id          TEXT NULL,
    ouverte_a      BIGINT NOT NULL,
    fermee_a       BIGINT NULL,
    motif          TEXT NULL
);

CREATE INDEX session_ouvertes ON session(fermee_a) WHERE fermee_a IS NULL;
