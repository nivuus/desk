# Lot 22 — le hub mène au bureau, et la sortie virtuelle cesse de naître à 1280×720

Date : 30 août 2026. Branche `package-nivuus`. Commit **`3fb7c91`**.

---

## 0. Deux murs, pas un

Le lot a commencé sur le mur nommé par le lot 20 (« le superviseur annonce,
personne n'écoute ») et en a rencontré un second en cours de route, signalé par
le coordinateur : la sortie virtuelle ne pouvait plus servir aucun viewport.
**Les deux sont fermés.** Le premier dans le dépôt, le second sur la VM.

---

## 1. Mur 1 — la racine ne menait à aucun bureau

### La voie retenue : (b), un chemin de navigation visible, et pourquoi

Le diagnostic du lot 20 nommait deux voies. **(b) est retenue**, et la raison
n'est pas de goût :

🔴 **le rôle `client` est EXCLUSIF** (`plateforme/src/signaling/
appariement.ts::declarer`), et c'est **mesuré** : le 30 août 2026 à 08:03:57 et
à 08:16:30, mon pilote s'est vu répondre
`{"type":"error","reason":"un client est déjà connecté à la session
3sxuA9dd56NpVdHi37R86g:bureau"}` — la place qu'occupe la page du propriétaire.

Si le hub prenait ce rôle, **toute PWA par application** — dont le `start_url`
est `shell.html?app=<id>` (`client/src/hub/manifeste.ts:244-245`), et dont le
propriétaire vient d'installer un exemplaire (Notepad) — **se verrait refuser
la session de contrôle dès qu'un onglet du hub serait ouvert quelque part**.
On ne casse pas un chemin qui marche pour en réparer un autre.

**Ce que (b) implique pour une PWA par application déjà installée : RIEN ne
change.** Elle ouvre `shell.html?app=<id>`, prend le rôle `client`, lance
l'application à l'ouverture du socket (`shell-page.ts::lancerLApplicationDemandee`)
et reçoit ses annonces, exactement comme avant. La seule conséquence nouvelle :
si l'utilisateur ouvre AUSSI le bureau depuis le hub pendant qu'une PWA tourne,
les deux se disputent le rôle exclusif et le second reçoit un bandeau rouge
(`shell.ts::canalDeControleRefuse`, déjà livré). Ce n'était pas atteignable
faute de chemin ; ça l'est désormais. **Dit, pas maquillé.**

La troisième voie du diagnostic — fusionner les deux surfaces — est une
décision de conception qui appartient au propriétaire.

### Ce que j'ai fait du `window.open` hors geste utilisateur

**Je ne l'ai pas déplacé d'un cran, et je ne l'ai pas retiré non plus.**

- `shell-page.ts:144` reste tel quel : il ouvre la fenêtre de session en
  réponse à un message WebSocket, donc hors geste, et il garde son repli **déjà
  livré et déjà testé** — la carte de la fenêtre dans `#fenetres` avec son
  bouton « Rouvrir » (`shell.ts::rouvrir`, `shell.html`), qui est **un geste**,
  plus le bandeau rouge « le navigateur a bloqué la pop-up ».
- Le maillon que ce lot ajoute, lui, **part d'un clic** : l'en-tête du hub
  porte un vrai `<a>` (jamais un bouton qui appelle `window.open` — une
  navigation par lien n'est bloquée par aucun navigateur), et le clic sur
  « Lancer » ouvre le bureau **avant** le `POST /lancer` (un `await`
  intercalé consommerait l'activation transitoire).
- Un bureau bloqué **n'empêche pas le lancement** : il est **dit**
  (« a été lancée, mais le navigateur a bloqué l'ouverture du bureau »).
  Refuser de lancer ferait d'une gêne une panne ; le taire ramènerait la panne
  d'origine.

🔴 **Aucun troisième mécanisme n'est inventé.** La fenêtre nommée
`nivuus-bureau` fait que deux clics retrouvent LE MÊME bureau — un second se
verrait refuser le rôle `client`.

### Le test de parcours — `client/src/parcours.test.ts`

Il modélise le parcours par les deux maillons que le code porte : ① la page
servie à la racine mène-t-elle à la surface qui traite `fenetre-ouverte` ?
② cette surface, recevant l'annonce, ouvre-t-elle la fenêtre ? **Le ② était
vert toute la soirée ; le ① était rouge et rien ne le regardait.** Il ne
recopie aucune valeur : la page servie est lue dans `resolution.ts`, l'entrée
de chaque page dans son `.html`, le graphe d'imports parcouru sur les fichiers
réels.

**ROUGE, sur le produit d'avant le correctif** (`hub.html` et `page.ts` remis
par `git stash`, `bureau.ts` écarté ; copies nommées sous
`/var/tmp/lot22-copies/`) :

```
 × le parcours : ouvrir le produit, lancer une application, voir sa fenêtre >
   ① la page servie à la racine mène à la surface qui traite « fenetre-ouverte »
   → pages nommées depuis hub.html :  : expected [] to not have a length of +0
 Test Files  1 failed (1)
      Tests  1 failed | 1 passed (2)
```

**VERTE, après** :

```
 ✓ src/hub/bureau.test.ts (4 tests) 7ms
 ✓ src/parcours.test.ts (2 tests) 9ms
 Test Files  2 passed (2)
      Tests  6 passed (6)
```

Suite complète : **596 tests client (54 fichiers)**, **308 proto**,
`tsc --noEmit` propre, **7/7 contrôles de design**, aucun fichier au-dessus de
500 lignes hors dette déclarée (`encode.rs` 1536, `windows_source.rs` 630).

### Déploiement, prouvé

`npm run build`, `rsync` vers `/opt/nivuus/desk/client/dist/`,
`chmod -R a+rX /opt/nivuus/desk/client`. **Aucun redémarrage du service** — donc
la connexion de contrôle de l'agent n'a pas été tuée.

```
$ curl -s -o … -w 'http=%{http_code} octets=%{size_download}\n' http://192.168.3.1:3445/
http=200 octets=4707
<title>Applications</title>
id="bureau" class="bouton bouton--secondaire">Mon bureau<
$ curl -s -o /dev/null -w 'http=%{http_code}\n' http://192.168.3.1:3445/shell.html
http=200
$ curl -s http://192.168.3.1:3445/assets/hub-BDPvzqYc.js | grep -c 'shell.html'
1
$ systemctl is-active desk-plateforme ; systemctl show desk-plateforme -p NRestarts
active
NRestarts=0
```

---

## 2. Mur 2 — la sortie virtuelle naissait à 1280×720

### La cause, établie

Le pilote crée bien la sortie à la taille demandée
(`sortie virtuelle créée … largeur=1860 hauteur=1080`), mais celle qui APPARAÎT
dans la topologie DXGI est `\\.\DISPLAY6 **1280x720**`, refusée par
`agent/src/superviseur/placement.rs::sortie_assez_grande`. **Ce n'est donc PAS
la cause n°1 du commentaire de `creation_sortie.rs:108-118` (« aucune sortie
n'est apparue »), c'est la n°2 (« plus PETITE que la demande »)** — les
`apparues=[]` sont les tours où le rattachement n'avait pas encore eu lieu dans
les 5 s de `LIMITE_RATTACHEMENT`.

🔴 **`agent/src/moniteurs_virtuels/pilote.rs:269`** donne à chaque sortie un
numéro de série EDID **déterministe**, `mesure{numero}`, et le distributeur
repart de 1 à chaque processus. Windows retient un mode d'affichage **par
série** : la clé

```
HKLM\SYSTEM\CurrentControlSet\Control\GraphicsDrivers\Configuration\
  MSBDD_NOEDID_1234_1111_00000000_00010000_0+SMKD1CEmesure1_20_07E8_19^F955…
```

portait `PrimSurfSize 1280x720` — laissé là par les sondes de l'époque D, dont
`montee::RESOLUTION` vaut précisément 1280×720 — et Windows le réappliquait à
toute sortie neuve. Les autres clés de la même racine gardent les traces des
anciennes séries : 3840×2160, 5120×1440, 2410×1080, 2142×960, 1920×1080,
800×600 — c'est la « pollution du registre » de `CLAUDE.md`, **nommée par sa
mécanique** pour la première fois.

**Sorties orphelines comptées** (le coordinateur l'avait demandé) : **dix**
nœuds de moniteur fantômes `DISPLAY\SMKD1CE\…UID256` à `UID265`, tous
`Status=Unknown`. Ils ne bloquent rien — les identifiants du pilote (256…265)
tournent en rond et chaque création réussit —, mais ils datent des arrêts
brutaux de la nuit.

### Le remède, appliqué, et sa vérification

**Copie nommée d'abord** : `reg export` de `Configuration` (70 036 o) et de
`Connectivity` (8 162 o) vers `C:\nivuus\state\lot22-configuration-avant.reg`
et `…-connectivity-avant.reg`. Puis retrait des **deux** clés `*mesure1*`
(une dans chaque racine). **Aucun redémarrage de la VM, aucun binaire
remplacé.**

**Vérifié par le produit lui-même**, lancement réel de Notepad par
`POST /application/<id>/lancer` :

```
08:07:51.505  sortie virtuelle créée id=265 … largeur=1860 hauteur=1080
08:07:51.638  enfant lancé session=…:w-73 pid=6732 sortie=\\.\DISPLAY6
08:07:51.665  attaché au capteur session=…:w-73 sortie=\\.\DISPLAY6 largeur=1860 hauteur=1080
08:07:51.707  déclaration de l'agent émise au signaling session="…:w-73"
```

**Avant/après, sur le même journal** : **zéro** `enfant lancé` entre 01:09:19 et
08:07:51 — toute la nuit, tous refusés —, puis **huit** entre 08:07:51 et
08:15:41, sur `\\.\DISPLAY6` à `\\.\DISPLAY12`, **toutes à 1860×1080**.
Relevé versionné : `docs/superpowers/plans/journaux-lot22/
agent-sorties-avant-apres.log`.

### Ce qui reste un défaut de PRODUIT, non corrigé (sur consigne)

🔴 **`agent/src/moniteurs_virtuels/pilote.rs:269`** : la série `mesure{numero}`
est déterministe et réutilisée, donc **tout mode que Windows retiendra un jour
pour `mesure1` replafonnera le produit entier au démarrage suivant**. Le remède
est un numéro de série unique par sortie (ou par exécution). Il touche l'ABI du
pilote et mérite sa propre mesure : **je ne l'ai pas pris en douce.**

⚠️ Second candidat, à trancher par le propriétaire :
`superviseur/placement.rs::sortie_pour_viewport` **refuse** une sortie plus
petite au lieu de retomber sur la plus grande apparue. Un repli (fenêtre posée
à la taille de la sortie, `taille_retenue` fait déjà le `min`) aurait rendu le
produit inébranlable à cette famille, au prix d'une image plus petite. **Non
appliqué** : c'est un arbitrage de qualité d'image, pas une correction.

---

## 3. L'épreuve au signaling : un pair reçoit-il `fenetre-ouverte` ?

**OUI — mais le pair n'est pas mon pilote, et c'est la mesure elle-même qui
l'explique.**

Le pilote `docs/superpowers/plans/journaux-lot22/instrument/pilote-parcours.mjs`
(écrit après lecture de `journaux-lot17/instrument/pilote-pair-tardif.mjs`, dont
il reprend la structure ; les trois différences sont listées dans son en-tête)
s'est vu **refuser le rôle `client` aux deux exécutions** — 08:03:57 et
08:16:30 — parce que **la page du propriétaire l'occupait**. Relevés :
`parcours-rouge-1860.json`, `parcours-verte-1860.json`.

C'est ce même refus qui prouve qu'un pair est là ; et le journal de l'agent
prouve ce qu'il fait : les lancements de 08:07:51 et 08:16:36, passés par la
**route de la plateforme**, ont produit `fenetre-ouverte`, **un viewport en
retour** (sans quoi aucune sortie ne serait créée) et **huit `enfant lancé`**.
Le juge est donc atteint par un pair **réel**, ce qui vaut mieux qu'un pilote —
mais **il n'est pas atteint par un instrument que je contrôle**, et je le dis.

---

## 4. Ce que je n'ai PAS pu établir

- **Qu'un média traverse.** Aucune session WebRTC n'a été jugée : le pilote n'a
  pas de pile WebRTC, et le navigateur réel reste hors d'atteinte (OAuth exige
  un humain — blocage ② du critère ⑦ d'`auth-pomerium`).
- **Qu'un humain voie la fenêtre.** Le parcours est établi par le code et par le
  journal de l'agent, jamais par un œil. Aucun jugement visuel n'est porté sur
  le bouton « Mon bureau » ni sur le nouveau message d'échec.
- **Que le clic « Lancer » ouvre effectivement le bureau dans un navigateur
  réel.** `window.open` sous activation transitoire n'est pas exerçable ici ;
  le test couvre la RÈGLE, pas le navigateur.
- **Le plafond de concurrence.** À 08:16, avec **sept sorties attachées**, les
  créations suivantes échouent de nouveau (`apparues=[]`, puis
  `apparues=["\\.\DISPLAY8 1860x1080"]` déjà prise). C'est le plafond documenté
  du chantier D, atteint parce que **huit fenêtres sont désormais réellement
  servies** — pas une régression, et **non mesuré** par ce lot.

## 5. Réserves et état laissé

- 🔴 **`superviseur/signalisation.rs` ne se reconnecte toujours JAMAIS.** Je
  n'ai **pas** redémarré `desk-plateforme` (NRestarts=0, actif depuis 09:35:48)
  et l'agent n'a **pas** été relancé : ses trois processus tournent depuis
  09:43, en session 1. **Quiconque redémarrera ce service devra relancer
  l'agent derrière.**
- ⚠️ **J'ai lancé Notepad trois fois** par la route de la plateforme (08:03,
  08:07, 08:16). Huit Notepad tournent sur la VM et huit sorties sont prises :
  **le propriétaire gagnera à en fermer quelques-unes**, sans quoi tout nouveau
  lancement bute sur le plafond de concurrence.
- ⚠️ Le premier lancement (08:03) a produit des refus sur la page du
  propriétaire, avant le remède. C'était le bruit du diagnostic.
- Les deux clés de registre retirées sont **restaurables** depuis
  `C:\nivuus\state\lot22-*-avant.reg`.
- Rien n'a été défait : la racine sert le hub, `icone_url` signée,
  `manifest-src 'self' blob:`, `jeton.ts::assurerAcces`, `pair-present`.
