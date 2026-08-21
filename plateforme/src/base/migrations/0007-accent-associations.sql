-- La couleur d accent d une application, et les extensions qu elle ouvre.
-- Sous-bloc G5, tranche F.
--
-- Les deux servent le manifeste PWA par application : `theme_color` d un cote,
-- `file_handlers` de l autre.
--
-- 🔴 POURQUOI `accent` EST UNE COLONNE ET `associations` UNE TABLE. Ce n est
-- pas une preference de style, c est la mesure de 0005 appliquee deux fois.
-- Sur SQLite 3.50.4, un ALTER TABLE ADD COLUMN NOT NULL sans DEFAUT est REFUSE
-- des que la table porte une seule ligne, et `application` n est plus vide
-- depuis la recette de G1. Donc :
--
--   * `accent` est NULLABLE, ET C EST DE TOUTE FACON JUSTE : une icone trop
--     pale, trop sombre ou trop transparente n a AUCUNE dominante, et
--     `accent::dominante` rend `None` par construction. NULL veut dire « pas
--     d accent », jamais « pas encore mesure » -- le manifeste OMET alors
--     `theme_color` plutot que d en inventer un ;
--
--   * les associations, elles, seraient PLUSIEURS par application. Les
--     entasser dans une colonne TEXT en JSON ferait porter a SQL une structure
--     qu il ne sait ni indexer ni contraindre, et la premiere requete qui
--     voudrait « quelles applications ouvrent .pdf » devrait la relire en
--     entier. Une TABLE NEUVE, elle, PEUT naitre avec ses NOT NULL et sa cle
--     etrangere -- c est la lecon de la divergence D3 du sous-bloc P1 :
--     SQLite ne sait pas AJOUTER une contrainte par ALTER TABLE, donc une
--     contrainte nait avec sa table ou n existe jamais.
--
-- ⚠️ LE COUPLE (application_id, extension) EST UNIQUE, ET LA CLE PRIMAIRE LE
-- DIT. Une meme application ne peut pas ouvrir deux fois `.txt`, et l agent
-- dedoublonne deja (`apps::associations::ranger`) -- mais une garantie qui ne
-- vit que dans l emetteur n est pas une garantie.
--
-- ⚠️ `ON DELETE CASCADE` : une application retiree emporte ses associations.
-- Sans lui, une ligne orpheline survivrait a l application qu elle decrit, et
-- le jour ou un identifiant serait reemploye elle lui serait attribuee.

ALTER TABLE application ADD COLUMN accent TEXT;

CREATE TABLE application_association (
    application_id TEXT NOT NULL REFERENCES application(id) ON DELETE CASCADE,
    extension TEXT NOT NULL,
    PRIMARY KEY (application_id, extension)
);

-- L index qui sert la question inverse -- « qui ouvre cette extension ? ».
-- La cle primaire indexe deja (application_id, extension) ; celui-ci indexe
-- l autre sens, que la cle primaire ne couvre pas.
CREATE INDEX application_association_extension ON application_association (extension);
