-- Le catalogue des applications : les colonnes que le sous-projet 4 ecrit sur
-- la table `application`, creee vide par 0003-agents.sql.
--
-- 🔴 POURQUOI TOUTES LES COLONNES NOT NULL NAISSENT ICI, ET PAS PLUS TARD.
-- Mesure le 19 aout 2026 sur SQLite 3.50.4 : un ALTER TABLE ADD COLUMN NOT
-- NULL sans DEFAUT est REFUSE des que la table porte une seule ligne --
--     Cannot add a NOT NULL column with default value NULL
-- et un DEFAUT litteral est impossible ici, `rendreMarqueurs` refusant tout
-- SQL portant une apostrophe ou un guillemet (base/pilote.ts). Une colonne
-- obligatoire qu un sous-bloc ulterieur voudrait ajouter serait donc
-- inajoutable des la premiere application decouverte : elles naissent toutes
-- maintenant, pendant que la table est encore vide.
--
-- 🔴 ET C EST AUSSI POURQUOI CE FICHIER NE FAIT PAS DROP PUIS CREATE. Cette
-- alternative a ete mesuree et fonctionne des deux cotes ; elle est ECARTEE
-- parce qu elle rendrait le geste dependant du fait que la table soit vide
-- CHEZ LE LECTEUR, et un DROP sur une table peuplee detruirait un catalogue
-- sans rien dire. ADD COLUMN, lui, CRIE quand la supposition est fausse.
-- Entre deux gestes qui supposent la meme chose, on prend celui qui crie.
--
-- 🔴 L UNICITE PASSE PAR UN INDEX, JAMAIS PAR ADD COLUMN ... UNIQUE. Mesure le
-- meme jour : SQLite rend `Cannot add a UNIQUE column` la ou Postgres
-- l accepte. Un seul des deux moteurs rougirait, et c est exactement ce que la
-- double passe existe pour attraper.
--
-- Les horodatages sont BIGINT et non INTEGER : INTEGER vaut 4 octets sur
-- Postgres, ou un Date.now() deborde (voir l en-tete de 0003-agents.sql). Ils
-- portent tous la convention `_a`, sans laquelle le lint statique de
-- sous-ensemble.test.ts ne pourrait pas les voir.
--
-- Aucune cle etrangere n est ajoutee : SQLite ne sait pas ajouter une
-- contrainte par ALTER TABLE, et une cle etrangere nait avec sa table ou
-- n existe jamais (leg n°2 de P1). Celle de `vm_id` est deja portee par
-- 0003-agents.sql.

-- L identite de l application, au sens du sous-projet 4 : l empreinte du
-- triplet (cible, arguments, repertoire). C est elle que l ordre de lancement
-- porte, jamais le chemin du raccourci.
ALTER TABLE application ADD COLUMN cle TEXT NOT NULL;

-- La cible du raccourci, NORMALISEE (absolue, casse repliee), et son
-- repertoire de travail, normalise de la meme facon.
ALTER TABLE application ADD COLUMN cible TEXT NOT NULL;

-- ⚠️ Les arguments sont BRUTS et SENSIBLES A LA CASSE, contrairement aux deux
-- chemins ci-dessus. Deux chemins Windows qui ne different que par la casse
-- designent le meme fichier ; deux lignes de commande qui ne different que par
-- la casse d un argument sont deux invocations distinctes. Vide = chaine vide,
-- jamais NULL.
ALTER TABLE application ADD COLUMN arguments TEXT NOT NULL;

ALTER TABLE application ADD COLUMN repertoire TEXT NOT NULL;

-- La premiere fois que la reconciliation a vu cette application.
--
-- ⚠️ ELLE EST ECRITE PAR G1 ET LUE PAR PERSONNE AVANT G3, et c est declare
-- plutot que decouvert. Elle nait maintenant parce qu une colonne NOT NULL ne
-- pourra plus etre ajoutee une fois la table peuplee, et le verdict
-- d installation en aura besoin. Precedent explicite du depot :
-- agents/fraicheur.ts, module pur sans appelant de production, acceptable
-- parce que declare tel.
ALTER TABLE application ADD COLUMN apparue_a BIGINT NOT NULL;

-- Quand l application a cesse d etre vue. POSEE, JAMAIS SUPPRIMEE : effacer la
-- ligne ferait perdre son identifiant a une application installee cote
-- navigateur, et une reapparition lui en donnerait un autre. Une resurrection
-- remet cette colonne a NULL sans toucher a l identifiant.
ALTER TABLE application ADD COLUMN disparue_a BIGINT NULL;

-- Le geste explicite qui masque une entree du catalogue -- un desinstalleur,
-- typiquement.
--
-- ⚠️ ELLE N EST ECRITE PAR PERSONNE EN G1 : aucun geste de masquage n existe
-- encore. Elle est NULLABLE, donc elle POURRAIT naitre plus tard ; elle nait
-- quand meme, pour ne pas fragmenter le schema d une meme table en deux
-- migrations. C est le seul point de ce fichier qui soit une commodite et non
-- une contrainte, et il est marque comme tel.
ALTER TABLE application ADD COLUMN masquee_a BIGINT NULL;

-- 🔴 L UNICITE PORTE SUR LE COUPLE (vm_id, cle), JAMAIS SUR LA CLE SEULE. La
-- cle est l empreinte d un triplet de chemins Windows : deux VMs qui portent
-- la meme application installee au meme endroit produisent la MEME cle, et un
-- index sur `cle` seule ferait que la seconde VM ne pourrait pas enregistrer
-- son catalogue.
CREATE UNIQUE INDEX application_cle ON application(vm_id, cle);
