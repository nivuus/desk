# Lot 3 — la campagne sur la VM Windows : plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Que les douze items qui n'étaient atteignables que sur la VM Windows cessent d'être des affirmations et deviennent des chiffres datés, chacun obtenu par un montage dont la rouge a été vue rouge.

**Architecture:** Une tâche de harnais d'abord — la VM s'éteint seule, et douze items vont s'appuyer sur elle. Puis les douze items, ordonnés par ce que leur échec coûte : le pont d'abord (une sauvegarde peut s'y perdre en silence), l'infrastructure ensuite, le média, les apps enfin. Chaque item est une tâche indépendante : son montage, son critère, sa rouge, son relevé, son document. Une revue transverse ferme le lot — elle seule voit les défauts qui franchissent une frontière de tâche.

**Tech Stack:** Bash et Node (pilotes de recette, CDP sur Chrome), PowerShell via WinRM (côté VM), Rust (agent, si un remède s'avère dû), `virsh`/libvirt.

**Spec:** [`docs/superpowers/specs/2026-08-28-lot3-campagne-vm-design.md`](../specs/2026-08-28-lot3-campagne-vm-design.md)
**Cadrage :** [`docs/superpowers/specs/2026-08-21-finalisation-cadrage.md`](../specs/2026-08-21-finalisation-cadrage.md)

## Global Constraints

- **Français partout** : symboles, commentaires, messages d'erreur, messages de commit. Les identifiants techniques gardent leur forme.
- **Branche `campagne-vm`**, jamais de travail direct sur `main`.
- **500 lignes maximum** par fichier de code source — les instruments de `docs/` en sont exemptés, le code de `agent/`, `client/`, `plateforme/`, `proto/` et `scripts/` ne l'est pas. 🔴 **Relever par `wc -l`, JAMAIS recopier.**
- 🔴 **Un contrôle qu'on n'a jamais vu rouge n'est pas un contrôle.** Chaque critère se joue **et** se voit rouge. **Une rouge restée verte se DIAGNOSTIQUE, elle ne se classe pas.**
- 🔴 **Deux exécutions par bras**, minimum. **Tout zéro exige un témoin négatif** relevé dans le même journal.
- 🔴 **Les journaux bruts sont VERSIONNÉS**, sous `docs/superpowers/plans/journaux-lot3-<item>/`. Une preuve ne vit jamais dans un rapport gitignoré.
- 🔴 **Ne jamais fabriquer une pièce** : ne jamais réutiliser la sortie d'une commande pour répondre à la question d'une AUTRE sans la relancer.
- 🔴 **Toute variable neuve part dans une TÂCHE DÉDIÉE**, et le contrôle qui vaut est de lire sa ligne dans le `run-agent.ps1` **GÉNÉRÉ sur la VM**, jamais de tracer le code.
- 🔴 **Les instruments existants se RÉUTILISENT par lecture de leur fichier d'origine, jamais par copie** — « une copie éprouverait la copie, pas l'instrument » (F3).
- Commencer chaque commande par `unset -f chpwd 2>/dev/null;` — le shell hôte injecte un `ls` dans toute sortie dès qu'un `cd` court dans un sous-shell.
- `set -a && source .env && set +a` avant tout travail VM. ⚠️ `scripts/build-agent.sh` sans `.env` sourcé **s'arrête EN SILENCE**, et le symptôme se lit comme une compilation réussie.
- `git status --porcelain agent/ proto/` **avant CHAQUE build** : `build-agent.sh` rsynchronise l'arbre entier.
- 🔴 `cargo clean --release -p proto -p agent` — **les DEUX crates** — avant toute compilation qui touche `proto/`.
- 🔴 **Tuer par PID relevé**, jamais par motif : `pkill -f <motif>` depuis un shell dont la ligne de commande porte le motif tue le shell.
- **ZSH** : toujours `${var}`, jamais `"$var:suffixe"` ; des arguments littéraux, jamais une liste dans une variable.
- Message de commit long **par un fichier** (`git commit -F`), jamais `-m` avec des accents graves ou des backticks.
- ⚠️ **Un `.ps1` sans BOM portant un seul caractère non-ASCII ne s'analyse pas.** Contrôle : `LC_ALL=C grep -c '[^ -~]' fichier.ps1` doit rendre **0**.
- ⚠️ **Toute mesure de plus de 5 minutes** exige `--disable-background-timer-throttling`, `--disable-backgrounding-occluded-windows` et `--disable-renderer-backgrounding` sur Chrome.
- ⚠️ **Aucun `execFileSync` dans la boucle d'événements d'un pilote** (sauf le lancement de l'agent), **aucune capture d'écran CDP pendant une mesure**, **toute évaluation CDP bornée**.

---

### Task 0: Le harnais de campagne

**Files:**
- Create: `docs/superpowers/plans/journaux-lot3/instrument/harnais.sh`
- Create: `docs/superpowers/plans/journaux-lot3/instrument/etat-vm.sh`
- Create: `docs/superpowers/plans/journaux-lot3/README.md`

**Interfaces:**
- Produces: `harnais.sh` expose trois fonctions sourçables — `vm_prete` (démarre si besoin, attend WinRM puis un accès RÉEL à `/media/vm`, rend 0 ou 1), `agent_absent` (échoue si un `agent.exe` tourne encore), `purger_orphelins` (lance `MULTIFENETRE_VDD_PURGE=1` et rend le compte purgé). `etat-vm.sh` imprime une ligne `etat=<en-cours|ferme> extinctions=<n> horodatage=<iso>`.
- Consumes: rien.

- [ ] **Step 1: Écrire `etat-vm.sh`, le relevé d'état, seul et sans jugement**

```bash
#!/usr/bin/env bash
# Imprime l'état de la VM et le compteur d'extinctions, sur UNE ligne.
#
# 🔴 CE SCRIPT NE JUGE RIEN. Il rend l'état ; c'est l'item qui juge. Un
# harnais qui classerait lui-même une séquence « valide » masquerait la
# distinction entre « la mesure a été faite » et « la VM a tenu ».
set -uo pipefail
unset -f chpwd 2>/dev/null || true

etat=$(virsh list --all 2>/dev/null | awk '$2=="Windows" {print $3, $4}' | tr -d ' ')
[ -z "${etat}" ] && etat="inconnu"
# Le compteur d'extinctions : les arrêts du domaine tracés par libvirt.
extinctions=$(grep -c "terminating on signal\|shutting down" \
    /var/log/libvirt/qemu/Windows.log 2>/dev/null || echo 0)
echo "etat=${etat} extinctions=${extinctions} horodatage=$(date -Is)"
```

- [ ] **Step 2: Le voir rendre les DEUX valeurs**

Run: `bash docs/superpowers/plans/journaux-lot3/instrument/etat-vm.sh` — VM allumée.
Expected: `etat=encoursd'exécution extinctions=<n> horodatage=…`

Puis `virsh shutdown Windows`, attendre l'arrêt, relancer.
Expected: `etat=fermé …`

🔴 **Sans ce second relevé, le script serait structurellement incapable de rendre l'autre valeur** — c'est le patron que ce dépôt punit. Redémarrer la VM ensuite.

- [ ] **Step 3: Écrire `harnais.sh`**

```bash
#!/usr/bin/env bash
# Le point unique de la campagne du lot 3. À SOURCER, pas à exécuter.
#
#     source docs/superpowers/plans/journaux-lot3/instrument/harnais.sh
#
# 🔴 POURQUOI IL EXISTE : la VM s'éteint seule par DEUX mécanismes distincts
# — une hibernation initiée DANS l'invité (Kernel-Power 187/42), et l'HÔTE
# qui tue QEMU (libvirtd --timeout 120 s'arrête sur inactivité et emporte le
# domaine). Douze items s'appuient sur cette VM pendant des séquences longues.
unset -f chpwd 2>/dev/null || true
RACINE_HARNAIS="$(git rev-parse --show-toplevel)"

vm_prete() {
    virsh list --all | grep -q "Windows.*en cours" || virsh start Windows
    # 1. WinRM répond.
    for _ in $(seq 1 60); do
        timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null && break
        sleep 5
    done
    timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null || {
        echo "🔴 WinRM ne répond pas"; return 1; }
    # 2. PUIS un ACCÈS RÉEL à /media/vm — jamais `mountpoint -q` : l'entrée
    #    CIFS persiste dans la table de montage VM éteinte et connexion morte.
    for _ in $(seq 1 60); do
        ls /media/vm/dev >/dev/null 2>&1 && break
        sleep 5
    done
    ls /media/vm/dev >/dev/null 2>&1 || { echo "🔴 /media/vm inaccessible"; return 1; }
    echo "vm prête : $(bash "${RACINE_HARNAIS}/docs/superpowers/plans/journaux-lot3/instrument/etat-vm.sh")"
}

agent_absent() {
    # 🔴 Un agent survivant tient agent.log, et l'on relit alors le journal de
    # la tentative PRÉCÉDENTE en croyant lire le sien. À appeler avant CHAQUE
    # tentative, y compris échouée.
    local restants
    restants=$(node "${RACINE_HARNAIS}/scripts/winrm.js" \
        '(Get-Process agent -ErrorAction SilentlyContinue | Measure-Object).Count' 2>/dev/null \
        | tr -dc '0-9')
    [ "${restants:-0}" = "0" ] || { echo "🔴 ${restants} agent(s) survivant(s)"; return 1; }
    echo "aucun agent survivant"
}

purger_orphelins() {
    # Une sortie virtuelle et une racine ProjFS survivent à un arrêt brutal :
    # le Drop ne court pas sur un TerminateProcess.
    MULTIFENETRE_VDD_PURGE=1 "${RACINE_HARNAIS}/scripts/run-agent.sh" >/tmp/lot3-purge.log 2>&1 || true
    grep -c "purge" /tmp/lot3-purge.log || echo 0
}
```

- [ ] **Step 4: Voir `agent_absent` rouge, puis vert**

```bash
unset -f chpwd 2>/dev/null; set -a && source .env && set +a
source docs/superpowers/plans/journaux-lot3/instrument/harnais.sh
vm_prete && agent_absent          # attendu : « aucun agent survivant »
scripts/run-agent.sh >/tmp/lot3-t0-agent.log 2>&1 &
sleep 20
agent_absent; echo "code=$?"      # attendu : ROUGE, « 1 agent(s) survivant(s) », code=1
scripts/stop-agent.sh
agent_absent; echo "code=$?"      # attendu : vert, code=0
```

🔴 **Le bras du milieu est le point de la tâche** : sans lui, `agent_absent` serait un contrôle qui ne peut pas échouer.

- [ ] **Step 5: Écrire le `README.md` du répertoire de journaux**

Il dit : ce que le harnais fait, ce qu'il **ne fait pas** (il ne juge rien), et la liste des items qui l'emploient, avec le chemin de leur propre répertoire de journaux.

- [ ] **Step 6: Commit**

```bash
git add docs/superpowers/plans/journaux-lot3/instrument/etat-vm.sh \
        docs/superpowers/plans/journaux-lot3/instrument/harnais.sh \
        docs/superpowers/plans/journaux-lot3/README.md
git commit -F /tmp/msg-t0.txt
```

Message : `harnais(lot 3) : le point unique de la campagne — et agent_absent vu ROUGE avant d'être cru`.

---

### Task 1: Les 48 chemins périmés — sans quoi rien ne se rejoue

🔴 **DÉCOUVERTE DU 28 AOÛT 2026, QUE NI LE CADRAGE NI LA SPEC N'AVAIENT VUE.**
Le dépôt a été déplacé de `/home/mallanic/Projects/Guacamole` vers
`/home/mallanic/Projects/Nivuus/packages/desk`. **48 scripts exécutables des
répertoires de journaux portent l'ancien chemin EN DUR**, dont **20 dans les
répertoires des douze pilotes** que la Task 5 doit rejouer :
`jouer-f2.sh` ouvre sur `RACINE=/home/mallanic/Projects/Guacamole`, et ce
répertoire existe encore, **vide**. Aucun rejeu n'est possible avant cette
tâche, et un rejeu tenté sans elle échouerait pour une raison qui n'a rien à
voir avec ce qu'il mesure.

⚠️ **Le même mécanisme a déjà mordu ailleurs** : le `target/` de Rust portait
ce chemin dans ses unités compilées, et cinquante tests échouaient sur un
`testdata` introuvable — diagnostiqué le 28 août 2026 avant la fusion de
`main`, remède `cargo clean -p agent -p proto`.

**Files:**
- Modify: les 48 fichiers listés par la commande de l'étape 1
- Create: `docs/superpowers/plans/journaux-lot3/chemins-perimes-<horodatage>.txt` (la liste, versionnée)

**Interfaces:**
- Consumes: rien.
- Produces: des scripts qui dérivent leur racine au lieu de la porter — patron déjà employé par `journaux-accent-a1/instrument/monter-a1.sh`, qui fait `cd "$(git rev-parse --show-toplevel)"`.

- [ ] **Step 1: Énumérer, et VERSIONNER la liste avant de toucher quoi que ce soit**

```bash
unset -f chpwd 2>/dev/null
grep -rl "Projects/Guacamole" docs/superpowers/plans/journaux-*/ \
  | grep -E '\.(sh|mjs|js|ps1|py|mts)$' | sort \
  | tee docs/superpowers/plans/journaux-lot3/chemins-perimes-$(date +%Y%m%dT%H%M).txt | wc -l
```

Expected: **48**. 🔴 **Relancer la commande plutôt que croire ce nombre** — il date du 28 août 2026, et une addition de journaux le change.

- [ ] **Step 2: Ne PAS toucher aux journaux**

```bash
grep -rl "Projects/Guacamole" docs/superpowers/plans/journaux-*/ \
  | grep -vE '\.(sh|mjs|js|ps1|py|mts)$' | wc -l
```

Ces fichiers-là (`.log`, `.json`, `.md`) sont des **pièces datées** : ils disent où la mesure a été faite le jour où elle l'a été. **Les réécrire serait falsifier une pièce.** Le contrôle de fin de tâche vérifie qu'aucun n'a bougé.

- [ ] **Step 3: Réparer, par la RACINE DÉRIVÉE et non par substitution aveugle**

Pour chaque script, remplacer l'affectation en dur par la dérivation :

```bash
# AVANT : RACINE=/home/mallanic/Projects/Guacamole
# APRÈS : RACINE="$(git rev-parse --show-toplevel)"
```

🔴 **Muter par un motif ancré sur la SYNTAXE (l'affectation), jamais par la
sous-chaîne du chemin** : dans un dépôt qui commente ses invariants, la
substitution frappe le commentaire avant le code. Pour chaque fichier :
`assert texte.count(ancre) == 1` avant d'écrire.
⚠️ Les `.mjs` dérivent autrement (`fileURLToPath(import.meta.url)`, déjà
employé par `pilote-f4.mjs`) ; les `.ps1` reçoivent leur racine **en
paramètre**, jamais par `git`, la VM n'ayant pas le dépôt.

- [ ] **Step 4: Le contrôle qui vaut — plus aucun chemin périmé dans un exécutable, et les pièces intactes**

```bash
grep -rl "Projects/Guacamole" docs/superpowers/plans/journaux-*/ \
  | grep -E '\.(sh|mjs|js|ps1|py|mts)$' | wc -l      # attendu : 0
git status --porcelain docs/ | grep -vE '\.(sh|mjs|js|ps1|py|mts)$|chemins-perimes' | wc -l   # attendu : 0
```

- [ ] **Step 5: La ROUGE — un script réparé se lance depuis un AUTRE répertoire courant**

```bash
cd /tmp && bash "$OLDPWD/docs/superpowers/plans/journaux-pont-fichiers-f2/instrument/jouer-f2.sh" --aide 2>&1 | head -3
```

Expected: il trouve ses fichiers. 🔴 **Puis le voir rouge** : depuis un répertoire hors du dépôt, `git rev-parse` échoue — le script doit **le dire** et s'arrêter, jamais poursuivre sur une racine vide. Si ce n'est pas le cas, ajouter le garde.

- [ ] **Step 6: `node --check` sur chaque `.mjs`/`.js` touché, `bash -n` sur chaque `.sh`**

```bash
while read -r f; do case "$f" in *.mjs|*.js) node --check "$f" || echo "🔴 $f";; *.sh) bash -n "$f" || echo "🔴 $f";; esac; done \
  < docs/superpowers/plans/journaux-lot3/chemins-perimes-*.txt
```

Expected: aucune ligne 🔴.

- [ ] **Step 7: Commit**

Message : `chemins(lot 3) : 48 scripts visaient le depot d'avant son deplacement — la racine se derive, les pieces datees ne bougent pas`.

---

### Task 2: Item 1 (3.7) — « temporaire + renommage » sur un éditeur réel

🔴 **Le seul item du lot dont l'échec perdrait des données de l'utilisateur, et le seul que ce lot CORRIGE.**

**Files:**
- Create: `docs/superpowers/plans/journaux-lot3-pont-sauvegarde/instrument/observer-editeurs.ps1`
- Create: `docs/superpowers/plans/journaux-lot3-pont-sauvegarde/instrument/jouer-item1.sh`
- Create: `docs/superpowers/plans/journaux-lot3-pont-sauvegarde/journal-<horodatage>.log` (produit par l'exécution, versionné)
- Modify (seulement si la mesure montre une perte) : `agent/src/pont/notifications.rs` et/ou `agent/src/pont/mutation.rs`

**Interfaces:**
- Consumes: `harnais.sh::vm_prete`, `agent_absent`, `purger_orphelins` (Tasks 0 et 1) ; le pilote `docs/superpowers/plans/journaux-pont-fichiers-f2/instrument/pilote-f2.mjs` **par lecture**, jamais par copie.
- Produces: le relevé `sequence-<editeur>.json` — pour chaque éditeur, la suite ordonnée des notifications ProjFS observées.

- [ ] **Step 1: Monter le pont et vérifier qu'il vit**

```bash
unset -f chpwd 2>/dev/null; set -a && source .env && set +a
source docs/superpowers/plans/journaux-lot3/instrument/harnais.sh
vm_prete && agent_absent && purger_orphelins
bash docs/superpowers/plans/journaux-pont-fichiers-f2/instrument/jouer-f2.sh "lot3-item1" 60
```

Expected: la page-shell tient une session, la racine du pont est visible côté VM.

🔴 **UNE SONDE EXISTE DÉJÀ, ET ELLE A TRANCHÉ LA MOITIÉ DE LA QUESTION.**
`docs/superpowers/plans/journaux-pont-fichiers-f2/instrument/sonde-idiome.ps1`
a été exécutée deux fois le 21 août 2026, en session 0 et en session 1, et son
verdict est versé à côté d'elle : les **cinq** outils éprouvés — `WriteAllText`,
`Add-Content`, `cmd >`, `Set-Content` et **`notepad.exe` en session
interactive** — écrivent **EN PLACE**. ⚠️ **Sa portée exacte, qu'elle énonce
elle-même** : un répertoire **NTFS ordinaire, PAS une racine ProjFS**, et elle
ne dit rien de VS Code, ni de Word, ni de LibreOffice.

**Ce que cette tâche ajoute donc, et rien de plus** : la même question **dans
la racine ProjFS**, et **VS Code**, que la sonde n'a pas éprouvé. La réutiliser
par lecture de son fichier d'origine, jamais par copie.

- [ ] **Step 2: OBSERVER, avant de supposer — écrire `observer-editeurs.ps1`**

Le script, pour chacun des trois éditeurs présents sur la VM (`C:\Windows\system32\notepad.exe`, `C:\Program Files\Windows NT\Accessories\wordpad.exe`, `C:\Program Files\Microsoft VS Code\Code.exe`) : ouvre un fichier de la racine du pont, y écrit un marqueur unique, enregistre, ferme.

⚠️ **Il ne conclut rien** : ce sont les notifications ProjFS de l'agent qui disent ce qui s'est passé.
⚠️ `LC_ALL=C grep -c '[^ -~]' observer-editeurs.ps1` doit rendre **0**.

- [ ] **Step 3: Relever la séquence RÉELLE, éditeur par éditeur**

```bash
grep -a "PRE_RENAME\|PRE_DELETE\|notification\|ecriture" /media/vm/dev/agent.log \
  | tail -80 | tee docs/superpowers/plans/journaux-lot3-pont-sauvegarde/sequence-brute.log
```

Expected: pour chaque éditeur, une suite ordonnée d'opérations. **Écrire laquelle**, éditeur par éditeur — c'est le livrable de l'étape, et il n'a pas de valeur attendue : on mesure ce qui est.

- [ ] **Step 4: Si aucun des trois n'emploie l'idiome, l'armer — et le DIRE**

VS Code porte un réglage de sauvegarde atomique. L'armer par son `settings.json` sur la VM, relancer l'étape 3, et **écrire dans le journal que le chiffre qui suit vient d'un idiome armé, non d'un comportement par défaut** : le chiffre change de sens.

- [ ] **Step 5: Le critère — la sauvegarde arrive-t-elle entière ?**

```bash
# Côté poste local (la racine servie par le navigateur), relever l'octet-à-octet.
sha256sum /tmp/f2-local/item1-sauvegarde.txt   # attendu : identique au marqueur écrit
ls -la /tmp/f2-local/ | grep -i "tmp\|~\|\.sav"   # attendu : AUCUN temporaire laissé
```

Expected: contenu identique, aucun résidu.

- [ ] **Step 6: La ROUGE — `PONT_MUTATION=0`**

```bash
scripts/stop-agent.sh
PONT_MUTATION=0 scripts/run-agent.sh > /tmp/lot3-item1-rouge.log 2>&1 &
```

Rejouer l'étape 2. Expected: le geste **échoue côté VM**, la **source reste présente**, la **cible est absente**, et le journal porte `mutations DESARMEES (PONT_MUTATION=0)`.

🔴 **Ne pas employer `PONT_ECRITURE=0` à sa place** : il pose `inscriptible=false`, ce qui fait refuser au `PRE_` pour une **autre** raison — une rouge de F3 y a été disqualifiée.

- [ ] **Step 7: Deuxième exécution des étapes 3 et 5**

Deux exécutions par bras, minimum. Relever les deux dans le journal versionné.

- [ ] **Step 8: Si la mesure montre une perte — corriger, rouge d'abord**

La rouge est déjà vue rouge (étape 6). Écrire le remède minimal dans `agent/src/pont/`, puis **rejouer les étapes 5 et 6** : le critère verdit, la rouge rougit toujours. ⚠️ Relever `wc -l` du fichier touché **avant** d'y écrire ; s'il approche 500, **extraire dans un commit dédié AVANT**.

- [ ] **Step 9: Commit**

```bash
git add docs/superpowers/plans/journaux-lot3-pont-sauvegarde/
git commit -F /tmp/msg-t2.txt
```

---

### Task 3: Item 2 (3.6) — les trois murs du pont, re-situés

**Files:**
- Create: `docs/superpowers/plans/journaux-lot3-pont-murs/instrument/paliers.sh`
- Create: `docs/superpowers/plans/journaux-lot3-pont-murs/releve-<horodatage>.json`

**Interfaces:**
- Consumes: `harnais.sh` (Tasks 0 et 1) ; `docs/superpowers/plans/journaux-pont-fichiers-f4/instrument/pilote-f4.mjs` et `analyser-f4.py` **par lecture**.
- Produces: un tableau à trois lignes — débit soutenu, taille de lecture maximale, rang d'énumération maximal — chacune avec sa valeur, son écart au relevé de F4 (30–33 Kio/s, 128 Kio, ~3 150 entrées), et la raison de l'écart s'il y en a un.

- [ ] **Step 1: Armer le recensement**

```bash
unset -f chpwd 2>/dev/null; set -a && source .env && set +a
source docs/superpowers/plans/journaux-lot3/instrument/harnais.sh
vm_prete && agent_absent
PONT_MESURE=1 scripts/run-agent.sh > /tmp/lot3-item2.log 2>&1 &
```

Expected: `banc de latence du pont ARME (PONT_MESURE=1) : instrument de banc, jamais une configuration livree`.

- [ ] **Step 2: Le bras « armé, AUCUN geste » — le témoin qui rend le reste discriminant**

Attendre deux périodes de recensement (2 × 10 s) sans toucher au pont.

```bash
grep -a "recensement" /media/vm/dev/agent.log | tail -3
```

Expected: `n:0` partout. 🔴 **Le contrôle qui vaut n'est pas que la ligne sorte, c'est qu'elle COMPTE ce qu'elle dit.**

- [ ] **Step 3: Les paliers de TAILLE, jusqu'au refus**

`paliers.sh` lit des fichiers de 16, 32, 64, 128, 256, 512 Kio depuis la racine du pont, et relève pour chacun : durée, issue, et le **différentiel** de l'histogramme entre deux recensements. ⚠️ **Les compteurs sont CUMULATIFS** : une mesure se lit par différence, jamais sur une ligne isolée.

Expected: le mur de lecture est **situé**. F4 le donnait à 128 Kio.

- [ ] **Step 4: Les paliers d'ENTRÉES, jusqu'au refus**

Répertoires de 500, 1 000, 2 000, 3 000, 3 150, 4 000 entrées. Relever durée et issue.
Expected: le rang maximal est **situé**. F4 le donnait à ~3 150 entrées, pour 18 à 24 s.

- [ ] **Step 5: Le débit soutenu**

Palier **plusieurs fois plus long** que la période de recensement (10 s) : au moins 60 s de lecture continue.
Expected: un débit en Kio/s, à comparer aux 30–33 Kio/s de F4.

- [ ] **Step 6: Deuxième exécution des étapes 3 à 5**

- [ ] **Step 7: Commit**

Message : `mesure(lot 3, item 2) : les trois murs du pont re-situes sur le produit d'aujourd'hui — et l'ecart a F4 nomme`.

---

### Task 4: Item 3 (3.8) — le répertoire frère qui disparaît

**Files:**
- Create: `docs/superpowers/plans/journaux-lot3-pont-frere/instrument/reproduire.sh`
- Create: `docs/superpowers/plans/journaux-lot3-pont-frere/maillons-<horodatage>.md`

**Interfaces:**
- Consumes: `harnais.sh` ; `PONT_CACHE=0` (variable livrée, bras de banc).
- Produces: le nom du maillon fautif, **ou** la liste des maillons disculpés avec la mesure qui disculpe chacun.

- [ ] **Step 1: Reproduire la disparition**

Créer deux répertoires frères sur la racine du pont, renommer le premier, lister le parent.
Expected: le frère **disparaît** du listage — le défaut de F5, reproduit.

- [ ] **Step 2: L'A/B d'attribution — `PONT_CACHE=0`**

```bash
scripts/stop-agent.sh
PONT_CACHE=0 scripts/run-agent.sh > /tmp/lot3-item3-sans-cache.log 2>&1 &
```

Rejouer l'étape 1. Expected: le frère **réapparaît** au listage suivant sans `Rafraichir` — le cache **prolonge** le défaut, il ne le crée pas (constat de F5, rejoué).

- [ ] **Step 3: Mesurer PAR MAILLON**

🔴 **Disculper un maillon ne désigne pas le coupable suivant.** Quatre maillons, quatre mesures distinctes :

| Maillon | Ce qu'on relève |
| --- | --- |
| la notification ProjFS reçue | `grep -a "PRE_RENAME\|RENAME" /media/vm/dev/agent.log` |
| ce que le pont en fait | la table (`agent/src/pont/table.rs`) — le frère y est-il encore ? |
| ce que le cache retient | `agent/src/pont/cache.rs` — l'entrée d'énumération du parent |
| ce que le poste local affiche | le listage côté navigateur |

- [ ] **Step 4: Écrire le verdict**

Soit le maillon est **nommé**, soit l'échec est écrit **avec ce qui a été éliminé et par quelle mesure**. ⚠️ Un échec à trouver n'est pas une preuve d'absence.

- [ ] **Step 5: Deuxième exécution, puis commit**


---

### Task 5: Le rejeu des douze pilotes (legs du lot 2)

**Files:**
- Create: `docs/superpowers/plans/journaux-lot3-pilotes/rejeu-<pilote>-<n>.log` (douze × deux, versionnés)
- Create: `docs/superpowers/plans/journaux-lot3-pilotes/verdicts.md`
- Modify: seulement les pilotes que le rejeu montre cassés, et **un commit par pilote**

**Interfaces:**
- Consumes: `harnais.sh` (Task 0), les chemins réparés (Task 1).
- Produces: pour chacun des douze, un verdict `établie|cassée` et l'écart au verdict métier d'origine.

**Les douze**, tels que le lot 2 les a énumérés (`grep -rln "signaling=" --include='*.mjs' --include='*.js' docs/superpowers/`) :
`accent-a1` · `micro-e2` · `micro-e3` (le pilote **et** son `injection-e3.js`) · `pont-fichiers` f1 à f5 · `presse-papier` p1 à p3.

⚠️ **Deux familles, et les confondre casse l'enrôlement** : les six à usage **navigateur seul** (f1→f5, e2, e3) suffixent `/signal` ; les quatre à **usage double** (accent-a1, pp-p1, pp-p2, pp-p3) gardent `SIGNALING_WS` **intacte** pour `SIGNALING_URL` de l'agent — contrat figé par le test `une_base_portant_deja_signal_casse_le_canal_agent` d'`agent/src/signaling.rs`.

- [ ] **Step 1: La ROUGE d'abord — un pilote pointé vers la racine nue**

🔴 **`SIGNALING_RELAIS` N'EST PAS UNE VARIABLE D'ENVIRONNEMENT** : c'est une
constante **dérivée dans le pilote** (`pilote-f1.mjs:32`,
`` `${SIGNALING.replace(/\/+$/, '')}/signal` ``), et la variable réellement lue
est `SIGNALING_WS`, qui est **la BASE**. La poser en environnement n'aurait
donc **aucun effet**, et la rouge serait verte pour une raison qui n'a rien à
voir avec ce qu'elle prétend éprouver.

**La rouge juste est une MUTATION du pilote**, depuis une **copie nommée** :

```bash
unset -f chpwd 2>/dev/null; set -a && source .env && set +a
source docs/superpowers/plans/journaux-lot3/instrument/harnais.sh
vm_prete && agent_absent
P=docs/superpowers/plans/journaux-pont-fichiers/instrument/pilote-f1.mjs
cp "${P}" /tmp/lot3-pilote-f1.copie          # 🔴 la copie NOMMÉE, jamais `git checkout --`
python3 - <<'MUT'
import io
p='docs/superpowers/plans/journaux-pont-fichiers/instrument/pilote-f1.mjs'
s=io.open(p,encoding='utf-8').read()
ancre="/signal`;"                            # ancrée sur la SYNTAXE, pas sur une sous-chaîne de prose
assert s.count(ancre)==1, s.count(ancre)
io.open(p,'w',encoding='utf-8').write(s.replace(ancre, "`;"))
MUT
node "${P}" /tmp/lot3-rouge-f1.json          # la racine nue d'avant auth-pomerium
cp /tmp/lot3-pilote-f1.copie "${P}"          # restauration DEPUIS LA COPIE
diff -q /tmp/lot3-pilote-f1.copie "${P}"     # le garde : la mutation a bien été défaite
```

Expected: **la session ne s'établit pas**. 🔴 **Sans ce bras, un vert ne distinguerait pas la réparation d'un chemin qui n'a jamais compté.** Relever le message exact.

- [ ] **Step 2: Rejouer les douze, un par un, DEUX fois chacun**

```bash
for p in f1 f2 f3 f4 f5; do
    agent_absent || scripts/stop-agent.sh
    bash docs/superpowers/plans/journaux-pont-fichiers*/instrument/jouer-${p}.sh "lot3-1" 40
done
```

🔴 **Deux exécutions ne se chevauchent JAMAIS** : F1 en a perdu une — deux se sont recouvertes de 2 min 23 s, la seconde a tué l'agent de la première **en pleine mesure**, et le journal versé sous le nom de la première était celui de la seconde.

- [ ] **Step 3: Pour chacun, relever DEUX choses distinctes**

| Ce qu'on relève | Comment |
| --- | --- |
| la session s'établit | `ice=connected` dans le journal du pilote |
| le verdict métier d'origine est retrouvé | la comparaison au document de résultats du sous-bloc concerné |

⚠️ **Le premier ne vaut pas le second** : une session établie avec un verdict métier changé est un **écart**, pas un succès.

- [ ] **Step 4: Écrire `verdicts.md` — douze lignes, chacune avec son écart**

- [ ] **Step 5: Relever `etat-vm.sh` avant/après la campagne entière de cette tâche**

Expected: mêmes `extinctions`. Si le compteur a bougé, **les mesures prises à cheval sur l'extinction sont disqualifiées** et rejouées.

- [ ] **Step 6: Commit** — `recette(lot 3, tache 5) : les douze pilotes rejoues, la rouge de la racine nue vue rouge`

---

### Task 6: Item 5 (3.3) — l'extinction du superviseur, et le chemin qui n'existe pas

🔴 **Le cadrage disait « jamais exercé ». Le relevé du 28 août 2026 dit « n'existe pas »** : `scripts/stop-agent.sh` ne fait que `schtasks /end` puis `Stop-Process -Name agent -Force`, et **aucun** gestionnaire de signal console n'existe dans `agent/src`.

**Files:**
- Create: `docs/superpowers/plans/journaux-lot3-extinction/orphelins-<horodatage>.log`
- Modify (seulement si l'étape 5 le décide) : `agent/src/main.rs`, `agent/src/superviseur.rs`

**Interfaces:**
- Consumes: `harnais.sh::purger_orphelins`.
- Produces: le **compte d'orphelins** laissés par une extinction brutale — sorties virtuelles, racines ProjFS, processus survivants.

- [ ] **Step 1: Constater l'absence par la commande, jamais de mémoire**

```bash
grep -rn "SetConsoleCtrlHandler\|ctrlc\|set_handler" agent/src/ | wc -l    # attendu : 0
cat scripts/stop-agent.sh
grep -rn "impl Drop" agent/src/moniteurs_virtuels.rs agent/src/superviseur/lanceur.rs
```

Expected: zéro gestionnaire ; deux `Drop` **structurellement inatteignables** par l'outillage livré.

- [ ] **Step 2: Lancer le superviseur, ouvrir trois fenêtres, relever l'état AVANT**

```bash
node scripts/winrm.js 'agent.exe' # via MULTIFENETRE_DXGI=1 : la topologie, depuis un processus NEUF
```

Expected: le nombre de sorties virtuelles vivantes, et la racine ProjFS présente.

- [ ] **Step 3: Tuer par le chemin livré, puis compter ce qui reste**

```bash
scripts/stop-agent.sh
node scripts/winrm.js '(Get-Process agent -ErrorAction SilentlyContinue | Measure-Object).Count'
MULTIFENETRE_DXGI=1 scripts/run-agent.sh > /tmp/lot3-t6-apres.log 2>&1
```

Expected: un **chiffre** d'orphelins. C'est le livrable de la tâche.

- [ ] **Step 4: La ROUGE — après purge, ce compte doit être ZÉRO**

```bash
purger_orphelins
MULTIFENETRE_DXGI=1 scripts/run-agent.sh > /tmp/lot3-t6-purge.log 2>&1
```

Expected: zéro sortie virtuelle. 🔴 **Un zéro qui resterait zéro à l'extinction suivante dirait que la mesure ne mesure rien** — donc rejouer les étapes 2 et 3 après la purge, et retrouver un compte non nul.

- [ ] **Step 5: Décider, et ne pas décider en douce**

Si le remède — un gestionnaire de signal console qui laisse courir les `Drop` déjà écrits — tient dans cette tâche, l'écrire, et **rejouer l'étape 3** : le compte d'orphelins doit tomber à zéro **sans purge**. S'il déborde, **s'arrêter sur le legs chiffré** : le choix de le construire revient au propriétaire du dépôt.

- [ ] **Step 6: Commit**

---

### Task 7: Item 6 — la file du capteur en charge réelle (legs du lot 2)

**Files:**
- Create: `docs/superpowers/plans/journaux-lot3-file-capteur/instrument/boucher.ps1`
- Create: `docs/superpowers/plans/journaux-lot3-file-capteur/releve-<n>.log`

**Interfaces:**
- Consumes: `harnais.sh` ; `agent/src/capteur/sommeil/file.rs` (`PROFONDEUR_MAX = 64`) **en lecture seule**.
- Produces: le relevé de la trace `file d'une session pleine : message REFUSE (trace au palier, puissance de deux)`, avec `session_cible`, le compte de refus et `profondeur_max`.

- [ ] **Step 1: Relire les constantes AVANT de dimensionner quoi que ce soit**

```bash
grep -n "PROFONDEUR_MAX\|const " agent/src/capteur/sommeil/file.rs | head
```

🔴 **Le palier de mesure se dérive de ces constantes, il ne se choisit pas.** `PROFONDEUR_MAX = 64` : il faut donc plus de 64 messages non consommés sur **une** session.

- [ ] **Step 2: Le bras NOMINAL d'abord — zéro refus, avec son témoin**

Lancer huit fenêtres en régime normal, 3 minutes.

```bash
grep -ac "message REFUSE" /media/vm/dev/agent.log            # attendu : 0
grep -ac "attache au capteur" /media/vm/dev/agent.log        # attendu : > 0, le TÉMOIN
```

🔴 **Le second compte est ce qui rend le premier interprétable** : un zéro seul serait rendu par un produit entièrement en panne.

- [ ] **Step 3: BOUCHER réellement une file**

`boucher.ps1` suspend le fil de fenêtre d'une session (`SuspendThread` sur le processus enfant relevé par son PID), pendant que les sept autres poussent leurs variantes.

🔴 **Tuer et suspendre par PID relevé**, jamais par nom ni par rang : l'ordre est superviseur, capteur, pont, **puis** les enfants.

- [ ] **Step 4: Le critère — la trace SORT, et compte ce qu'elle dit**

```bash
grep -a "message REFUSE" /media/vm/dev/agent.log | tail -8
```

Expected: la ligne porte `session_cible=<id>`, un compte de refus, `profondeur_max=64` — et les paliers successifs progressent **en puissances de deux** (1, 2, 4, 8, …).

⚠️ **Un compteur de journal peut compter des LIGNES et non des ÉVÉNEMENTS** : c'est le palier qui doit progresser, pas le nombre de lignes.

- [ ] **Step 5: Deuxième exécution des étapes 2 à 4**

- [ ] **Step 6: Écrire ce que la tâche NE ferme PAS**

⚠️ **Un `Sommeil` refusé reste PERDU** (tracé, non réémis) — legs déclaré par le lot 2, que cette tâche **mesure** sans le fermer : le remède casserait la preuve de terminaison de la boucle actuelle.

- [ ] **Step 7: Commit**

---

### Task 8: Item 7 (3.1) — la latence capture → affichage

🔴 **Jamais mesurée par aucun sous-bloc depuis D1.**

**Files:**
- Create: `docs/superpowers/plans/journaux-lot3-latence/instrument/sonde-latence.js` (injectée par CDP, **jamais** dans `client/src/`)
- Create: `docs/superpowers/plans/journaux-lot3-latence/instrument/pilote-latence.mjs`
- Create: `docs/superpowers/plans/journaux-lot3-latence/distribution-<n>.json`

**Interfaces:**
- Consumes: `commun-f1.mjs` (`Cdp`, `attendreDevtools`, `dodo`, `lancerChrome`) **par lecture** ; `journaux-pont-fichiers/instrument/it-mire.ps1` pour la mire animée.
- Produces: une **distribution** (médiane, p90, p99) de `now - captureTime`, par régime (une fenêtre, puis N).

- [ ] **Step 1: Écrire la sonde — `requestVideoFrameCallback`, rien d'autre**

```javascript
// Sonde de latence du lot 3 — injectée dans la page de session par CDP.
//
// 🔴 ELLE N'EST PAS UN CHANGEMENT DE PRODUIT : le client n'appelle pas
// requestVideoFrameCallback, et cette sonde ne l'y ajoute pas — elle
// s'attache depuis l'extérieur, le temps de la mesure.
//
// L'agent annonce l'instant de capture au pair par le sender report RTCP
// (agent/src/transport/piste_video.rs, tenu par un test) ; le navigateur le
// rend dans metadata.captureTime. La soustraction EST la latence.
(() => {
  const echantillons = [];
  const attacher = (video) => {
    const tour = (maintenant, metadata) => {
      // captureTime est ABSENT tant que l'horloge distante n'est pas connue :
      // un échantillon sans lui n'est pas un zéro, il n'existe pas.
      if (typeof metadata.captureTime === 'number') {
        echantillons.push({
          latence_ms: metadata.presentationTime - metadata.captureTime,
          rtp: metadata.rtpTimestamp,
          horodatage: maintenant,
        });
      }
      video.requestVideoFrameCallback(tour);
    };
    video.requestVideoFrameCallback(tour);
  };
  document.querySelectorAll('video').forEach(attacher);
  window.__latenceLot3 = () => echantillons;
})();
```

- [ ] **Step 2: Le TÉMOIN NÉGATIF — la sonde doit pouvoir ne rien rendre**

Injecter la sonde sur une page **sans** flux vidéo.
Expected: `window.__latenceLot3()` rend `[]`. 🔴 **Un zéro rendu par une sonde qui ne peut rien rendre n'est pas une mesure** ; ce bras établit que le tableau non vide du bras suivant vient bien du flux.

- [ ] **Step 3: Monter la mesure — mire ANIMÉE, à cadence connue**

🔴 **Une mire immobile ne produit aucune image** : Desktop Duplication n'émet qu'au changement. Employer `it-mire.ps1`, qui anime **et affiche sa propre cadence**.

⚠️ Chrome avec `--disable-background-timer-throttling`, `--disable-backgrounding-occluded-windows`, `--disable-renderer-backgrounding` ; visibilité et focus **imposés page par page** ; toute évaluation CDP **bornée**.

- [ ] **Step 4: Relever la distribution, régime par régime**

Une fenêtre, puis quatre, puis huit — **le régime est nommé dans le relevé**, une latence sans son régime n'est pas attribuable.
Expected: médiane, p90, p99, et le nombre d'échantillons.

- [ ] **Step 5: La ROUGE — le réseau dégradé doit DÉPLACER la distribution**

```bash
scripts/netem.sh adsl        # 8 Mbit/s, 30 ms de latence, 5 ms de gigue
# … rejouer l'étape 4 …
scripts/netem.sh off         # ⚠️ TOUJOURS reposer `off`
```

Expected: la médiane **monte**. 🔴 Sans ce bras, un chiffre plausible serait indiscernable d'un instrument qui mesure autre chose.

- [ ] **Step 6: Deuxième exécution des étapes 4 et 5**

- [ ] **Step 7: Écrire ce que la mesure N'ÉTABLIT PAS**

La latence **verre à verre** : le maillon d'affichage après `presentationTime` n'y est pas, et la **boucle d'entrée** pas du tout.

- [ ] **Step 8: Commit**

---

### Task 9: Item 8 (3.2) — le propriétaire mono-fenêtre

**Files:**
- Create: `docs/superpowers/plans/journaux-lot3-mono/releve-<n>.log`

**Interfaces:**
- Consumes: `harnais.sh` ; `journaux-presse-papier-p1/instrument/copieur-pp.ps1` et `pilote-pp-p1.mjs` **par lecture** ; `journaux-accent-a1/instrument/pilote-accent-a1.mjs`.
- Produces: un oui/non mesuré, avec le **compte de messages**, pour le presse-papier et pour l'accent, en mode mono-fenêtre.

- [ ] **Step 1: Lancer l'agent SANS `SUPERVISEUR` ni `CAPTEUR`**

```bash
unset -f chpwd 2>/dev/null; set -a && source .env && set +a
source docs/superpowers/plans/journaux-lot3/instrument/harnais.sh
vm_prete && agent_absent
SUPERVISEUR=0 CAPTEUR=0 scripts/run-agent.sh > /tmp/lot3-t9.log 2>&1 &
```

🔴 **Lire la ligne dans le `run-agent.ps1` GÉNÉRÉ sur la VM**, pas dans le script hôte :

```bash
grep -a "SUPERVISEUR\|CAPTEUR" /media/vm/dev/run-agent.ps1
```

- [ ] **Step 2: Le geste — copier dans la VM, lire au navigateur**

Expected: un **compte de messages** `presse-papier`, zéro ou non-zéro. C'est la réponse à l'item : le propriétaire existe-t-il en mono-fenêtre ?

- [ ] **Step 3: Le même pour l'accent**

Expected: le compte d'annonces `accent de la fenetre Windows`.

- [ ] **Step 4: La ROUGE — `PRESSE_PAPIER=0` et `ACCENT=0`**

Expected: **zéro message** dans les deux cas, et les traces de désarmement.
🔴 **Le contrôle qui vaut est le zéro de messages, jamais la trace** : elle prouve que la variable a atteint le processus, pas que le mécanisme est coupé — et c'est le bras SANS la variable qui rend ce zéro discriminant.

- [ ] **Step 5: Deuxième exécution, puis commit**

---

### Task 10: Item 9 (3.4) — les deux replis micro, et la faute jamais armée

**Files:**
- Create: `docs/superpowers/plans/journaux-lot3-micro/replis-<n>.log`

**Interfaces:**
- Consumes: `harnais.sh` ; `MICRO_PERIPHERIQUE`, `MICRO_FAUTE_ECRITURE` (variables livrées) ; `journaux-micro-e3/instrument/jouer-e3.sh` **par lecture**.
- Produces: le relevé des trois branches de repli de `resoudre` (`agent/src/micro/boucle_locale.rs`) et du chemin d'échec d'écriture WASAPI (`agent/src/wasapi/ecriture.rs:392`).

- [ ] **Step 1: Le cas NOMINAL d'abord — le témoin**

`MICRO_PERIPHERIQUE` **absente**, donc la désignation intégrée `"VB-Audio"`.

```bash
grep -a "cable de rendu retenu" /media/vm/dev/agent.log
```

Expected: la ligne porte `integree=true` et un `critere=`. 🔴 **Comparer la valeur RETENUE, jamais la seule présence de la ligne.** Et `ready` porte `mic: true`.

- [ ] **Step 2: Repli `Introuvable`**

```bash
MICRO_PERIPHERIQUE="peripherique-qui-n-existe-pas-lot3" scripts/run-agent.sh > /tmp/lot3-t10-introuvable.log 2>&1 &
```

Expected: **aucun fil de rendu**, `mic: false`, un `warn!` **portant l'inventaire** des périphériques. 🔴 **Aucun repli vers le défaut de Windows** : ici, « la voix de l'utilisateur dans le mauvais tuyau » serait une **fuite**, pas un moindre mal.

- [ ] **Step 3: Repli `Ambigu`**

Une sous-chaîne que **deux** points de terminaison portent (relever l'inventaire de l'étape 2 pour en choisir une réelle).
Expected: `mic: false`, et le refus de trancher — jamais le premier de la liste.

- [ ] **Step 4: `MICRO_FAUTE_ECRITURE` — jamais armée à ce jour**

```bash
MICRO_FAUTE_ECRITURE=5 scripts/run-agent.sh > /tmp/lot3-t10-faute.log 2>&1 &
grep -a "injection de fautes d'ecriture du micro ARMEE" /media/vm/dev/agent.log
```

Expected: la trace d'armement, puis **cinq** échecs d'écriture consommés. ⚠️ **Budget GLOBAL au processus** : si le compte ne quitte jamais zéro, c'est le budget qu'il faut regarder avant le produit — la panne de mesure que D10 a payée.

- [ ] **Step 5: Deuxième exécution des étapes 2 à 4, puis commit**

---

### Task 11: Item 10 (3.5) — une cause naturelle de mort de capture audio

**Files:**
- Create: `docs/superpowers/plans/journaux-lot3-audio/causes-tentees.md`

**Interfaces:**
- Consumes: `harnais.sh` ; `AUDIO_FAUTE_LECTURE`, `AUDIO_FAUTE_RECONSTRUCTION` (pour le **témoin**, pas pour la mesure).
- Produces: soit une cause naturelle **nommée et reproduite**, soit la **liste de ce qui a été tenté**.

- [ ] **Step 1: Le témoin — le remède fonctionne, et on sait à quoi ressemble une mort**

```bash
AUDIO_FAUTE_LECTURE=15 scripts/run-agent.sh > /tmp/lot3-t11-temoin.log 2>&1 &
```

Expected: au-delà de `LECTURES_ECHOUEES_MAX = 10`, la capture se déclare morte et la reconstruction part. **C'est la signature à reconnaître** dans les bras suivants.

- [ ] **Step 2: Les causes plausibles, une par exécution**

| Cause tentée | Geste |
| --- | --- |
| changement du périphérique de rendu **par défaut** en cours de session | `Set-AudioDevice` côté VM, session interactive |
| mise en veille du point de terminaison | désactiver puis réactiver le périphérique |
| arrêt du service audio | `Stop-Service Audiosrv` puis `Start-Service` |

🔴 **Une par exécution du binaire** : ces API échouent par **plantage du processus**, pas par code d'erreur, et deux causes dans la même exécution rendraient l'attribution impossible.

- [ ] **Step 3: Relever, pour chaque cause, si la signature de l'étape 1 apparaît**

- [ ] **Step 4: Écrire le verdict, et sa limite**

⚠️ **Un échec à trouver n'est pas une preuve d'absence**, et le document doit le dire dans ces termes : la liste de ce qui a été tenté est le livrable, autant que le verdict.

- [ ] **Step 5: Commit**

---

### Task 12: Item 11 (3.9) — les 71 applications `NonMesuree`, et l'icône qui change

**Files:**
- Create: `docs/superpowers/plans/journaux-lot3-apps/mesurees-<horodatage>.json`

**Interfaces:**
- Consumes: `harnais.sh` ; `agent/testdata/gapps-corpus-vm.json` (le corpus relevé) ; `APPS_SURVEILLANCE` (bras rouge).
- Produces: le compte de `NonMesuree` **après** campagne, et le verdict oui/non sur une icône mutée sans que son raccourci change.

- [ ] **Step 1: Relever le compte AVANT**

```bash
grep -ac "NonMesuree" /media/vm/dev/agent.log
```

🔴 **Le « 71 » vient de `CLAUDE.md` et date** : le relancer, ne pas le recopier.

- [ ] **Step 2: Lancer la mesure sur le corpus réel, et relever le compte APRÈS**

- [ ] **Step 3: Muter une icône SANS toucher au raccourci**

Remplacer l'icône d'un exécutable du catalogue, laisser le `.lnk` intact, attendre une période de réconciliation.
Expected: le catalogue la revoit — **ou ne la revoit pas**, ce qui est le défaut annoncé, mesuré.

- [ ] **Step 4: La ROUGE — `APPS_SURVEILLANCE=0`**

```bash
APPS_SURVEILLANCE=0 scripts/run-agent.sh > /tmp/lot3-t12-rouge.log 2>&1 &
grep -ac "racine surveillée" /media/vm/dev/agent.log     # attendu : 0
grep -a "mode de surveillance retenu" /media/vm/dev/agent.log
```

🔴 **Ce qui discrimine est l'ABSENCE de toute ligne `racine surveillée`**, jamais la trace de mode — elle prouve seulement que la variable a atteint le processus.

- [ ] **Step 5: Deuxième exécution, puis commit**

---

### Task 13: Item 12 (3.10) — le maillon fautif du `Resize`

**Files:**
- Create: `docs/superpowers/plans/journaux-lot3-resize/maillons-<horodatage>.md`

**Interfaces:**
- Consumes: `harnais.sh` ; le pilote de recette de D10 (`journaux-multifenetres-d10/instrument/pilote-critere2-d10.mjs`) **par lecture**.
- Produces: le maillon **nommé**, ou la liste des maillons disculpés avec la mesure qui disculpe chacun.

- [ ] **Step 1: Provoquer un `Resize` et relever le chemin, maillon par maillon**

| Maillon | Ce qu'on relève |
| --- | --- |
| l'annonce du navigateur | le message `Resize` émis, côté client |
| la réception par l'enfant | `transport/controle.rs` — le bras du `match` |
| le relais au capteur | le message dans le canal, et son **acceptation** (la file bornée peut le refuser) |
| l'application par la source | `windows_source/redimensionnement.rs` |
| la taille effectivement encodée | `encode.rs`, et l'image reçue |

- [ ] **Step 2: Écrire ce que chaque maillon rend**

🔴 **Disculper un maillon ne désigne pas le coupable suivant** : chaque ligne du tableau porte **sa** mesure.

- [ ] **Step 3: La ROUGE — un `Resize` en mode `SortieEntiere`**

Expected: `redimensionnement ignoré : la source capture une sortie DXGI entière (voir le constat de mesure de capteur::plein_ecran, sous-bloc D9)`. C'est un refus **connu et voulu** : il établit que l'instrument voit bien le maillon d'application.

- [ ] **Step 4: Écrire le verdict, ou l'échec avec ce qui a été éliminé**

- [ ] **Step 5: Commit**

---

### Task 14: La revue transverse, et la mise à jour de `CLAUDE.md`

🔴 **UNE REVUE PAR TÂCHE NE PEUT PAS VOIR UN DÉFAUT QUI FRANCHIT UNE FRONTIÈRE DE TÂCHE** — chacun est correct des deux côtés pris séparément. La revue transverse du lot 2 a trouvé que `CLAUDE.md` décrivait encore huit legs comme ouverts après que le code les avait fermés.

**Files:**
- Create: `docs/superpowers/plans/2026-08-28-lot3-campagne-vm-resultats.md`
- Modify: `CLAUDE.md`

**Interfaces:**
- Consumes: les treize documents et journaux des tâches précédentes.
- Produces: le document de clôture du lot, et un `CLAUDE.md` dont chaque affirmation touchée par le lot est vraie.

- [ ] **Step 1: Relire chaque message de commit CONTRE son diff**

```bash
git log --oneline main..campagne-vm | wc -l
git log main..campagne-vm --format='%H %s' | while read -r h s; do echo "== $s"; git show --stat "$h" | tail -5; done
```

🔴 **Un message de commit est une pièce du dépôt, et personne ne le relit.** Sept écarts sur neuf, dans un lot antérieur, n'existaient que là.

- [ ] **Step 2: Relever les tailles APRÈS la dernière édition de la ronde, la sienne comprise**

```bash
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
```

Expected: `agent/src/encode.rs` et `agent/src/windows_source.rs` **seuls**, sauf remède écrit par les Tasks 2 ou 6 — auquel cas le tableau de dette de `CLAUDE.md` se corrige **dans le même mouvement**.

- [ ] **Step 3: La suite entière, depuis un shell propre**

```bash
env -u TURN_URL -u TURN_SECRET ./scripts/verify-all.sh
```

⚠️ **Le script compte DIX étapes et affiche DIX-HUIT en-têtes `==>`** — dire lequel on annonce.
⚠️ Si `cargo test` rougit sur un `testdata` introuvable : `cargo clean -p agent -p proto` d'abord — le piège du chemin périmé, déjà payé le 28 août 2026.

- [ ] **Step 4: Chercher les affirmations que le lot a rendues FAUSSES**

Par le **sens**, pas par la formule : une négation se dit de plusieurs façons, et c'est celle qu'on n'a pas listée qui survit. Au minimum, relire dans `CLAUDE.md` : le § « Ce qu'aucun chantier n'a jamais mesuré » (la latence de bout en bout **cesse d'y être vraie** si la Task 8 a abouti), le tableau « Par sous-projet », et la ligne du chemin d'extinction propre.

- [ ] **Step 5: Écrire le document de résultats**

Il porte, en propre : le verdict de chaque item, **les rouges vues rouges**, un § « **Ce que ce lot n'établit PAS** » (aucun jugement humain, aucune constante calibrée, aucun mur déplacé, la latence verre à verre hors périmètre), et les **legs neufs chiffrés**.

- [ ] **Step 6: Mettre `CLAUDE.md` à jour — une ligne à l'index, les legs fermés ET les legs neufs**

🔴 **Ne recopier AUCUN chiffre sans relancer sa commande.** Un relevé daté n'est pas une consigne : ce qui porte une date va au journal, ce qui porte une règle reste.

- [ ] **Step 7: Commit, puis proposer la fusion dans `main`**

⚠️ **La fusion appartient au propriétaire du dépôt**, comme la poussée vers `origin`.

---

## Ce que ce plan sait ne PAS couvrir

- **Le lot 4** — les jugements humains : le critère ⑦ derrière Pomerium, les 25 jugements visuels de ⑥, l'écoute du micro, le niveau 2 du presse-papier, la calibration de toutes les constantes, `showDirectoryPicker()`.
- **Les trois couches inconnues du chantier D** : le plafond de 8 encodeurs, le plafond de 4 processus tenant une duplication, le mécanisme de l'abandon du mutex DXGI. Ce lot les **contourne** comme tous les précédents.
- **La scalabilité horizontale de la plateforme** et **TURNS sur 443** : ni l'une ni l'autre n'est atteignable par une mesure sur cette VM.
- **Le WCO**, écarté par décision tant que le hub n'est pas installable.
