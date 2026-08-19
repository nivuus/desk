-- L'identite des humains : la chaine de rafraichissement, rotative et
-- detectrice de rejeu.
--
-- La table `utilisateur` existe deja (0001-socle.sql) : P1 l'a creee vide,
-- P2 lui donne son comportement et non sa table. Rien n'est ajoute ici a son
-- schema.
--
-- 🔴 `famille` ET `remplace_par` NAISSENT AVEC LA TABLE, et ce n'est pas de la
-- prevoyance : sans elles, rien ne relie un jeton tourne a son successeur, et
-- la presentation d'un jeton deja tourne ne pourrait revoquer QUE la ligne
-- deja revoquee. Le voleur qui a tourne le premier garderait son jeton neuf,
-- et la detection de rejeu ne protegerait rien.
--
-- Elles ne peuvent pas etre ajoutees plus tard avec leurs contraintes :
-- SQLite ne sait pas ajouter une contrainte par ALTER TABLE -- mesure par P1
-- le 19 aout 2026 sur SQLite 3.50.4 : near "CONSTRAINT": syntax error. Une
-- cle etrangere naît avec sa table ou n'existe jamais. Meme raison pour
-- `REFERENCES utilisateur(id)` ci-dessous.
--
-- Les trois horodatages sont BIGINT et non INTEGER, et c'est MESURE par P1 :
-- INTEGER vaut 4 octets sur Postgres, ou un Date.now() rendait
--     value "1787136773742" is out of range for type integer
-- et le service ne pouvait pas appliquer ses PROPRES migrations. Ils portent
-- tous la convention de nommage `_a`, sans laquelle le lint statique de
-- `sous-ensemble.test.ts` ne pourrait pas les voir.
--
-- Aucune valeur litterale, pas meme un DEFAUT : `rendreMarqueurs` refuse tout
-- SQL portant une apostrophe ou un guillemet.

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

-- UNIQUE : l'empreinte est la CLE DE RECHERCHE d'un jeton presente. Deux
-- lignes de meme empreinte rendraient la rotation ambigue, et le choix de
-- celle a revoquer arbitraire.
CREATE UNIQUE INDEX jeton_empreinte ON jeton_rafraichissement(empreinte);

-- La famille est parcourue d'un coup a la detection d'un rejeu.
CREATE INDEX jeton_famille ON jeton_rafraichissement(famille);
