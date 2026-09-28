-- The identity of humans: the refresh chain, rotating and
-- replay-detecting.
--
-- The `utilisateur` table already exists (0001-socle.sql): P1 created it empty,
-- P2 gives it its behaviour and not its table. Nothing is added here to its
-- schema.
--
-- 🔴 `famille` AND `remplace_par` ARE BORN WITH THE TABLE, and it is not
-- foresight: without them, nothing links a rotated token to its successor, and
-- presenting an already rotated token could revoke ONLY the row
-- already revoked. The thief who rotated first would keep their new token,
-- and replay detection would protect nothing.
--
-- They cannot be added later with their constraints:
-- SQLite cannot add a constraint through ALTER TABLE -- measured by P1
-- on 19 August 2026 on SQLite 3.50.4: near "CONSTRAINT": syntax error. A
-- foreign key is born with its table or never exists. Same reason for
-- `REFERENCES utilisateur(id)` below.
--
-- The three timestamps are BIGINT and not INTEGER, and it is MEASURED by P1:
-- INTEGER is 4 bytes on Postgres, where a Date.now() returned
--     value "1787136773742" is out of range for type integer
-- and the service could not apply its OWN migrations. They all carry
-- the `_a` naming convention, without which the static lint of
-- `sous-ensemble.test.ts` could not see them.
--
-- No literal value, not even a DEFAULT: `rendreMarqueurs` refuses any
-- SQL carrying an apostrophe or a double quote.

CREATE TABLE jeton_rafraichissement (
    id             TEXT PRIMARY KEY,
    utilisateur_id TEXT NOT NULL REFERENCES utilisateur(id),
    famille        TEXT NOT NULL,
    empreinte      TEXT NOT NULL,
    remplace_par   TEXT NULL,
    cree_a         BIGINT NOT NULL,
    expire_a       BIGINT NOT NULL,
    revoque_a      BIGINT NULL
);

-- UNIQUE: the hash is the LOOKUP KEY of a presented token. Two
-- rows with the same hash would make the rotation ambiguous, and the choice of
-- the one to revoke arbitrary.
CREATE UNIQUE INDEX jeton_empreinte ON jeton_rafraichissement(empreinte);

-- The family is walked in one go when a replay is detected.
CREATE INDEX jeton_famille ON jeton_rafraichissement(famille);
