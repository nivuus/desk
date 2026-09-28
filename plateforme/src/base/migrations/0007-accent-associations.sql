-- The accent colour of an application, and the extensions it opens.
-- Sub-block G5, slice F.
--
-- Both serve the per-application PWA manifest: `theme_color` on one side,
-- `file_handlers` on the other.
--
-- 🔴 WHY `accent` IS A COLUMN AND `associations` A TABLE. It is
-- not a style preference, it is the measurement of 0005 applied twice.
-- On SQLite 3.50.4, an ALTER TABLE ADD COLUMN NOT NULL without a DEFAULT is REFUSED
-- as soon as the table carries a single row, and `application` is no longer empty
-- since the acceptance of G1. Therefore:
--
--   * `accent` is NULLABLE, AND IT IS RIGHT ANYWAY: an icon too
--     pale, too dark or too transparent has NO dominant colour, and
--     `accent::dominante` returns `None` by construction. NULL means « no
--     accent », never « not measured yet » -- the manifest then OMITS
--     `theme_color` rather than inventing one;
--
--   * the associations, for their part, would be SEVERAL per application.
--     Piling them into a TEXT column as JSON would make SQL carry a structure
--     it can neither index nor constrain, and the first query that
--     wanted « which applications open .pdf » would have to reread it
--     whole. A NEW TABLE, for its part, CAN be born with its NOT NULL and its foreign
--     key -- it is the lesson of divergence D3 of sub-block P1:
--     SQLite cannot ADD a constraint through ALTER TABLE, so a
--     constraint is born with its table or never exists.
--
-- ⚠️ THE PAIR (application_id, extension) IS UNIQUE, AND THE PRIMARY KEY
-- SAYS SO. The same application cannot open `.txt` twice, and the agent
-- already deduplicates (`apps::associations::ranger`) -- but a guarantee that
-- only lives in the emitter is not a guarantee.
--
-- ⚠️ `ON DELETE CASCADE`: a removed application takes its associations with it.
-- Without it, an orphan row would survive the application it describes, and
-- the day an identifier was reused it would be attributed to it.

ALTER TABLE application ADD COLUMN accent TEXT;

CREATE TABLE application_association (
    application_id TEXT NOT NULL REFERENCES application(id) ON DELETE CASCADE,
    extension TEXT NOT NULL,
    PRIMARY KEY (application_id, extension)
);

-- The index that serves the reverse question -- « who opens this extension? ».
-- The primary key already indexes (application_id, extension); this one indexes
-- the other direction, which the primary key does not cover.
CREATE INDEX application_association_extension ON application_association (extension);
