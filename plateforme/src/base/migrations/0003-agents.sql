-- The identity of agents: the table the /agent channel reads and writes, and the
-- `application` table that is born with its constraints and stays empty.
--
-- UPDATE (sub-block G1, 20 August 2026): it NO LONGER STAYS EMPTY. The
-- migration 0004 adds its columns and its uniqueness index, and
-- `agents/canal.ts` writes it at each `catalogue` received. See the box placed
-- on the table itself, further down.
--
-- The timestamps are BIGINT and not INTEGER, and it is MEASURED by P1:
-- INTEGER is up to 8 bytes on SQLite and exactly 4 on Postgres, where
-- on 19 August 2026, on PostgreSQL 16.15, a Date.now() returned
--     value "1787136773742" is out of range for type integer
-- and the service could not apply its OWN migrations. They all carry
-- the `_a` naming convention, without which the static lint of
-- `sous-ensemble.test.ts` could not see them -- its reach is bounded
-- by this convention, and by nothing else (P1's legacy item no. 8).
--
-- No literal value, not even a DEFAULT: `rendreMarqueurs` refuses any
-- SQL carrying an apostrophe or a double quote.

-- The enrolled agent of a VM.
--
-- `vm_id` is the PRIMARY KEY and not an identifier of its own: a VM carries at
-- most one agent, and two rows for the same VM would make no sense --
-- which one would be the right one?
--
-- `empreinte_secret` carries the `scrypt$N$r$p$sel$empreinte` format of
-- `identite/mot-de-passe.ts`, the SAME as the human accounts. A second
-- derivation in the same service would diverge from the first the day one
-- of the two was hardened.
--
-- 🔴 `prefixe_session` is UNIQUE, and this index is not decorative: it is
-- what makes `lireParPrefixe` DECIDABLE. Two VMs with the same prefix would make
-- the resolution of a session name ambiguous, and the choice of VM arbitrary
-- -- that is, exactly the problem the prefix exists to close.
--
-- `vu_a` is born NULL: an enrolled VM that has never beaten has nothing honest to
-- record there, and `0` would read as an epoch of 1970 hence as an agent
-- unreachable for fifty-six years. `agents/fraicheur.ts` distinguishes the
-- two cases.
CREATE TABLE agent_enrole (
    vm_id            TEXT PRIMARY KEY REFERENCES vm(id),
    empreinte_secret TEXT NOT NULL,
    prefixe_session  TEXT NOT NULL UNIQUE,
    vu_a             BIGINT NULL
);

-- ❌ "STAYS EMPTY" IS FALSE SINCE SUB-BLOCK G1, which gave it its
-- write path (`agents/canal.ts`, through `apps/catalogue.ts::fusionner`), and
-- G2 added its icon columns. The sentence below remains the EXACT
-- record of P3 at its date, and that is why it is not erased.
-- ⚠️ Found by sub-block G3, which created its own tables next to it.
-- `application` is created by P3 and STAYS EMPTY: its write path is
-- sub-project ④, which will borrow the /agent channel to fill it.
--
-- ❌ THESE TWO SENTENCES BECAME FALSE ON 20 AUGUST 2026, sub-block G1 --
-- that is, through sub-project ④ itself, which took them at their word. They
-- are kept as a dated record of P3 rather than rewritten, and corrected
-- here: `application` now has a writer, `agents/canal.ts`, which applies
-- the merge of `apps/catalogue.ts` at each `catalogue` message from the agent, and
-- columns that migration 0004 adds to it (cle, cible, arguments,
-- repertoire, apparue_a, disparue_a, masquee_a). Measured at acceptance:
-- 154 rows for the development VM.
--
-- It is born here, and not later, FOR ITS FOREIGN KEY: SQLite cannot
-- add a constraint through ALTER TABLE -- measured by P1 on 19 August 2026 on
-- SQLite 3.50.4: near "CONSTRAINT": syntax error. A foreign key is born
-- with its table or never exists (P1's legacy item no. 2). It is therefore NOT an
-- oversight if no code writes it: it is the price, known and paid in advance, of
-- the constraint it carries.
--
-- ⚠️ THE FINAL CLAUSE NO LONGER DESCRIBES THE REPOSITORY since G1: code writes it. The
-- REASONING, for its part, stays whole and that is why it is not removed --
-- it is because the foreign key could only be born here that the table had to
-- be born empty in P3, and migration 0004 only had to add
-- columns to it. G1's measurement confirms the price paid in advance: `ADD COLUMN ...
-- NOT NULL` without a DEFAULT passes ONLY on an empty table (divergence E8).
CREATE TABLE application (
    id     TEXT PRIMARY KEY,
    vm_id  TEXT NOT NULL REFERENCES vm(id),
    nom    TEXT NOT NULL,
    chemin TEXT NOT NULL,
    vue_a  BIGINT NOT NULL
);
