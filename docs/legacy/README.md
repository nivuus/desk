# Archive du Guacamole historique

**Date de l'archivage** : 21 août 2026.

⛔ **CE RÉPERTOIRE NE SUPPRIME RIEN, ET RIEN N'A ÉTÉ SUPPRIMÉ AILLEURS.** Il
**ajoute** des fichiers. Les 19 fichiers du legacy sont tous à leur place, le
produit n'est pas touché, aucun service n'a été démarré ni arrêté.

🔴 **RIEN NE PEUT PARTIR AUJOURD'HUI, et ce n'est pas une prudence de rédaction :
c'est un relevé.** Sur les dix verrous de la spécification de retrait, **deux
sont satisfaits**. L'état verrou par verrou, avec ses commandes, vit dans
`../superpowers/plans/2026-08-21-retrait-legacy-etat-des-verrous.md`.

⚠️ **CET ÉTAT N'EST PAS RECOPIÉ ICI, ET C'EST DÉLIBÉRÉ.** Un second tableau de
verrous divergerait du premier au premier chantier qui bouge, et ce dépôt a payé
neuf fois le naufrage du « 487 ». **Lire le relevé, jamais ce fichier-ci, pour
savoir où en est le retrait.**

---

## 1. Pourquoi cette archive existe : 890 lignes ne sont dans AUCUN commit

Le motif `index.js` de `.gitignore` (l. 18) **n'a pas d'ancre de début de
chemin** : il attrape `web/index.js` à n'importe quelle profondeur. Sa raison est
écrite juste au-dessus et elle est **bonne** — `index.js` porte des mots de passe
Windows en clair. C'est sa **portée** qui est trop large.

**Remesuré le 21 août 2026** (`wc -l`, `git check-ignore -v`, `git ls-files`) :

| Fichier vivant | Lignes | Suivi par git ? |
| --- | --- | --- |
| `web/index.js` | **804** | 🔴 **NON** — `.gitignore:18:index.js` |
| `index.js` | **61** | 🔴 **NON** — `.gitignore:18:index.js` |
| `docker-compose.yml` | **25** | 🔴 **NON** — `.gitignore:19:docker-compose.yml` |
| **Total hors git** | **890** | |

⚠️ **Témoin positif du zéro** : `git ls-files -- web/home.js` rend bien
`web/home.js`. La sortie vide sur les trois fichiers ci-dessus est donc un **vrai
zéro**, pas une commande cassée.

**« L'historique git reste » est donc FAUX pour 890 lignes sur les 3 170 du
legacy.** Une suppression de `web/index.js` détruirait définitivement 804 lignes,
et c'est le seul report **irréversible** de tout le chantier de retrait.

---

## 2. Ce qui est archivé

| Fichier d'archive | Origine | Lignes | Forme |
| --- | --- | --- | --- |
| `web-index.js` | `web/index.js` | **804** | **VERBATIM**, octet pour octet |
| `racine-index.js` | `index.js` | **61** | **CAVIARDÉ** aux lignes 2, 3 et 4 |

### 2.1 🔴 Les numéros de ligne sont PRÉSERVÉS, et c'est la propriété qui compte

**77 occurrences réparties dans 11 documents** de `docs/superpowers/` citent
`web/index.js`, **dont 29 avec un numéro de ligne distinct** — vers un fichier qui
n'existe dans aucun commit. *(Comptes remesurés le 21 août 2026, ce README et le
relevé exclus du balayage ; témoin négatif sur une chaîne inexistante : 0.)*

**Une archive qui décalerait les numéros de ligne rendrait ces 29 citations
fausses au lieu de les sauver.** C'est pourquoi :

- `web-index.js` ne porte **aucun en-tête ajouté** : `diff web/index.js
  docs/legacy/web-index.js` est **vide** ;
- `racine-index.js` est caviardé **en place**, une ligne pour une ligne : le
  `diff` porte sur les seules lignes **2, 3 et 4**, et le compte reste **61**.

**Toute la prose de cette archive vit donc ici, dans ce README, et nulle part
dans les fichiers archivés.**

### 2.2 Le caviardage de `racine-index.js`

Les lignes 2 à 4 du fichier vivant affectent trois identifiants Windows en clair.
Dans l'archive, la **structure est conservée et la valeur remplacée** par un objet
de remplacement explicite : les trois lignes portent toujours
`process.env.<NOM> = …`, si bien qu'un lecteur voit **quel nom était renseigné à
quelle ligne**, sans jamais lire ce qu'il valait.

⚠️ **Cette archive ne purge AUCUN secret de la machine.** `index.js`,
`docker-compose.yml` et `.env` vivants les portent toujours. Caviarder une copie
n'est pas une rotation.

### 2.3 🔴 Le test des secrets NE PROTÈGE PAS ce répertoire contre la forme du legacy

`plateforme/src/securite/secrets.test.ts` balaie `git ls-files`, `docs/` compris,
et dénonce toute affectation littérale de sept noms connus — `WINDOWS_PASSWORD`
et `WINDOWS_ADMIN_PASSWORD` en font partie. Il est **vert** sur cette archive.

⚠️ **MAIS CE VERT EST VACUEUX POUR LA FORME QUE LE LEGACY EMPLOIE, et c'est
mesuré, pas soupçonné.** Son motif exige que le nom soit précédé d'un début de
ligne ou de l'un de `[espace " ' \` ( , ; {]`. Or `index.js` écrit
`process.env.WINDOWS_PASSWORD = …` : le caractère qui précède le nom est un
**point**, que le motif n'accepte pas.

**Rouge jouée sur ce répertoire même, une seule variable changée entre les deux
bras** — même fichier, même ligne, même valeur factice :

| Ligne 2 de `racine-index.js` | Verdict du test |
| --- | --- |
| `process.env.WINDOWS_PASSWORD = "<valeur factice>";` | 🔴 **VERT — le secret passe** |
| `WINDOWS_PASSWORD = "<valeur factice>";` | ✅ **ROUGE — le test dénonce** |

*(Arbre restauré après chaque bras, empreinte `sha256` identique avant et
après.)*

🔴 **Conséquence pour qui ajoutera un fichier ici** : archiver `index.js`
**verbatim** aurait committé trois identifiants en clair **sans qu'aucun test ne
bronche**. Ce qui protège cette archive n'est pas le test — c'est le caviardage
fait à la main et **relu** (§2.2). **Relire, ne pas se fier au vert.**

⚠️ **Le trou est NOMMÉ, NON CORRIGÉ** : `secrets.test.ts` appartient au
sous-projet ⑤, et élargir sa classe de caractères est une décision qui lui
revient.

---

## 3. 🔴 Ce qui N'EST PAS archivé : `docker-compose.yml`

**Il est délibérément absent, et il ne doit pas y entrer.** `docs/` est
**versionné** : y déposer ce fichier reviendrait à committer des mots de passe
Windows en clair — exactement la faute qu'un test du dépôt
(`plateforme/src/securite/secrets.test.ts`) vient d'attraper ailleurs, et dont le
commit `aad98d9` a retiré la trace.

**À la place, sa STRUCTURE, sans une seule valeur** (relevée le 21 août 2026 en
masquant tout ce qui suit un `=` ou un `:`) :

- **un seul service**, nommé `web` ;
- `build:` depuis la racine du dépôt, `network_mode: host`, `privileged: true` ;
- **deux montages** : la racine du dépôt vers `/opt/server`, et `/media/vm` vers
  `/media/vm` ;
- **un port publié** : `3445` de l'hôte vers `8080` du conteneur ;
- **une politique de redémarrage** (voir le §4 ci-dessous) ;
- **six variables d'environnement, désignées par leur NOM et par rien d'autre** :
  `TZ`, `WINDOWS_HOSTNAME`, `WINDOWS_USERNAME`, `WINDOWS_PASSWORD`,
  `WINDOWS_ADMIN_USERNAME`, `WINDOWS_ADMIN_PASSWORD` ;
- un commentaire de pied renvoyant au relais TURN, qui vit dans
  `docker-compose.coturn.yml` — **celui-là versionné, et sans secret**.

**Les quatre dernières de ces six sont des identifiants.** Leurs valeurs ne sont
écrites nulle part dans `docs/`, et ne doivent jamais l'être.

---

## 4. Le geste voisin du même jour : la politique de redémarrage

`docker-compose.yml` portait `restart: always`. Il porte désormais
`restart: unless-stopped`, **et c'est la seule directive changée**.

**Pourquoi** : le conteneur `guacamole-web-1` est arrêté volontairement depuis le
20 août 2026 à 05:53:54 UTC, et le verrou **C6** du retrait exige **sept jours**
d'arrêt sans réclamation. Sous `always`, le prochain démarrage du démon Docker
relancerait le conteneur et **remettrait cette fenêtre à zéro**.

🔴 **ET CE GESTE NE SUFFIT PAS — c'est mesuré, pas déduit.** Un conteneur porte la
politique qui lui a été donnée **à sa création** ; éditer le fichier de
composition ne la change **pas** rétroactivement. Après l'édition,
`docker inspect guacamole-web-1` rend toujours `RestartPolicy=always`.

**Le fichier est donc juste pour tout `up` futur, et le conteneur EXISTANT reste
exposé.** Le refermer demande un geste sur l'état Docker, **qui n'a pas été
joué** :

```bash
docker update --restart=unless-stopped guacamole-web-1   # NON JOUÉ
```

⚠️ Il ne démarre rien — c'est un changement de métadonnée —, **mais il sort du
périmètre d'écriture de ce chantier, qui est le fichier.** *Nommé, non fait.*

---

## 5. Ce que le remplacement NE COUVRE PAS

⚠️ **Cette liste n'est pas recopiée ici** : elle vit au **§4 du relevé**, en
**trois familles** qu'il ne faut pas confondre — **A**, des fonctions qui
*disparaissent* ; **B**, des fonctions livrées que *rien n'a mesurées* ; **C**,
des fonctions mesurées mais *bornées par un plafond* là où le legacy ne l'était
pas (ou n'était pas mesuré du tout).

**Les trois trous que personne ne reprend**, nommés ici parce qu'ils justifient à
eux seuls que l'archive précède toute suppression :

- 🔴 le **service worker** — aucun code dans le nouveau produit, aucun sous-bloc ;
- 🔴 les **formats riches du presse-papier** — régression **décidée**, pour une
  raison de canal ;
- 🔴 le **tactile** — régression **NON décidée**, qu'aucune spécification du
  nouveau produit ne nomme. Les handlers du legacy étaient **vivants**
  (`web-index.js:271`, `:282`, `:295`), et l'archive est désormais le seul endroit
  où ces trois lignes survivent à une suppression.

**Le détail, ses pièces et ses réserves sont dans le relevé. Ne pas le
paraphraser.**

---

## 6. La règle des 500 lignes

`web-index.js` fait **804 lignes**, donc au-dessus du plafond. **Ce n'est pas une
dette, et il n'y a rien à inscrire**, sur **trois** motifs indépendants :

1. la **Portée** de la règle est nommée — `agent/src/`, `client/src/`,
   `plateforme/`, `proto/`, `src/`, `web/`, `scripts/` — et `docs/` n'y figure
   pas ;
2. la règle vise « le code source **écrit à la main** » : une archive est une
   **copie**, et découper un fichier d'archive le rendrait inutilisable comme
   référence — c'est précisément ce que les 29 citations à numéro de ligne
   interdisent ;
3. `docs/` est **explicitement exempté**. ⚠️ *Ce troisième motif est le plus
   faible des trois, et il faut le dire : la justification écrite de l'exemption
   nomme « les plans, specs et recettes », et une archive de code n'est aucun des
   trois. Ce sont les motifs 1 et 2 qui portent.*

**Vérifié par la commande** : le tableau de dette a toujours **deux** entrées
(`agent/src/encode.rs` **1536**, `agent/src/windows_source.rs` **630**), et
l'archive en est **structurellement absente** — la commande filtre `^docs/`.

⚠️ **À ne pas confondre avec une dette voisine, réelle et NON inscrite** : le
fichier **vivant** `web/index.js` est, lui, dans la portée (`web/`), fait **804
lignes**, et **échappe à la commande de vérification parce qu'il est ignoré par
git**. La table en compte donc deux là où le texte de sa propre portée en impose
trois. **C'est la seconde moitié du bloc L1 du retrait ; elle n'est pas faite.**

---

## 7. Le piège qui a failli rendre cette archive invisible

🔴 **Les fichiers ne s'appellent PAS `index.js`, et c'est mesuré, pas
esthétique.** Le motif `.gitignore:18` étant **non ancré**, il attrape le nom
`index.js` **à n'importe quelle profondeur** :

```
git check-ignore -v docs/legacy/index.js      → .gitignore:18:index.js
git check-ignore -v docs/legacy/web/index.js  → .gitignore:18:index.js
```

**Une archive déposée sous son nom d'origine aurait été ignorée EN SILENCE**, et
l'on aurait cru avoir sauvé 890 lignes sans en avoir sauvé une. Les noms
`web-index.js` et `racine-index.js` échappent au motif — **vérifié, et avec un
témoin qui montre que la commande sait dire « ignoré » quand elle doit**.

⚠️ **La mine reste armée pour les autres** : un futur `client/src/index.js` serait
ignoré sans un mot. Le point d'entrée de la plateforme s'appelle `index.ts` et y
échappe **par hasard**.

---

## 8. Ce que cette archive N'ÉTABLIT PAS

- **Elle ne débloque aucun verrou.** L'état du retrait est inchangé.
- **Elle ne purge aucun secret de la machine.**
- **Elle n'archive pas les 16 autres fichiers du legacy** : ils sont **suivis par
  git**, donc récupérables par l'historique. L'archive vise ce qui ne l'est pas.
- **Elle n'archive pas les sections legacy de `CLAUDE.md`** — c'est un travail du
  bloc **L4**, et `CLAUDE.md` n'a pas été touché.
- **Elle ne compare pas les deux produits.** Rien, dans ce dépôt, n'oppose une
  mesure de l'ancien à une mesure du nouveau.
- **Elle n'a lancé aucune commande vers la VM Windows**, ni démarré, ni arrêté, ni
  redémarré quoi que ce soit.
