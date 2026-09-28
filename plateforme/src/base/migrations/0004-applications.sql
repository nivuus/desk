-- The application catalogue: the columns sub-project 4 writes on
-- the `application` table, created empty by 0003-agents.sql.
--
-- 🔴 WHY ALL THE NOT NULL COLUMNS ARE BORN HERE, AND NOT LATER.
-- Measured on 19 August 2026 on SQLite 3.50.4: an ALTER TABLE ADD COLUMN NOT
-- NULL without a DEFAULT is REFUSED as soon as the table carries a single row --
--     Cannot add a NOT NULL column with default value NULL
-- and a literal DEFAULT is impossible here, `rendreMarqueurs` refusing any
-- SQL carrying an apostrophe or a double quote (base/pilote.ts). A mandatory
-- column a later sub-block wanted to add would therefore be
-- impossible to add from the first discovered application: they are all born
-- now, while the table is still empty.
--
-- 🔴 AND IT IS ALSO WHY THIS FILE DOES NOT DROP THEN CREATE. This
-- alternative was measured and works on both sides; it is RULED OUT
-- because it would make the gesture depend on the table being empty
-- ON THE READER'S SIDE, and a DROP on a populated table would destroy a catalogue
-- without saying anything. ADD COLUMN, for its part, SHOUTS when the assumption is wrong.
-- Between two gestures that assume the same thing, we take the one that shouts.
--
-- 🔴 UNIQUENESS GOES THROUGH AN INDEX, NEVER THROUGH ADD COLUMN ... UNIQUE. Measured the
-- same day: SQLite returns `Cannot add a UNIQUE column` where Postgres
-- accepts it. Only one of the two engines would turn red, and it is exactly what the
-- double pass exists to catch.
--
-- The timestamps are BIGINT and not INTEGER: INTEGER is 4 bytes on
-- Postgres, where a Date.now() overflows (see the header of 0003-agents.sql). They
-- all carry the `_a` convention, without which the static lint of
-- sous-ensemble.test.ts could not see them.
--
-- No foreign key is added: SQLite cannot add a
-- constraint through ALTER TABLE, and a foreign key is born with its table or
-- never exists (P1's legacy item no. 2). That of `vm_id` is already carried by
-- 0003-agents.sql.

-- The identity of the application, in the sense of sub-project 4: the hash of the
-- triple (cible, arguments, repertoire). It is what the launch order
-- carries, never the path of the shortcut.
ALTER TABLE application ADD COLUMN cle TEXT NOT NULL;

-- The target of the shortcut, NORMALISED (absolute, case folded), and its
-- working directory, normalised the same way.
ALTER TABLE application ADD COLUMN cible TEXT NOT NULL;

-- ⚠️ The arguments are RAW and CASE SENSITIVE, unlike the two
-- paths above. Two Windows paths that differ only by case
-- designate the same file; two command lines that differ only by
-- the case of an argument are two distinct invocations. Empty = empty string,
-- never NULL.
ALTER TABLE application ADD COLUMN arguments TEXT NOT NULL;

ALTER TABLE application ADD COLUMN repertoire TEXT NOT NULL;

-- The first time the reconciliation saw this application.
--
-- ⚠️ IT IS WRITTEN BY G1 AND READ BY NOBODY BEFORE G3, and it is declared
-- rather than discovered. It is born now because a NOT NULL column can
-- no longer be added once the table is populated, and the installation
-- verdict will need it. Explicit precedent of the repository:
-- agents/fraicheur.ts, a pure module without a production caller, acceptable
-- because declared as such.
ALTER TABLE application ADD COLUMN apparue_a BIGINT NOT NULL;

-- When the application stopped being seen. SET, NEVER REMOVED: erasing the
-- row would make an application installed on the browser side lose its identifier,
-- and a reappearance would give it another one. A resurrection
-- sets this column back to NULL without touching the identifier.
ALTER TABLE application ADD COLUMN disparue_a BIGINT NULL;

-- The explicit gesture that hides a catalogue entry -- an uninstaller,
-- typically.
--
-- ⚠️ IT IS WRITTEN BY NOBODY IN G1: no hiding gesture exists
-- yet. It is NULLABLE, so it COULD be born later; it is born
-- anyway, so as not to fragment the schema of one table across two
-- migrations. It is the only point of this file that is a convenience and not
-- a constraint, and it is marked as such.
ALTER TABLE application ADD COLUMN masquee_a BIGINT NULL;

-- 🔴 UNIQUENESS BEARS ON THE PAIR (vm_id, cle), NEVER ON THE KEY ALONE. The
-- key is the hash of a triple of Windows paths: two VMs that carry
-- the same application installed at the same place produce the SAME key, and an
-- index on `cle` alone would mean the second VM could not record
-- its catalogue.
CREATE UNIQUE INDEX application_cle ON application(vm_id, cle);
