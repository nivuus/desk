-- Le televersement d un installeur, et l installation qui en decoule.
--
-- ⚠️ CE FICHIER NE PORTE NI APOSTROPHE NI GUILLEMET, y compris dans ses
-- commentaires : `rendreMarqueurs` (base/pilote.ts) REFUSE tout SQL qui en
-- porte, et la migration ne serait pas jouable. C est la convention de ses
-- cinq voisines, et elle explique la prose sans elisions qui suit.
--
-- 🔴 POURQUOI TOUTES LES COLONNES NAISSENT ICI. Un ALTER TABLE ADD COLUMN NOT
-- NULL sans DEFAUT est REFUSE par SQLite des que la table porte une ligne, et
-- un DEFAUT litteral est impossible (voir ci-dessus). Toute colonne
-- obligatoire qu un sous-bloc ULTERIEUR voudrait ajouter serait donc
-- inajoutable des le premier televersement : elles naissent toutes maintenant.
--
-- 🔴 LES CLES ETRANGERES NAISSENT AVEC LEURS TABLES, ET SANS ON DELETE. SQLite
-- ne sait pas ajouter une contrainte par ALTER TABLE : une cle etrangere nait
-- avec sa table ou n existe jamais (leg n°2 du sous-bloc P1). Et elles sont
-- APPLIQUEES des deux cotes -- pilote-sqlite.ts pose PRAGMA foreign_keys = ON.
--
-- ⚠️ L ABSENCE d ON DELETE est le choix de `application.vm_id`, reconduit : une
-- ligne orpheline n est inserable NULLE PART, et supprimer un televersement
-- qu une installation reference est REFUSE. C est voulu -- l historique d une
-- installation doit rester lisible, et les TRANCHES du disque, elles, sont
-- balayees par ailleurs.
--
-- Les horodatages sont BIGINT et non INTEGER : INTEGER vaut 4 octets sur
-- Postgres, ou un Date.now() deborde. Ils portent tous la convention `_a`,
-- sans laquelle le lint statique de sous-ensemble.test.ts ne peut pas les
-- voir.

-- Un fichier depose par un utilisateur, decoupe en tranches sur le disque du
-- service. Les tranches ne sont JAMAIS assemblees : le scellement est une
-- passe de flux qui recalcule le SHA-256, et la reprise est un LISTAGE de
-- repertoire -- jamais une comptabilite qui pourrait diverger du disque.
CREATE TABLE televersement (
    id              TEXT PRIMARY KEY,
    -- Le proprietaire. Toute route verifie cette colonne et rend le MEME refus
    -- indistinguable qu une ressource inconnue : distinguer les deux serait un
    -- oracle d enumeration, et le proprietaire du depot a tranche ce point
    -- pour le sous-bloc G1.
    utilisateur_id  TEXT NOT NULL REFERENCES utilisateur(id),
    -- Le nom tel que le NAVIGATEUR l annonce. Il est ASSAINI cote agent avant
    -- de devenir un chemin -- jamais ici, ou il n est qu une donnee.
    nom             TEXT NOT NULL,
    taille          BIGINT NOT NULL,
    -- L empreinte du fichier ENTIER, annoncee a la creation par le navigateur
    -- et RECALCULEE au scellement. Une seule valeur, comparable partout, y
    -- compris par un humain avec un sha256sum.
    sha256          TEXT NOT NULL,
    -- Fige a la creation : le decoupage ne doit pas changer sous les tranches
    -- deja deposees.
    taille_tranche  BIGINT NOT NULL,
    cree_a          BIGINT NOT NULL,
    -- NULL tant que le scellement n a pas eu lieu. Un televersement non scelle
    -- n est JAMAIS servi a un agent : il recevrait un fichier partiel dont
    -- l empreinte echouerait, et un refus au bon endroit vaut mieux qu un
    -- refus au bon moment.
    scelle_a        BIGINT NULL
);

CREATE INDEX televersement_par_utilisateur ON televersement(utilisateur_id);

-- Une demande d installation, sur une VM nommee, d un televersement scelle.
CREATE TABLE installation (
    id               TEXT PRIMARY KEY,
    vm_id            TEXT NOT NULL REFERENCES vm(id),
    televersement_id TEXT NOT NULL REFERENCES televersement(id),
    demandee_a       BIGINT NOT NULL,
    -- en_attente | en_cours | terminee. La plateforme cesse de REEMETTRE
    -- l ordre des qu il n est plus en_attente : c est la premiere des deux
    -- ceintures contre une double execution, la seconde etant le marqueur sur
    -- le disque de la VM.
    etat             TEXT NOT NULL,
    -- transfert | execution | reconciliation, ou la chaine vide tant que rien
    -- n a commence. ⚠️ `empreinte` n est PAS une phase de ce canal : elle se
    -- deroule dans le navigateur, avant que ce service n ait une ligne a
    -- ecrire.
    phase            TEXT NOT NULL,
    octets_faits     BIGINT NOT NULL,
    -- Vaut zero en phase `execution`, ou il n y a rien a totaliser : un
    -- installeur ne publie aucun pourcentage, et en inventer un serait mentir.
    octets_total     BIGINT NOT NULL,
    ecoule_ms        BIGINT NOT NULL,
    -- 🔴 NULL veut dire QUE LE CODE N A PAS PU ETRE RECUEILLI, et c est un fait
    -- different de tout code entier. Une sentinelle -1 les confondrait. Il est
    -- RAPPORTE, jamais interprete : msiexec rend 3010 pour un succes qui
    -- demande un redemarrage, et beaucoup d installeurs rendent 0 apres une
    -- annulation.
    code_sortie      INTEGER NULL,
    -- reussie | sans-effet | issue-inconnue | refusee. NULL tant que
    -- l installation n est pas terminee.
    issue            TEXT NULL,
    -- Le motif d un refus, et NULL autrement.
    motif            TEXT NULL,
    -- La QUEUE du journal de l installeur, bornee. ⚠️ Un journal vide est le
    -- cas NORMAL : la plupart des installeurs Windows sont graphiques et
    -- n ecrivent rien sur les flux standard.
    journal          TEXT NULL,
    journal_tronque  INTEGER NOT NULL,
    terminee_a       BIGINT NULL,
    maj_a            BIGINT NOT NULL
);

-- C est la requete de la reemission a l enrolement : les installations
-- en_attente d une VM donnee.
CREATE INDEX installation_par_vm_et_etat ON installation(vm_id, etat);
