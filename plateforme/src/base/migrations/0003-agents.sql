-- L'identite des agents : la table que le canal /agent lit et ecrit, et la
-- table `application` qui nait avec ses contraintes et reste vide.
--
-- MISE A JOUR (sous-bloc G1, 20 aout 2026) : elle NE RESTE PLUS VIDE. La
-- migration 0004 lui ajoute ses colonnes et son index d'unicite, et
-- `agents/canal.ts` l'ecrit a chaque `catalogue` recu. Voir l'encadre pose
-- sur la table elle-meme, plus bas.
--
-- Les horodatages sont BIGINT et non INTEGER, et c'est MESURE par P1 :
-- INTEGER vaut jusqu'a 8 octets sur SQLite et exactement 4 sur Postgres, ou
-- le 19 aout 2026, sur PostgreSQL 16.15, un Date.now() rendait
--     value "1787136773742" is out of range for type integer
-- et le service ne pouvait pas appliquer ses PROPRES migrations. Ils portent
-- tous la convention de nommage `_a`, sans laquelle le lint statique de
-- `sous-ensemble.test.ts` ne pourrait pas les voir -- sa portee est bornee
-- par cette convention, et par rien d'autre (leg n°8 de P1).
--
-- Aucune valeur litterale, pas meme un DEFAUT : `rendreMarqueurs` refuse tout
-- SQL portant une apostrophe ou un guillemet.

-- L'agent enrole d'une VM.
--
-- `vm_id` est la CLE PRIMAIRE et non un identifiant propre : une VM porte au
-- plus un agent, et deux lignes pour la meme VM n'auraient aucun sens --
-- laquelle serait la bonne ?
--
-- `empreinte_secret` porte le format `scrypt$N$r$p$sel$empreinte` de
-- `identite/mot-de-passe.ts`, le MEME que les comptes humains. Une seconde
-- derivation dans le meme service divergerait de la premiere le jour ou l'une
-- des deux serait durcie.
--
-- 🔴 `prefixe_session` est UNIQUE, et cet index n'est pas decoratif : c'est
-- lui qui rend `lireParPrefixe` DECIDABLE. Deux VMs de meme prefixe rendraient
-- la resolution d'un nom de session ambigue, et le choix de la VM arbitraire
-- -- c'est-a-dire exactement le probleme que le prefixe existe pour fermer.
--
-- `vu_a` nait NULL : une VM enrolee qui n'a jamais battu n'a rien d'honnete a
-- y inscrire, et `0` se lirait comme une epoque de 1970 donc comme un agent
-- injoignable depuis cinquante-six ans. `agents/fraicheur.ts` distingue les
-- deux cas.
CREATE TABLE agent_enrole (
    vm_id            TEXT PRIMARY KEY REFERENCES vm(id),
    empreinte_secret TEXT NOT NULL,
    prefixe_session  TEXT NOT NULL UNIQUE,
    vu_a             BIGINT NULL
);

-- ❌ « RESTE VIDE » EST FAUX DEPUIS LE SOUS-BLOC G1, qui lui a donne son
-- chemin d ecriture (`agents/canal.ts`, par `apps/catalogue.ts::fusionner`), et
-- G2 lui a ajoute ses colonnes d icone. La phrase ci-dessous reste le releve
-- EXACT de P3 a sa date, et c est pour cela qu elle n est pas effacee.
-- ⚠️ Releve par le sous-bloc G3, qui creait ses propres tables a cote.
-- `application` est creee par P3 et RESTE VIDE : son chemin d'ecriture est le
-- sous-projet ④, qui empruntera le canal /agent pour la remplir.
--
-- ❌ CES DEUX PHRASES SONT DEVENUES FAUSSES LE 20 AOUT 2026, sous-bloc G1 --
-- c'est-a-dire par le sous-projet ④ lui-meme, qui les a prises au mot. Elles
-- sont conservees comme relevé daté de P3 plutot que reecrites, et corrigees
-- ici : `application` a desormais un ecrivain, `agents/canal.ts`, qui applique
-- la fusion de `apps/catalogue.ts` a chaque message `catalogue` de l'agent, et
-- des colonnes que lui ajoute la migration 0004 (cle, cible, arguments,
-- repertoire, apparue_a, disparue_a, masquee_a). Mesure en recette :
-- 154 lignes pour la VM de developpement.
--
-- Elle nait ici, et pas plus tard, POUR SA CLE ETRANGERE : SQLite ne sait pas
-- ajouter une contrainte par ALTER TABLE -- mesure par P1 le 19 aout 2026 sur
-- SQLite 3.50.4 : near "CONSTRAINT": syntax error. Une cle etrangere nait
-- avec sa table ou n'existe jamais (leg n°2 de P1). Ce n'est donc PAS un
-- oubli si aucun code ne l'ecrit : c'est le prix, connu et paye d'avance, de
-- la contrainte qu'elle porte.
--
-- ⚠️ LA CLAUSE FINALE NE DECRIT PLUS LE DEPOT depuis G1 : du code l'ecrit. Le
-- RAISONNEMENT, lui, reste entier et c'est pourquoi il n'est pas supprime --
-- c'est parce que la cle etrangere ne pouvait naitre qu'ici que la table a du
-- naitre vide en P3, et la migration 0004 n'a eu qu'a lui ajouter des
-- colonnes. La mesure de G1 confirme le prix paye d'avance : `ADD COLUMN ...
-- NOT NULL` sans DEFAUT ne passe QUE sur une table vide (divergence E8).
CREATE TABLE application (
    id     TEXT PRIMARY KEY,
    vm_id  TEXT NOT NULL REFERENCES vm(id),
    nom    TEXT NOT NULL,
    chemin TEXT NOT NULL,
    vue_a  BIGINT NOT NULL
);
