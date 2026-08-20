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
--
-- ❌ LA DERNIÈRE PHRASE CI-DESSUS EST FAUSSE, ET LA PHRASE « une seconde
-- attribution est refusée » EST AMBIGUË AU POINT DE TROMPER (relevé le
-- 20 août 2026, revue transverse du sous-bloc P4, sur MESURE et non par
-- lecture). Ce que cet index interdit est qu'UN UTILISATEUR AIT DEUX VMs :
-- `utilisateur_id` est unique À TRAVERS LES LIGNES. Il n'interdit RIEN à
-- `UPDATE vm SET utilisateur_id = 'bob' WHERE id = 'v1'` quand v1 est déjà à
-- alice — une VM n'a qu'un `utilisateur_id`, et l'ÉCRASER ne viole aucune
-- unicité. Le vol d'une VM par un tiers passe donc, index en place.
--
-- 🔵 CE N'EST PAS UN RAISONNEMENT, C'EST UNE ROUGE JOUÉE. Journal versé :
-- `docs/superpowers/plans/journaux-plateforme-p4/rouge-2a-vol-sans-clause-conditionnelle.log`.
-- Cet index INTACT, il a suffi de retirer la clause `AND utilisateur_id IS
-- NULL` de `depot/vm.ts::attribuerSiLibre` pour que le vol réussisse —
-- `lignes touchées par l'UPDATE de vol : 1`, propriétaire de v1 changé — sur
-- SQLite 3.50.4 comme sur PostgreSQL 16.15. UNE exécution par moteur.
--
-- Ce que le critère 2 de P4 exige se scinde donc en DEUX propriétés, à DEUX
-- gardes, qui ne sont PAS toutes deux ici :
--   ②a « une VM n'est attribuée qu'une fois » -> la clause `AND
--       utilisateur_id IS NULL` de l'UPDATE, dans `depot/vm.ts`, PAS cet
--       index ;
--   ②b « un utilisateur ne reçoit qu'une VM »  -> cet index, qui LÈVE.
-- La même attribution fausse vit dans la spec §3.2 et son §4 « P4 » ; elle y
-- est annotée au même endroit et à la même date.
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
-- serait donc FAUX, pas seulement coûteux.
--
-- ✅ `vm_id` EST RENSEIGNÉE DEPUIS P3 (19 août 2026), et cette ligne disait
-- encore « reste entièrement vide : c'est P3 ». Le nom de session PORTE la VM
-- (`<préfixe>:bureau`) : `signaling/trace.ts` découpe le préfixe, le cherche
-- dans `agent_enrole` et inscrit l'identifiant trouvé.
--
-- ⚠️ ELLE RESTE NULLABLE, ET POUR UNE RAISON, PAS PAR DETTE : deux cas rendent
-- `null` honnêtement -- une session sans préfixe (mode d'essai local, spec
-- §10) et un préfixe inconnu de la base. `NOT NULL` refuserait alors d'écrire
-- une trace qui est par ailleurs juste.
--
-- ⚠️ La correction de cette ligne a été MANQUÉE une première fois : le commit
-- `254fdd5` de cette même branche a réécrit les six lignes qui précèdent sans
-- balayer les deux suivantes. C'est le naufrage du « 487 » -- corriger une
-- affirmation là où on nous l'a montrée, au lieu de la CHERCHER.

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
