# `desk` devient un package Nivuus — résultats

**29 août 2026.** Chantier `package-nivuus`, tâches 1 à 9. Commits `2cc9949`
(exclu, c'est le plan) à `ed0e53f` inclus — dix-sept commits. Spec :
`docs/superpowers/specs/2026-08-29-package-nivuus-design.md`. Journal de
bord complet, avec chaque ronde de revue et chaque ruling du contrôleur :
`.superpowers/sdd/2026-08-29-package-nivuus/progress.md`.

Ce document est la revue **transverse** de fin de lot : ce qu'aucune revue
par tâche ne pouvait voir, parce que chaque tâche était correcte prise
séparément (huit tâches, huit revues, une ronde de correction chacune sauf
la tâche 2 et la tâche 7, revues propres du premier coup).

---

## 1. Ce que le lot livre

Le trou qu'il ferme, nommé dans la spec : `console/guest/payload.py`
déclarait l'agent Windows « extracted before the wipe » et
`fetch_payload.py` écrivait « Not fetched, and never fetchable » — la VM
cible était devenue une **appliance** provisionnée par `packages/installer`
(retrait délibéré de `C:\dev`, de Rust et du montage CIFS), et plus personne
ne savait refabriquer le binaire autour duquel elle se reconstruit.

| Tâche | Ce qu'elle ajoute | Commits |
| --- | --- | --- |
| 1 | `scripts/build-agent-croise.sh` — compile l'agent en croisé (`x86_64-pc-windows-gnu`), sans VM ni CIFS, et le dépose à une destination donnée | `1dc6117`, `b63ea1e` |
| 2 | `nivuus-package.yaml` (tier userspace, `requires.packages: [console]`) et `wizard.yaml` (quatre questions) | `9be0d58` |
| 3 | `hooks/resolve.py` — refuse AVANT que le disque soit touché (VM injoignable, `auth_mode` inconnu ou `pomerium` sans garde de confiance possible) ; émet les faits (`vm_repond`, `node_version`, `turn_ecoute`, `turn_relais`, `port`) | `fa5dc03`, `175ae64`, `e1d07a7` |
| 4 | `hooks/install.py` — pose `desk.env` (600), `/opt/nivuus/desk/{plateforme,client/dist}`, l'unité systemd (posée, pas armée), `turnserver.conf` (posé, pas armé) | `b6a0c92`, `92cacfb` |
| 5 | `hooks/activate.py` — arme l'unité par un LIEN, crée le compte admin et enrôle l'agent (mot de passe et secret jamais sur l'argv), idempotent | `911c910`, `cc1da83` |
| 6 | `hooks/vm.py` — pose ProjFS et VB-Audio dans la VM par le chemin WinRM de `console` ; `vb_audio: true` refusé dans `resolve` avant tout octet écrit | `a7cffb4`, `ca03468` |
| 7 | dépose `agent.exe` là où `console` va le chercher (`<NIVUUS_PACKAGES_DIR>/console/guest/payload/agent/agent.exe`) ; deux extractions préalables (`administration.py`, `env_fichier.py`) pour rester sous 500 lignes | `c774d02`, `b1d27da`, `4fa540c` |
| 8 | `Makefile` (`make test`, découverte par préfixe), `README-package.md` (packaging ≠ produit) | `f1bbaf8`, `ed0e53f` |

État final, rejoué le 29 août 2026 : `make test` — 7 suites, exit 0.
`env -u TURN_URL -u TURN_SECRET ./scripts/verify-all.sh` — **10/10 étapes
vertes**, 65 fichiers de test plateforme, **717 tests**, tous passés
(rejoué par moi indépendamment pour cette revue, même compte exact que le
commit `ed0e53f` l'annonce).

---

## 2. Relecture de chaque message de commit contre son diff

**Dix-sept commits relus, un par un**, `git show --stat` et le diff complet
en regard du message. Méthode : pour chaque affirmation vérifiable du
message (nombre de lignes, chemin de fichier, numéro de ligne cité dans un
dépôt voisin, comportement d'une fonction), la commande qui l'établit a été
rejouée — jamais recopiée d'un rapport de tâche.

**Vérifications indépendantes effectuées** (au-delà du simple diff) :

- `1dc6117`, `9be0d58`, `fa5dc03`, `175ae64`, `e1d07a7`, `b6a0c92`, `92cacfb`,
  `911c910`, `cc1da83`, `a7cffb4`, `ca03468`, `c774d02`, `b1d27da`, `4fa540c`,
  `f1bbaf8`, `ed0e53f` : stat + message relus. `b63ea1e` idem.
- `c774d02`/`b1d27da`/`4fa540c` : les transitions de taille annoncées pour
  `hooks/activate.py` (479→420, 420→374, 374→453) **rejouées par
  `git show <commit>^:hooks/activate.py | wc -l` et `git show
  <commit>:hooks/activate.py | wc -l`** — les six nombres correspondent
  exactement.
- `fa5dc03` : la citation de `plateforme/src/config.ts::lireConfig` («
  refuse de démarrer sans `PLATEFORME_PROXY_DE_CONFIANCE` » en mode
  `pomerium`) et la borne `node_version` « RELUE dans
  `plateforme/package.json::engines.node` » — les deux confirmées dans les
  fichiers cités (`config.ts:302-308`, `package.json:6-8`,
  `">=24.0.0 <25.0.0"`).
- `4fa540c` : les trois citations de fichiers du dépôt voisin `installer`
  (`fetch_payload.py:61` → `PACKAGED_AGENT_EXE`, `discovery.py:22` →
  `PACKAGES_DIR = os.environ.get("NIVUUS_PACKAGES_DIR", …)`,
  `activate_cli.py:107-108` → `run_activate(match[0], detected,
  state[name].get("answers") or {}, …)`) — **les trois lues au numéro de
  ligne exact cité**, dans `installer/console/guest/fetch_payload.py` et
  `installer/installer/packages/{discovery,activate_cli}.py`.
- `a7cffb4` : la citation `guest-ready-watch.py:113` (`WINRM_EXEC =
  "/opt/nivuus-packages/console/guest/winrm_exec.py"`) — confirmée au
  numéro de ligne exact.
- `b6a0c92`/`92cacfb` : la garantie `run_install(manifest, hw, answers,
  root)` **sans** `facts`, et `run_activate(…, facts=facts)` qui les
  fusionne via `merge_into_hw` — confirmée en lisant
  `installer/installer/packages/runner.py:315-338` intégralement.
- `92cacfb` : `DynamicUser=yes` / `StateDirectory=nivuus-desk` /
  `PLATEFORME_ICONES`+`PLATEFORME_TELEVERSEMENTS` vers
  `/var/lib/nivuus-desk/{icones,televersements}` — confirmés dans
  `hooks/assets/desk-plateforme.service` et `hooks/install.py:265-266`.
  `os.open(…, 0o600)` atomique pour `desk.env` et `turnserver.conf` —
  confirmé, deux occurrences, `hooks/install.py:129,189`.
- `ca03468`/`ed0e53f` : la doctrine « refuse dans `resolve`, avant tout
  octet écrit » pour `vb_audio: true` — confirmée
  (`hooks/resolve.py:221-241,310-312`).
- **`ed0e53f` : l'empreinte `fc66b5649cf2782e` recalculée indépendamment**,
  en rejouant l'algorithme exact du détecteur (regex d'affectation, extraction
  de valeur par `valeurApres`, SHA-256 tronqué à 16 hex) sur la ligne 59 de
  `tests/desk_activate_fixtures.py` telle qu'elle vit sur le disque — **même
  résultat, à l'octet près** (script Node ad hoc, non versionné). L'annonce
  finale du commit (« 65 fichiers, 717 tests, tous passés ») a été **rejouée
  en entier** pour cette revue, pas relue seulement : même compte exact.
- `92cacfb` : l'extraction de `hooks/commun.py` (`interface_de_route_par_defaut`,
  `adresse_ipv4_de`, `PORT_DEFAUT`) — le fichier existe, 64 lignes, importé
  par les deux hooks, aucune duplication résiduelle trouvée par grep.

**Écart trouvé** : **un seul**, mineur, purement numérique.

> `b6a0c92` affirme « `hooks/install.py` (314 lignes) » dans son propre
> corps de message. Mesuré à ce commit
> (`git show b6a0c92:hooks/install.py | wc -l`) : **312 lignes**, exactement
> ce que `git show --stat` annonçait déjà (`312 +++`). Écart de 2 lignes.
> Ce même écart avait déjà été repéré par le contrôleur pendant la ronde de
> revue de la tâche 4, mais **attribué au rapport de tâche** (« minor
> deferred: rapport inexact (314 l./750 annonces, 312/0755 mesures) — à
> corriger dans le rapport, pas dans le code ») — l'écart existe
> **identiquement dans le message de commit lui-même**, une pièce du dépôt
> que personne ne relit ensuite. Aucune conséquence fonctionnelle : la
> phrase suivante du plan ne s'appuie sur aucun de ces deux nombres. Nommé
> ici plutôt que corrigé — corriger un message de commit après coup en
> réécrirait l'historique, ce que ce chantier ne fait jamais.

Aucun autre écart trouvé sur les dix-sept commits. Les affirmations
comportementales non directement rejouables (par ex. « 9 échecs » de
`e1d07a7`, « 8 assertions » de `4fa540c`, les mutations ciblées « restaurées
depuis une copie nommée ») sont cohérentes avec la doctrine TDD que ce
chantier applique partout et avec les tests actuellement présents dans le
dépôt ; elles décrivent des exécutions ponctuelles d'hier, non rejouables à
l'identique (l'état du hook a changé depuis), et n'ont pas été mises en
doute faute de pouvoir les rejouer sans réintroduire le bug corrigé.

---

## 3. Tailles des fichiers, après la dernière édition de la ronde

Commande de `CLAUDE.md`, relancée à `HEAD` (`ed0e53f`) :

```
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
```

Résultat :

```
   1536 agent/src/encode.rs
    630 agent/src/windows_source.rs
```

**Aucun fichier neuf de ce lot ne franchit 500 lignes.** Les deux seules
lignes qui dépassent sont la dette déjà gelée et documentée dans le tableau
de dette de `CLAUDE.md` (`encode.rs`, `windows_source.rs`), inchangée par ce
chantier.

Tailles des fichiers produits par ce lot, à `HEAD` :

| Fichier | Lignes |
| --- | --- |
| `scripts/build-agent-croise.sh` | 48 |
| `nivuus-package.yaml` | 32 |
| `wizard.yaml` | 32 |
| `hooks/resolve.py` | 382 |
| `hooks/install.py` | 308 |
| `hooks/activate.py` | 453 |
| `hooks/vm.py` | 203 |
| `hooks/administration.py` | 86 |
| `hooks/env_fichier.py` | 59 |
| `hooks/commun.py` | 64 |
| `hooks/assets/desk-plateforme.service` | 52 |
| `tests/test_desk_build_croise.py` | 89 |
| `tests/test_desk_manifeste.py` | 64 |
| `tests/test_desk_resolve.py` | 155 |
| `tests/test_desk_install.py` | 233 |
| `tests/test_desk_activate.py` | 397 |
| `tests/test_desk_vm.py` | 253 |
| `tests/test_desk_payload.py` | 262 |
| `tests/desk_activate_fixtures.py` | 235 |
| `Makefile` | 37 |
| `README-package.md` | 89 |

Ces tailles correspondent exactement à celles que les rapports de tâche 6/7
annonçaient (`activate.py` 453, `administration.py` 86, `env_fichier.py` 59,
`test_desk_payload.py` 262, `desk_activate_fixtures.py` 235).

---

## 4. Ce que ce lot n'établit PAS

### 4.1 Que l'agent bâti en croisé FONCTIONNE

`scripts/build-agent-croise.sh` compile et lie `agent.exe` — la compilation
prouve que l'agent **se lie**, pas qu'il **tourne**. Le binaire produit est
un produit **mingw** (`x86_64-pc-windows-gnu`, éditeur de liens
`x86_64-w64-mingw32-gcc`), là où l'ancien binaire de production était bâti
**sur la VM, en MSVC** (chaîne `cargo build` invoquée directement dans
Windows, avec `link.exe` de Visual Studio). **Ce binaire n'a jamais tourné
sur la VM Windows.** Le script lui-même le dit dans son propre en-tête
(`scripts/build-agent-croise.sh:9-11`), la tâche 7 le répète dans son
rapport (réserve 3), et la spec (§4.3) nomme le juge qui reste dû : une
exécution réelle sur la VM. Rien dans ce lot n'a changé cet état.

### 4.2 Que l'installation a été jouée de bout en bout, sur une machine neuve

Elle ne l'a pas été. Les rouges de la tâche 4 (« démarrer le service ») et
de la tâche 5 (l'armement par lien, l'idempotence) se jouent **sous une
racine temporaire** (`--root=/tmp/…` ou équivalent, `TMPDIR=/var/tmp` sur
cette machine) et sur des **noms d'unité factices** — jamais sur
`/etc/systemd/system` réel, jamais avec `systemctl daemon-reload` ni
`systemctl start` sur le système hôte. C'est une décision du chantier
(ruling du contrôleur, `progress.md`, avant la tâche 4) : le système de
l'hôte ne devait se toucher qu'à un **lot 10**, séparé, pas couvert par ce
plan à neuf tâches. Conséquence directe, nommée par les deux rapports de
tâche concernés : le chemin `--root=/` (armement réel de l'unité,
`daemon-reload`, démarrage réel du service `npm start`) **n'a aucune
couverture automatisée**, par construction. Personne n'a encore observé le
package s'installer, de `resolve` à un service qui répond sur le port 3445,
sur une machine où rien n'existait avant.

### 4.3 Qu'aucun des douze items du lot 3 n'est mesuré

Le lot 3 (la campagne de mesure sur la VM — voir la spec, §1 et §2) est
**suspendu**, pas retiré : il a buté sur une VM qui refusait les
identifiants du dépôt, et ce chantier existe précisément pour lever ce
blocage (produire et déposer `agent.exe`, adapter `desk` à une VM devenue
appliance). Il **rend** la VM atteignable ; il ne **mesure** rien lui-même.
Les douze items de ce lot 3 restent à zéro mesure après ce chantier — la
spec et le plan qui les décrivent restent valides pour le jour où la VM
sera équipée pour les recevoir.

### 4.4 L'ordre d'activation : `console` avant `desk`

`desk` déclare `requires.packages: [console]` (`nivuus-package.yaml`), et
le moteur trie les packages sélectionnés par dépendance avant de les
appliquer (tri de Kahn, `installer/installer/packages/dependencies.py:59-…`,
vérifié en lisant le code) : `console` s'active donc **avant** `desk`, dans
le sens de la dépendance. Conséquence directement nommée par la tâche 7
(réserve 2, reprise ici sur décision du contrôleur) : le dépôt d'`agent.exe`
que la tâche 7 met en place **ne bénéficie qu'aux reconstructions futures**
de la VM (un opérateur qui relance `fetch_payload.py` après coup) — **jamais
au tout premier provisionnement**, qui consomme la source vendorisée déjà
committée dans `console` au moment où l'image d'installation a été bâtie.
Résoudre cet ordre (faire porter le dépôt par `console` lui-même, ou changer
l'ordre de dépendance) est une décision de conception hors du périmètre de
ce plan.

### 4.5 VB-Audio : la question existe, la charge n'existe pas

Le wizard pose la question `vb_audio` (défaut désarmé, licence VB-Audio
personnelle seulement). Depuis la ronde de correction de la tâche 6,
`hooks/resolve.py::valider_vb_audio` **refuse** `vb_audio: true` — avant
qu'un octet touche le disque — en nommant la vraie raison (aucune charge
livrable, licence personnelle). La levée de `hooks/vm.py::poser_vb_audio`
reste en place en défense en profondeur, mais elle n'est plus le premier
rempart. La question elle-même n'a **pas** été retirée du wizard : elle
documente une intention réelle de produit, et son défaut (`false`) ne bloque
rien. **C'est une dette assumée**, nommée dans le ruling du contrôleur
(`progress.md`) et vérifiée ici dans le code (`hooks/resolve.py:221-241`) —
pas un oubli, et pas quelque chose que ce lot corrige plus avant : il faudrait
une charge VB-Audio livrable pour que la question cesse d'être un refus
garanti.

### 4.6 Node sur la cible

`hooks/assets/desk-plateforme.service::ExecStart=/usr/bin/npm start`
suppose un `npm` **à l'échelle du système** (paquet Debian ou NodeSource).
**Aucune tâche de ce plan ne provisionne Node.js ni `plateforme/node_modules`
sur la machine cible.** 🔴 **Vérifié dans sa forme la plus dure sur cette
machine de développement** (`ls /usr/bin/node /usr/bin/npm
/usr/local/bin/node /usr/local/bin/npm`, `dpkg -l | grep nodejs`, `type node
npm`) : **il n'existe AUCUN Node à l'échelle du système, ni pour root ni
pour personne d'autre** — pas seulement « sous `/root/.nvm`, en mode 700,
inatteignable à un non-root » (la première formulation de la réserve 2 de
la tâche 4). `node` et `npm` ne sont que des **fonctions shell** qui
sourcent `nvm` depuis `$HOME/.nvm` ; `/usr/bin/npm` n'existe **pour
personne**, `DynamicUser=yes` ou non. Le service, tel quel, ne démarrerait
donc pas sur cette machine si son unité était armée pour de vrai : c'est
nommé dans le code lui-même (`hooks/assets/desk-plateforme.service`,
commentaire au-dessus de `ExecStart`) et dans le rapport de la tâche 4
(réserve 2). Aucune tâche de ce plan ne referme ce trou.

### 4.7 coturn : posée, jamais armée

`hooks/install.py::ecrire_turnserver_conf` écrit `/etc/turnserver.conf`,
mais **ne touche jamais** `/etc/default/coturn` (`TURNSERVER_ENABLED`) : le
fichier de configuration existe, le service ne démarre pas pour autant. Le
contenu de ce fichier est de surcroît **extrapolé** depuis les options déjà
vérifiées de `docker-compose.coturn.yml` (`--listening-ip`, `--relay-ip`,
`--static-auth-secret`, etc.), **jamais vérifié contre un vrai binaire
`coturn`** : `dpkg -l coturn` ne rend rien sur la machine où ce lot a été
écrit. Le code le dit lui-même, au mot près : « POSÉE, PAS ARMÉE » et
« FORMAT NON VÉRIFIÉ SUR CETTE MACHINE » (`hooks/install.py:149-166`).
`nivuus-package.yaml` déclare `apt: [coturn]`, donc le paquet Debian sera
installé par le moteur — mais installer le paquet, poser sa configuration et
armer son service sont trois gestes distincts, et seuls les deux premiers
sont couverts.

---

## 5. Défaut transverse trouvé par cette revue — nommé, non corrigé

**🔴 Rien, dans ce lot ni dans le dépôt voisin `console` (tel qu'observé),
ne dépose `AGENT_VM`/`AGENT_SECRET` dans l'environnement du processus
`agent.exe` qui tourne réellement dans la VM Windows.**

C'est le genre de défaut qu'aucune revue par tâche ne pouvait voir, parce
que les deux moitiés sont chacune correctes prises séparément :

- **Côté `desk`** (tâche 5, `hooks/activate.py:444`) : une fois le compte
  admin créé et l'agent enrôlé (`npm run admin:agent`), le hook écrit
  `AGENT_VM` et `AGENT_SECRET` dans **`desk.env`** — le fichier
  d'environnement du **service `plateforme` sur l'hôte Linux**. C'est
  correct et suffisant pour que la plateforme **reconnaisse** un agent qui
  se présenterait avec ce couple.
- **Côté VM Windows** (dépôt voisin `console`, hors périmètre de ce lot,
  observé par la reconnaissance en lecture seule du 29 août 2026,
  `.superpowers/sdd/2026-08-29-package-nivuus/recon-appliance.md`, Q2) : le
  lanceur que `console` dépose et arme dans la VM
  (`console/guest/provision/assets/run-agent.ps1`, copié par
  `console/guest/provision/40-agent.ps1`) ne pose **que trois** variables :
  `SIGNALING_URL`, `LOCAL_IP`, `RUST_LOG`. **Ni `AGENT_VM` ni
  `AGENT_SECRET`.**
- **Côté agent Rust** (`agent/src/plateforme/identite.rs:44-49`) : sans
  `AGENT_JETON` hérité ET sans le couple `AGENT_VM`/`AGENT_SECRET`, la
  source d'identité résout à `SourceIdentite::Aucune`, dont le commentaire
  du code dit, au mot près : *« Ni l'un ni l'autre : aucun jeton, donc
  aucune session ne s'établira. Ce n'est pas un mode de repli, c'est une
  panne annoncée. »*

**Conséquence** : même si `resolve`/`install`/`activate` réussissent
intégralement sur l'hôte Linux (ce que ce lot établit), et même si la VM
Windows est une appliance saine avec `agent.exe` à jour (ce que la tâche 7
établit), **l'agent qui démarre dans la VM ne présentera jamais** le secret
que `desk` vient de créer pour lui — parce que rien ne le lui a transmis. Le
compte admin existe, la ligne `vm` existe dans la base de la plateforme,
mais **aucune session WebRTC ne s'établira jamais** sans une intervention
manuelle qui n'est décrite nulle part dans ce lot (ni dans `console`, pour
autant que cette reconnaissance en lecture seule ait pu l'établir).

**Ce que j'ai vérifié, pas seulement supposé** :
`grep -rn "AGENT_VM\|AGENT_SECRET" hooks/` ne montre que l'écriture dans
`desk.env` (`hooks/activate.py:444`) et sa documentation — aucune fonction
de `hooks/vm.py` (qui, lui, pose ProjFS et VB-Audio dans la VM par le
même chemin WinRM) ne s'occupe de ce couple. Aucun rapport de tâche (1 à 8)
ne nomme ce trou. ⚠️ **Corrigé après une première rédaction de ce
paragraphe** : le contrôleur a lui-même consigné le même constat dans
`progress.md` (« FAIT B », section « Deux faits établis par moi avant le
lot 10 »), en parallèle de cette revue et sans connaissance de son
avancement — trouvaille indépendante, pas reprise l'une de l'autre,
convergente sur les mêmes trois fichiers (`hooks/activate.py:444`,
`console/guest/provision/assets/run-agent.ps1`,
`agent/src/plateforme/identite.rs` — le contrôleur cite en outre
`configuration.rs:200-201` et `main.rs:270`, lus et confirmés ici aussi).

**Je ne l'ai pas corrigé** (règle du plan pour cette tâche 9 : aucune
modification de code). C'est, à mon jugement, le défaut le plus important
que cette revue transverse ait trouvé — plus important qu'aucune des
mineures différées ci-dessous — parce qu'il touche directement la mission
du propriétaire pour la suite (« rendre le service utilisable ») : sans un
mécanisme qui pousse `AGENT_VM`/`AGENT_SECRET` (ou un jeton équivalent) dans
l'environnement du processus superviseur de la VM, aucune installation de ce
package, aussi correcte soit-elle par ailleurs, ne peut aboutir à une session
qui fonctionne. Le remède le plus direct serait sans doute que `hooks/vm.py`
(tâche 6), qui possède déjà le chemin WinRM vers la VM, y dépose aussi ces
deux variables une fois l'enrôlement réussi — mais choisir entre ce remède,
un autre porté par `console`, ou un mécanisme distinct, est une décision de
conception qui appartient au propriétaire du dépôt.

---

## 6. Tri des mineures différées

Seize mineures ont été différées au fil des huit tâches (`progress.md`).
Pour chacune : correction avant fusion, ou reste due — et pourquoi. Aucune
n'a été corrigée par cette tâche 9 (règle : ne pas corriger par réflexe).

| # | Mineure (tâche d'origine) | Verdict | Pourquoi |
| --- | --- | --- | --- |
| 1 | `rustup target list \| grep` sous `2>/dev/null` attribuerait à « cible absente » une panne de `rustup` lui-même (T1) | **Reste due** | Cosmétique : si `rustup` lui-même est cassé, l'étape suivante (`cargo build`) échouera de toute façon avec un message qui le révèle. Aucun chemin ne rend un succès faux. |
| 2 | `mkdir -p` rend un message générique si la destination existe et n'est pas un répertoire (T1) | **Reste due** | Cas limite qui suppose un appelant se trompant sur son propre argument `<destination>` ; `mkdir -p` échoue déjà (code non nul), seul le message est moins parlant. Coût de correction disproportionné à la fréquence du cas. |
| 3 | `auth_mode` porte `required: true` ET `default: motdepasse` ; `validate_answers` lève avant de consulter le défaut (T2) | **Reste due** | Hérité du plan verbatim, pas un écart de l'implémenteur ; le défaut ne sert qu'au pré-remplissage du portail, jamais à un repli silencieux — comportement voulu ailleurs dans ce dépôt (« un défaut de mode est une ouverture, pas un confort »). |
| 4 | Aucun test versionné n'exerce le garde **générique** de `resolve.py` sur un cas vraiment imprévu — seul un `RecursionError` joué à la main par le relecteur l'a fait rougir (T3) | **DEVRAIT être corrigée avant fusion** | C'est l'invariant CENTRAL de la tâche 3 (« refuse, ne lève jamais »), et ce dépôt a déjà érigé en règle transverse « un contrôle qu'on n'a jamais vu rouge n'est pas un contrôle » — appliquée à ce MÊME chantier pour le garde éditeur-de-liens de la tâche 1 (élevé en Importante pour la même raison, `b63ea1e`). Peu coûteux : un test qui pousse `hw` ou `answers` en profondeur de récursion suffirait. Vérifié à cette revue : `grep -n "RecursionError" tests/test_desk_resolve.py` ne rend toujours rien. |
| 5 | Le test ne vérifie pas la forme complète de `facts` (T3) | **Reste due** | Repris verbatim du plan, pas un écart de l'implémenteur ; les champs individuels sont vérifiés ailleurs dans la suite. |
| 6 | Le contrôle de version Node interroge la machine qui exécute le hook, jamais une cible pas encore installée (T3) | **Reste due, et n'est probablement pas un défaut** | Réserve honnête plutôt qu'un trou : `resolve` s'exécute réellement SUR la machine cible (c'est le contrat du moteur), donc interroger « la machine qui exécute le hook » EST interroger la cible. |
| 7 | Pas de garde défensif sur `facts["turn_ecoute"]` (T4) | **Reste due** | `facts` a une forme fixe, produite par `resolve.py` du même package et jamais par un tiers ; un accès direct est cohérent avec le reste du hook, qui ne défend pas non plus contre une forme de `facts` inventée. |
| 8 | `sous()` avalerait un chemin absolu (`pathlib.Path.__truediv__` avec un opérande absolu écrase le préfixe) (T4) | **Reste due, vérifié inoffensif aujourd'hui** | Vérifié par cette revue (`grep -n 'sous(' hooks/install.py`) : les cinq appels passent tous un littéral relatif codé en dur (`"etc/nivuus/desk.env"`, etc.), jamais une valeur dérivée d'`answers`/`facts`. Le risque est réel EN PRINCIPE mais inatteignable EN PRATIQUE dans le code actuel. |
| 9 | Un `install` rejoué reforge le secret en silence (T4) | **Reste due, mais à surveiller** | Vérifié : `ecrire_secret()` (`secrets.token_hex(32)`) est appelé inconditionnellement à chaque exécution d'`install.py` (`hooks/install.py:234-235`), sans lire un `desk.env` préexistant. Une réinstallation/réparation regénère silencieusement `PLATEFORME_SECRET_JETON` ET `TURN_SECRET`, invalidant toute session en cours — opérationnellement surprenant, pas une faille de sécurité. `install` n'est normalement joué qu'une fois ; nommé pour le jour où quelqu'un le rejoue en réparation. |
| 10 | Rapport de tâche 4 inexact (« 314 lignes » / « 750 » annoncés contre 312/0755 mesurés) (T4) | **Non pertinent à corriger dans le code** ; **le même écart existe dans le message de commit `b6a0c92`, voir §2** | Le rapport de tâche n'est pas une pièce versionnée dans le même sens qu'un commit ; l'écart réel et actionnable est celui du message de commit, déjà nommé ci-dessus. |
| 11 | `activate.py` en 644 là où ses frères sont en 755 (T5) | **Sans objet, confirmé** | Le moteur lance `[sys.executable, hook, "--phase", phase]` (`installer/installer/packages/runner.py::_run_hook`) : le bit exécutable n'entre jamais en jeu. Vérifié à nouveau par cette revue, même lecture que le ruling du contrôleur pendant la tâche 5. |
| 12 | Rapport de tâche 5 annonce 394 lignes de test pour 379 (T5) | **Reste due, cosmétique** | Écart de rapport, pas de code ni de commit ; `git show 911c910:tests/test_desk_activate.py \| wc -l` confirme 379, cohérent avec le message de commit lui-même qui ne cite aucun total. |
| 13 | Aucune garde d'idempotence sur le dépôt d'`agent.exe` : chaque `activate` rejoué redéclenche une compilation croisée complète (~40 s) (T7) | **Reste due** | Assumé explicitement par l'implémenteur et le contrôleur : « sans corruption possible », coût de performance/UX seulement, pas de correction. Non commenté dans le code lui-même — pourrait l'être à peu de frais, mais ce n'est pas un défaut fonctionnel. |
| 14 | Le toucher de `tests/desk_activate_fixtures.py` au-delà du périmètre littéral de la tâche 7 (T7) | **Non pertinent : déjà jugé légitime par la revue de tâche 7** | Sans ce toucher, les scénarios existants auraient soit refusé (« console absent »), soit déclenché une compilation réelle de 40 s à chaque appel de test — la revue de tâche 7 l'a explicitement approuvé, avec la raison écrite dans son propre rapport. |
| 15 | Le glob de découverte de suites est `test_*.py`, pas `test_desk_*.py` (T8) | **Reste due** | Fonctionne aujourd'hui parce que toutes les suites de `tests/` portent le préfixe `desk_` implicitement (elles ne s'appellent QUE `test_desk_*.py`) ; un futur `test_foo.py` non préfixé serait pris pour une suite de ce package. Improbable tant que `tests/` reste privé à `desk`, et le `Makefile` documente déjà la règle de sélection énoncée — respecte la doctrine « énoncer la règle de sélection avant de compter ». |
| 16 | Le README ne restitue pas lui-même la distinction « 10 étapes contre 18 en-têtes » de `verify-all.sh`, il défère à `CLAUDE.md` (T8) | **Reste due, choix délibéré** | Éviter la duplication d'une même information à deux endroits qui pourraient diverger — cohérent avec la doctrine anti-« naufrage du 487 » de ce dépôt (une affirmation vérifiable ne devrait vivre qu'à un seul endroit). |

**Résumé du tri** : sur seize mineures, **une seule** (#4, le test manquant
sur le garde générique de `resolve.py`) est recommandée pour correction
avant fusion — parce qu'elle touche l'invariant central d'une tâche que ce
chantier lui-même a érigé en doctrine transverse ailleurs. Les quinze autres
restent dues, avec leur raison propre.

---

## 7. Ce que le lot 10 (mise en service réelle) devra trancher

Consolidé depuis les rulings de conception du contrôleur (`progress.md`,
« Faits de la reconnaissance… qui gouvernent le lot 10 ») — non exécuté par
ce plan, rappelé ici pour qu'il ne se perde pas :

- **Le défaut transverse du §5** (`AGENT_VM`/`AGENT_SECRET` jamais poussés
  dans la VM) devra être résolu avant qu'une session réelle ne puisse
  s'établir — c'est la découverte la plus importante de cette revue.
- Port et hôte de la plateforme : **3445** / **192.168.3.1**, dérivés de la
  route Pomerium existante (`app.allanic.me → 127.0.0.1:3445`) et du fait que
  cette adresse est joignable à la fois par Pomerium (`network_mode: host`)
  et par la VM. La route Pomerium devra être recadrée de `127.0.0.1:3445`
  vers `192.168.3.1:3445`.
- `SIGNALING_URL` de la VM devra passer de `ws://192.168.3.1:8080` (valeur
  actuelle, posée par `run-agent.ps1` de `console`) à
  `ws://192.168.3.1:3445` — **sans oublier le suffixe `/signal`** que le
  chantier `auth-pomerium` a introduit et que `run-agent.ps1` de `console`
  ne porte pas aujourd'hui (écart potentiel relevé, non confirmé en usage
  réel par la reconnaissance).
- `PLATEFORME_AUTH=motdepasse` retenu plutôt que `pomerium`, pour ne pas
  ajouter `pass_identity_headers` à une route Pomerium qui sert six autres
  hôtes.
- `coturn` (§4.7 ci-dessus) et Node sur la cible (§4.6) restent à équiper
  avant qu'un service réel puisse tourner sans intervention manuelle.

---

## 8. Réserves de cette revue elle-même

- Les affirmations comportementales ponctuelles des messages de commit
  (comptes d'échecs exacts sur un état de code révolu, ex. « 9 échecs » de
  `e1d07a7`) n'ont pas été rejouées à l'identique — l'état du hook qui les a
  produites n'existe plus tel quel. Elles ont été jugées cohérentes avec la
  doctrine TDD de ce chantier et avec la suite de tests actuelle, pas
  revérifiées bit à bit.
- Cette revue n'a pas cherché à établir la provenance exacte de
  `guest_workdir` ni le comportement réel de Pomerium sans
  `pass_identity_headers` explicite (§4.4 de `recon-appliance.md`, « Ce que
  je n'ai pas pu établir ») — hors périmètre de la tâche 9, déjà nommé par
  la reconnaissance elle-même.
- Le défaut transverse du §5 a été établi par lecture de code des trois
  dépôts concernés (`desk`, `console` via `installer/`, et `agent/src`),
  jamais par une exécution réelle sur la VM (qui n'a pas été touchée par
  cette revue, conformément à son périmètre en lecture seule).
