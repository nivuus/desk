# Réveil de la VM au lancement d'une app, arrêt après 30 min d'inactivité — conception

Date : 30 septembre 2026. Statut : **à relire** (aucun code écrit).
Touche deux dépôts : `desk` (plateforme, client) et `installer` (package `console`).

## 1. Ce qui est demandé

1. Lancer une app depuis le hub **réveille la VM** si elle est éteinte ou en hibernation.
2. La VM **s'éteint** (hibernation, comme aujourd'hui) après **30 min sans aucune session
   Moonlight et sans aucune session d'app**. Le garde-fou CPU existant est **conservé**
   (décision du propriétaire, 30 septembre 2026) : une VM dont le CPU reste ≥ 50 % n'est pas coupée.

Critères de succès :

- VM éteinte, l'utilisateur lance une app : la VM démarre, le hub affiche un état « démarrage »,
  puis la session s'ouvre sans action manuelle.
- Une session d'app ouverte maintient la VM allumée ; 30 min après la fermeture de la dernière
  session (et sans flux Moonlight/RDP, CPU calme), la VM hiberne.
- Aucun privilège hôte n'est donné à la plateforme au-delà de « réveiller » et « signaler ».

## 2. Ce que le code fait aujourd'hui (relevé du 30 septembre 2026)

- La VM n'a pas d'`autostart` (`virsh dominfo Windows` : `Autostart Once: désactiver`). Le réveil à
  la demande vient de `console/host/vm-wake-gate.py`, déclenché **uniquement** par une requête
  Moonlight (`GET /serverinfo`, port 47989), qui appelle `handle-vm-start.sh`. Aucun chemin WebRTC
  ne l'appelle.
- `OrchestrateurStatique.demarrer()` (`plateforme/src/orchestration/inventaire-statique.ts`) refuse
  (`non-supporte`). `routes-session.ts` répond `503` avec `redemarrage.possible: false`.
- `EtatVm = EtatAgent` n'a que deux membres (`prete`, `injoignable`).
- `vm-idle-shutdown.sh` (timer 10 min, 3 passages = 30 min) juge l'activité sur les flux conntrack
  Moonlight/RDP et le CPU de la VM. Il ignore les sessions d'app WebRTC.
- La plateforme est le service systemd `desk-plateforme.service` avec `DynamicUser=yes` et
  `NoNewPrivileges=yes` : elle ne peut pas appeler `virsh`.
- 🔴 **Le chemin de lancement d'une app est `POST /application/:id/lancer`**
  (`http/routes-applications.ts`), pas `/session` : il appelle `RegistreAgents.lancer`, qui rend
  `agent-injoignable` quand aucun socket d'agent n'est ouvert (VM éteinte), d'où un `503
  {refus:'agent-injoignable'}`. Le hub (`client/src/hub/page.ts`) affiche alors
  « n'a pas pu être lancée : agent-injoignable ». **Correction du 30 septembre 2026** : la première
  rédaction de cette spec plaçait le réveil sur `/session`, ce qui n'aurait rien changé au lancement.
- `handle-vm-start.sh` **bloque** jusqu'à ce que la VM ait une IP (jusqu'à 180 s) puis pose la règle
  de redirection du port 47984. Le canal ne peut donc pas l'exécuter en ligne : il le démarre comme
  unité `oneshot` avec `systemctl start --no-block`.
- Le relais de signaling expose le port `ObservateurDeSession` (`apparie` / `separe`), implémenté en
  production par `signaling/trace.ts`.

## 3. Approche retenue : un socket Unix de contrôle, fourni par `console`

Écartées : (A) un faux `GET /serverinfo` sur 47989 — couplage au protocole Moonlight et
comportement de scanner ; (C) le script d'arrêt qui interroge l'API de la plateforme —
authentification côté hôte et couplage de `console` à l'API de `desk`.

Recherche préalable : il n'existe pas de bibliothèque à adopter ; c'est de l'activation par socket
systemd standard (`ListenStream=` + `Accept=yes`), déjà employée par `vm-trigger-*.socket`.

### 3.1 Canal hôte — dépôt `installer`, `console/host/`

- `systemd/nivuus-vm-control.socket` : `ListenStream=/run/nivuus/vm-control.sock`,
  `SocketMode=0660`, `SocketGroup=nivuus-vm`, `Accept=yes`.
- `systemd/nivuus-vm-control@.service` : lit **une ligne** sur stdin, exécute le verbe, répond
  sur stdout, sort. Sans état.
- Groupe `nivuus-vm`, déclaré par un fichier `sysusers.d` (mécanisme standard, pas de `groupadd`
  maison) posé par l'installation de `console` et appliqué par `systemd-sysusers` à l'activation ; `desk-plateforme.service` reçoit
  `SupplementaryGroups=nivuus-vm` (un seul ajout d'unité, aucun autre privilège).
- Protocole : une ligne texte, verbes en liste blanche, tout le reste refusé (`err unknown-verb`).
  - `wake` → `systemctl start --no-block nivuus-vm-wake.service` (unité `oneshot` qui exécute
    `handle-vm-start.sh` : idempotent, `flock`, gère les états transitoires). Réponse `ok` dès que
    le démarrage est **demandé** (pas terminé) ; `err <cause>` si `systemctl` échoue.
  - `busy` → écrit l'horodatage courant dans `/run/nivuus-vm-idle/app-activity`, réponse `ok`.
- Le canal ne prend **aucun paramètre** : pas de nom de VM, pas d'argument. La VM est celle de
  l'hôte (`VM_NAME` du script existant).

### 3.2 Arrêt — `vm-idle-shutdown.sh`

Une passe est **active** (strikes remis à 0) si l'une de ces conditions est vraie :

1. flux Moonlight/RDP établis (inchangé) ;
2. CPU ≥ 50 % (inchangé, conservé) ;
3. **nouveau** : `app-activity` date de moins de `APP_ACTIVITY_MAX_AGE_S` (**180 s**, soit 3 battements
   de 60 s manqués).

🔴 **Correction du 30 septembre 2026 (relecture du plan)** : la première rédaction disait « moins de
30 min », ce qui comptait le délai **deux fois**. Le signal `busy` dit « une session d'app est
ouverte *maintenant* » (il est renvoyé toutes les 60 s) ; les **30 min** viennent des 3 passages de
10 min déjà en place, qui ne commencent à compter qu'une fois le signal éteint. Avec 30 min sur le
signal, la VM aurait tenu 50 à 60 min après la fermeture de la dernière session.

`APP_ACTIVITY_MAX_AGE_S = 3 × APP_HEARTBEAT_S`, où `APP_HEARTBEAT_S` (60) est **égal** à la période
d'émission de la plateforme (`BUSY_PERIOD_MS`) : les deux constantes se citent mutuellement en
commentaire.

Si le fichier est absent ou illisible, la condition 3
est **fausse** (absence de signal = pas d'activité applicative), et la passe journalise la cause.

### 3.3 Plateforme — dépôt `desk`

- **`orchestration/host-orchestrator.ts`** (nouveau) : implémentation d'`Orchestrateur` pour `demarrer()`. Il
  parle au socket via `node:net` (aucune dépendance). Délai court, erreurs typées en `Resultat`
  (jamais d'exception qui remonte au routage, jamais de `catch` muet).
  `arreter`, `instantane`, `attribuer` gardent le comportement actuel.
- **`EtatVm` gagne `demarrage`.** L'union s'élargit ; les `Record<Motif, …>` exhaustifs cassent
  à la compilation, ce qui est voulu (voir l'en-tête d'`interface.ts`). `demarrage` est émis
  quand un `wake` a été accepté récemment et que l'agent n'a pas encore battu. L'horodatage du
  dernier `wake` est tenu en mémoire par le processus, avec une durée maximale nommée au-delà de
  laquelle l'état redevient `injoignable` (une VM qui ne démarre jamais ne reste pas « en
  démarrage » indéfiniment).
- **`http/routes-applications.ts`** (le déclencheur) : quand `registre.lancer` rend
  `agent-injoignable`, la route appelle `orchestrateur.demarrer(application.vm_id)` — après la garde
  d'appartenance déjà en place, donc un utilisateur ne réveille que **sa** VM — et répond `503
  {refus:'agent-injoignable', etat:'demarrage'}` si le réveil est accepté ; sinon
  `{refus:'agent-injoignable', etat:'injoignable', reveil: <motif>}`. `registre.lancer` n'envoie
  aucun ordre quand l'agent est injoignable : **rejouer le lancement est donc sans risque de
  doublon** (ce n'est pas le cas du `504 delai`, qui n'est jamais rejoué).
- **`http/routes-session.ts` et `routes-vm.ts`** : utilisent le **même** orchestrateur d'hôte
  (construit une fois dans `serveur.ts`, et non plus à chaque requête), si bien que `etat` y rend
  `demarrage` pendant un réveil ; `redemarrage.possible` y vaut `true` quand le canal est configuré.
- **`signaling/app-activity.ts`** (nouveau) : `ObservateurDeSession` qui tient l'ensemble des
  **sessions de fenêtre** appariées. 🔴 **Seules les sessions dont le nom (après le préfixe) est
  `w-<N>` comptent** (`agents/prefixe.ts::decouper`) : la session de contrôle `bureau` et le pont
  `fichiers` sont appariés dès que le hub est ouvert sur une VM allumée, et les compter ferait
  qu'un onglet de hub oublié empêcherait indéfiniment l'arrêt (relevé du 30 septembre 2026,
  `client/src/bureau/porteur-dom.ts`, `client/src/fichiers/canal.ts`). Un ensemble de noms et non
  un compteur : un `apparie` répété ou un `separe` inconnu ne peut pas fausser le compte. Tant que
  l'ensemble n'est pas vide, envoie `busy` au socket toutes les `BUSY_PERIOD_MS` = 60 s (et
  immédiatement à la première session) ; un échec d'envoi est **journalisé en `warn`** et retenté
  au tick suivant. Composé avec `trace.ts` (le relais n'accepte qu'un seul observateur : un
  composeur léger les enchaîne, sans que l'un dépende de l'autre).
- **Configuration** (`config.ts`) : `PLATEFORME_SOCKET_VM` (chemin du socket ; vide = canal
  désactivé, et `demarrer()` refuse alors avec `non-supporte`, comportement actuel). Documenté
  dans `docs/claude/variables-environnement.md`. Le gabarit `desk.env` (`hooks/`) le pose.

### 3.4 Client — `client/src`

- Le hub (`hub/page.ts` + `hub/catalogue.ts::lancerApplication`) traite `503` avec
  `etat: 'demarrage'` comme une attente : message « La VM démarre… », puis **nouveau lancement**
  toutes les 3 s jusqu'à un lancement abouti, avec une borne nommée (180 s, `MAX_WAIT_SECONDS` de
  `handle-vm-start.sh`) après laquelle un message d'échec est affiché. Le lancement n'est rejoué
  que sur ce cas précis, jamais sur `504 delai`.
- 🔴 **Limite connue, à ne pas cacher : le client n'a aucun mécanisme de traduction.** Ses libellés
  sont des constantes françaises (`presse-papier.ts::MESSAGE_ECHEC`, `bureau/porteur-dom.ts::TEXTE_SUIVEUR`).
  Le message « La VM démarre… » suivra cette convention existante, donc **français seulement** ;
  introduire un système d'internationalisation pour un seul message serait hors de proportion, et
  le construire pour tout le client est un chantier à part (à décider par le propriétaire).
  Pour ne pas dépendre de la langue sur le chemin critique, le client ne **déduit** rien du texte :
  il lit `etat` (valeur technique), jamais un message.

## 4. Gestion des erreurs

| Situation | Comportement |
| --- | --- |
| Socket absent (canal non installé) | `demarrer()` → refus `non-supporte`, journalisé `warn` ; hub : message d'indisponibilité, pas d'attente infinie |
| Socket présent, droits refusés | refus `hote-inaccessible`, journalisé `warn` avec la cause système |
| `wake` renvoie `err` | refus typé avec la cause ; pas de retry masqué |
| VM ne devient jamais `prete` | après le délai maximal, `injoignable` + message d'échec côté client |
| `busy` échoue | `warn` + retenté au tick suivant ; la VM peut s'arrêter si l'échec dure plus de 30 min — c'est journalisé, pas silencieux |
| Libvirtd mort | déjà géré par `handle-vm-start.sh` (incident du 24 août 2026) : `err`, remonté tel quel |

## 5. Tests

- **`installer`** : le test de fichiers posés (`test_console_install.py`) et d'armement
  (`test_console_activate.py`) couvrent les nouvelles unités ; test shell avec faux `virsh`, dans le style de `tests/test_handle_vm_start.sh` :
  verbes `wake` et `busy`, verbe inconnu, ligne vide, ligne trop longue ; `vm-idle-shutdown.sh` :
  `app-activity` récent → pas de strike, ancien/absent → strike, CPU et flux toujours prioritaires.
- **`desk`** (`vitest`) : orchestrateur d'hôte contre un vrai socket Unix de test (succès, refus,
  socket absent, délai) ; route de session (`demarrage`, refus, `prete`) ; observateur d'activité
  (comptage, jamais négatif, `busy` périodique avec horloge injectée, arrêt à 0) ; exhaustivité des
  `Record<Motif, …>` après l'ajout de `demarrage`.
- **Recette réelle** (VM `Windows`) : éteinte → lancement d'une app depuis le hub → VM démarrée et
  session ouverte ; fermeture → 30 min → hibernation ; les mesures vont au journal du chantier.

## 6. Hors périmètre

- Arrêt ou instantané depuis le hub (`arreter`, `instantane` restent `non-supporte`).
- Lecture des flux WebRTC dans conntrack : leurs ports ne sont pas ceux que le script surveille ;
  le signal applicatif `busy` remplace cette lecture.
- Réveil depuis une autre source que le lancement d'une app (par exemple à la connexion au hub ou
  à `/session`).
- L'internationalisation du client (voir §3.4) : le message d'attente reste en français.

## 7. Découpage prévu (pour le plan)

1. `installer` : socket + service + verbes + tests shell.
2. `installer` : condition 3 du script d'arrêt + tests.
3. `desk` : `EtatVm.demarrage`, orchestrateur d'hôte, config.
4. `desk` : route de session.
5. `desk` : observateur d'activité + composeur.
6. `desk` : client (attente et libellés).
7. `desk` : gabarit `desk.env`, unité systemd (`SupplementaryGroups`), documentation.
8. Recette réelle et journal.

Les tâches 1–2 (dépôt `installer`) et 3–7 (dépôt `desk`) sont indépendantes jusqu'à la recette.
