-- The instant the BYTES of an upload were released, the row being kept.
--
-- ⚠️ THIS FILE CARRIES NEITHER APOSTROPHE NOR DOUBLE QUOTE, including in its
-- comments: `rendreMarqueurs` (base/pilote.ts) REFUSES any SQL that carries one.
--
-- 🔴 WHY A COLUMN AND NOT A DELETION. `installation.televersement_id`
-- references `televersement(id)` without ON DELETE, and nothing deletes an
-- installation row: its history must stay readable. The upload row could
-- therefore never be deleted once ordered, so its chunks -- up to 4 GiB per
-- installer -- stayed on the host root filesystem forever, finished or not.
-- The cleanup now releases the chunks of an upload whose installations are ALL
-- finished, and writes here when; the row keeps the name, size and fingerprint
-- the history refers to. NULL means the bytes are still held.
--
-- ⚠️ NULLABLE AND WITHOUT DEFAULT, on purpose: an ALTER TABLE ADD COLUMN NOT NULL
-- without a DEFAULT is refused by SQLite on a table that carries rows, and a
-- literal DEFAULT is impossible here. Every existing row holds its bytes, which
-- is exactly what NULL says. BIGINT for the epoch in milliseconds, as everywhere.

ALTER TABLE televersement ADD COLUMN contenu_purge_a BIGINT;
