# Legs ouverts — décisions du propriétaire et dû du chantier auth-pomerium

> Extrait de [`CLAUDE.md`](../../CLAUDE.md) (lignes 1129-1262 de l'original), déplacé tel quel. Liens relatifs à ce dossier.

## 🔴 Legs ouverts

**Consolidé au 21 août 2026, depuis les § « Ce que … lègue » et « Ce que … n'établit PAS ».**
Le détail et les pièces de chacun sont dans [`docs/JOURNAL.md`](../JOURNAL.md).

### Décisions qui appartiennent au propriétaire du dépôt

Aucune n'est une correction, et aucun chantier ne doit les prendre en douce.

- ✅ ~~🔴 **`403 vm-etrangere` contre `404 vm-inconnue`** — le même service rend
  deux réponses différentes pour la même situation.~~ **TRANCHÉ le 21 août 2026
  en faveur du `404` — ET LE CODE L'APPLIQUAIT DÉJÀ : CE LEGS ÉTAIT PÉRIMÉ, PAS
  OUVERT.** Relevé le jour de la décision : `vm-etrangere` **n'est émis nulle
  part** (`grep -rn 'vm-etrangere' plateforme/src --include='*.ts'` ne rend que
  des commentaires historiques), il **n'est pas un membre du type `Motif`**
  (`plateforme/src/orchestration/refus.ts`), et `CODE_HTTP` mappe
  `'vm-inconnue' → 404`. Les `403` qui subsistent portent sur `jeton-agent` —
  un jeton d'agent employé sur une route d'utilisateur — ce qui est **une autre
  question, et une réponse juste**. ⚠️ **Aucune ligne de code n'a été modifiée
  par cette décision** : elle n'a fait que constater. 🔴 **La leçon vaut plus
  que le legs : un § « Legs ouverts » consolidé À LA MAIN vieillit comme
  n'importe quel relevé daté, et celui-ci affirmait une contradiction que le
  produit avait déjà résolue.**
- 🔴 **L'écho acoustique**, trois voies : ① rassembler la restitution (**défait
  D7**) ; ② une AEC côté agent (**que la spec exclut nommément**) ; ③ le casque,
  **dit au bon moment** — la seule dont le défaut mesuré ait encore besoin, et
  la seule livrable par un mécanisme déjà construit.
- ✅ ~~**Le retrait du legacy** — arrêté, à moitié archivé, jamais retiré.~~
  **TRANCHÉ ET EXÉCUTÉ le 21 août 2026** : voir « Le legacy » plus haut. Ce
  qu'il emporte — les fonctions que personne ne reprend — n'est PAS un legs
  ouvert mais une **régression assumée**, et elle est nommée là-bas.
- ⚠️ **La portée de la règle des 500 lignes** : le § « Portée » ne liste que
  `client/src/`, la commande attrape `client/verify-webrtc.mjs`. **Signalé
  depuis D10, jamais tranché.**
- ⚠️ **`FilterAdministratorToken=1` sur la VM** — sans lui, la porte d'élévation
  de G3 reste **non mesurable**, et un installeur qui exige une élévation
  s'exécute **sans aucune boîte de dialogue**.

### Ce que le chantier `auth-pomerium` laisse dû (21 août 2026)

🔴 **INSCRIT ICI ET NON DANS LE SEUL DOCUMENT DE RÉSULTATS, PARCE QUE CE DÉPÔT
VIENT DE CONSTATER QU'UN LEGS QUI NE VIT QUE DANS UN RELEVÉ DATÉ EST UN LEGS
PERDU** — c'est la leçon du legs `403/404`, déclaré « ouvert » alors que le
produit l'avait résolu, et de six constats de revue disparus avec un rapport
gitignoré.

- 🔴 **LE CRITÈRE ⑦ RESTE NON JOUÉ** — la page, dans un navigateur réel,
  derrière Pomerium — ~~**mais ses deux blocages ne sont PAS de même nature,
  et les confondre est l'erreur à éviter**~~ **CE N'EST PLUS VRAI QUE D'UN
  SEUL DES DEUX, DEPUIS LE 22 AOÛT 2026 :**
  - ① ~~**un défaut de CONCEPTION, déjà appliqué au `config.yaml` réel** : la
    route nue de Pomerium (spec §7.2, commentée « La page et l'API ») vise
    `http://192.168.3.1:8080`, c'est-à-dire la plateforme — **qui ne sert
    aucun fichier statique**, `GET /` y rendant `404 introuvable` (mesuré).
    C'est nginx qui sert la page, et ce bloc le saute. Il reste à CONCEVOIR,
    pas à mesurer : soit router Pomerium vers nginx (…), soit doter la
    plateforme d'un servant statique. Aucune des deux n'est tranchée.~~
    **LEVÉ.** La plateforme sert désormais la page bâtie
    (`plateforme/src/http/page/`, variable `PLATEFORME_PAGE` — voir le
    tableau des variables serveur), donc la route nue de la spec §7.2, qui
    vise `http://192.168.3.1:8080`, vise un backend qui répond
    (`GET /` y rend la page si `PLATEFORME_PAGE` est posée vers `client/dist`
    bâti, ou le `404` d'hier si elle ne l'est pas). Voir spec `auth-pomerium`
    § 7, dont l'annotation du 21 août 2026 est levée à son tour.
  - ② **DEMEURE, seul désormais** : une limite de recette, celle-là
    ordinaire — le flux OAuth Google exige **un humain**, et aucun Chrome
    sans interface ne le franchit. ⚠️ **Le critère ⑦ reste donc NON JOUÉ**,
    et écrire « critère ⑦ levé » serait faux : lever un blocage de
    conception ne joue pas le critère à sa place.
- ~~🔴 **AUCUN FREIN SUR `/auth/moi`, ET C'EST DÉSORMAIS LA SEULE SURFACE
  D'AUTHENTIFICATION** en mode `pomerium`. `securite/frein.ts` n'est consulté
  que par `servirAuth` (`grep -ln 'deps\.frein' plateforme/src/http/routes-*.ts`
  ne rend que `routes-auth.ts`), or ce routeur **se retire** dans ce mode. La
  route **crée une ligne `utilisateur` par courriel distinct**, sans borne :
  qui atteint le port `8080` — dont la VM Windows — fait grossir la table à
  volonté, l'en-tête n'étant vérifié par aucune signature. Non mesuré, et ce
  n'est pas une raison de l'écrire moins fort : c'est une lecture de code,
  elle est dite comme telle.~~
  🔴 **REQUALIFIÉ ET FERMÉ, 22 août 2026.** Ce legs se lisait comme un frein
  manquant, appelant une borne de cadence — **ce n'en était pas un**. C'était
  un **contournement COMPLET de l'authentification** : quiconque atteignait le
  port `8080` — dont la VM Windows — obtenait, par un simple en-tête
  `X-Pomerium-Claim-Email` qu'AUCUNE signature ne vérifiait, un jeton interne
  valide pour l'identité de son choix. Un frein n'aurait borné que la
  **cadence** d'un contournement qui n'a besoin d'aboutir **qu'une fois**. La
  garde qui ferme ce trou est `PLATEFORME_PROXY_DE_CONFIANCE`, désormais
  **obligatoire en mode `pomerium`** (`plateforme/src/config.ts::lireConfig`,
  qui refuse de démarrer sans elle) : `routes-identite.ts::servirIdentite`
  n'accepte l'en-tête que d'un pair dont l'adresse socket figure dans cette
  liste (`pairDeConfiance`), et rend `401 pair-non-de-confiance` **avant même
  de la lire** sinon. Voir sa ligne dans le tableau des variables serveur.
- ~~🔴 **ONZE PILOTES DE RECETTE VISENT UNE RACINE QUE CE CHANTIER A FERMÉE.** Le
  relais a quitté `/` pour `/signal`, et `?signaling=` reste **explicite** —
  il ne reçoit pas le suffixe. Or les pilotes de
  `docs/superpowers/plans/journaux-*/instrument/` posent tous une URL **sans
  chemin** : `accent-a1`, `micro-e3` (le pilote et son `injection-e3.js`),
  `pont-fichiers` f1 à f5, `presse-papier` p1 à p3 — **plus un douzième hors de
  ce répertoire**, `journaux-micro-e2/pilote-recette-e2.mjs`. **Aucun n'a été
  réparé** : c'est un chantier à part.~~ **FERMÉ (lot `legs-sans-vm`)** : onze
  des douze pilotes réparés vers `/signal` (le douzième,
  `injection-e3.js`, n'avait rien à changer — sa valeur vient déjà corrigée de
  son pilote appelant). ⚠️ **Aucun pilote n'est rejoué** : la VM Windows est
  hors périmètre de ce lot, le contrôle joué est `node --check` sur chaque
  fichier modifié. 🔴 **LE REJEU A ÉTÉ TENTÉ LE 5 SEPTEMBRE 2026 : AUCUN DES
  DOUZE N'ÉTABLIT DE SESSION — ET LA CAUSE DOMINANTE N'EST PAS UN DÉFAUT DE
  PILOTE.** **Sept** s'arrêtent sur `POST /auth/connexion` → **404**, le
  service tournant en mode **`pomerium`** (mesuré sur le processus qui
  écoute : `PLATEFORME_AUTH=pomerium`), où cette route est **retirée** ; les
  pilotes ont été écrits pour `motdepasse`. ⚠️ **CE QUI N'EST PAS LA CAUSE, ET
  QUI A ÉTÉ VÉRIFIÉ AVANT DE CONCLURE** : neuf portent
  `ws://192.168.3.1:8080`, mais c'est un **DÉFAUT de paramètre**, pas une
  adresse en dur — tous les bras ont été joués avec `SIGNALING_WS` **imposé**,
  et ils échouent ailleurs. 🔵 **12/12 passent `node --check`.** 🔴 **La rouge
  prescrite est VACUEUSE ici, et c'est mesuré** : la mutation de
  `pilote-f1.mjs` rend **la même erreur** avant et après — l'exécution
  n'atteint jamais la ligne mutée.
  [Verdicts des douze](../superpowers/plans/journaux-lot3-pilotes/verdicts.md)
  ⚠️ **Le commentaire qui affirmait qu'« aucune recette n'en
  pose » a été corrigé** dans `client/src/adresse-plateforme.ts` : son `grep`
  ne couvrait pas `docs/superpowers/`, où vivent TOUS les pilotes — patron du
  « naufrage du 487 ».
- ~~🔴 **LE CONTRAT DE `SIGNALING_URL` N'EST FIGÉ PAR AUCUN TEST.** La variable
  est **la BASE du service**, jamais l'URL du relais : `url_du_relais` y ajoute
  `/signal`, `url_du_canal` y ajoute `/agent`. **Y écrire `/signal` casserait
  l'enrôlement** (`ws://h:8080/signal/agent`) **sans qu'aucun test ne
  bronche** — le test `the_agent_channel_is_not_affected` d'`agent/src/
  signaling.rs` passe une base PROPRE, donc n'éprouve pas ce cas, alors que son
  commentaire prétendait le fermer. Le commentaire est corrigé ; **le test
  manquant, lui, reste dû.**~~ **FERMÉ (lot `legs-sans-vm`)** : le test
  `a_base_already_carrying_signal_breaks_the_agent_channel` (`agent/src/
  signaling.rs`) joue désormais ce cas et fige le contrat — vérifié VERT sur
  le produit d'aujourd'hui, puis rougi par mutation ciblée d'`url_du_canal`,
  restaurée depuis une copie nommée.
