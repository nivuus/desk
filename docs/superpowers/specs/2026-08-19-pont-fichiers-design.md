# Sous-projet ③ — Pont fichiers : ProjFS ⇆ File System Access API

**Date** : 19 août 2026
**Cadrage** : `2026-07-27-refonte-produit-design.md` §5 ③, §7, §9.2, §10, et son
**amendement du 28/07/2026 sur le sélecteur de fichiers Windows**.
**Objet** : exposer, dans la VM Windows de l'utilisateur, un lecteur virtuel
« Mes Fichiers » dont le contenu est un répertoire du poste local que le
navigateur a ouvert par `showDirectoryPicker()`.

---

## 1. Objet, et ce que ce sous-projet remplace

Le produit historique montait le répertoire de l'utilisateur dans la VM par
**FUSE côté serveur Linux + WebSocket + File System Access API côté
navigateur** (`src/file.js`, `web/index.js:427-675`). Ce pont est la cause
nommée de la refonte : le crash `double free or corruption` de la chaîne
`guacd`/`fuse-native` est cité en tête du cadrage (§1) et occupe une section
entière de `CLAUDE.md` (« Problèmes Connus Non Résolus », n°1), avec sa cause
probable — une course entre la fermeture du WebSocket et une opération FUSE en
vol.

Ce sous-projet ne le porte pas ; il le remplace. Le montage change de côté
(le lecteur vit **dans la VM**, plus sur l'hôte Linux), de mécanisme (ProjFS,
un composant Windows de première partie, plus un module noyau tiers non
maintenu) et de transport (data channel WebRTC de bout en bout, plus un
WebSocket qui traverse la plateforme).

**L'ancien pont n'est ni modifié ni supprimé** (cadrage §11) : il reste
fonctionnel jusqu'au remplacement. Il sert ici de **source de faits**, jamais
de modèle d'architecture — l'annexe §13 en donne l'inventaire, relevé ligne à
ligne, et c'est de lui que sortent trois décisions de périmètre de cette spec.

---

## 2. Préalable — l'état de ProjFS sur la VM, relevé

> ✅ **CE RELEVÉ EST DE L'HISTOIRE DEPUIS LE 19 AOÛT 2026 : le sous-bloc F0 a
> ACTIVÉ ProjFS**, et toutes les phrases de cette spec qui parlent de l'état
> « **aujourd'hui** » ou du « **ROUGE aujourd'hui** » (ici, et aux §3.1, §8 et
> §10) décrivent désormais un état révolu. Elles sont **datées, donc vraies
> comme histoire**, et conservées telles quelles — mais le mot « aujourd'hui »
> ne porte pas sa date au niveau de la phrase, d'où cet encadré.
> État courant, relevé par la commande : `State : Enabled`,
> `ProjectedFSLib.dll : True`, `PrjFlt.sys : True`, filtre `PrjFlt` chargé à
> l'altitude 189800, service `Running`/`Automatic`
> (`docs/superpowers/plans/journaux-pont-fichiers/f0-apres.txt`).
> La recette de F1 a dû **renommer la DLL** pour reconstituer le rouge.

**Relevé le 19 août 2026, par WinRM, en lecture seule**, sur la VM `Windows`
(`virsh list --all` : « en cours d'exécution »). Aucune activation, aucune
installation, aucune compilation n'a été faite ; l'agent concurrent qui
conduisait ses recettes n'a pas été touché.

| Question | Commande | Résultat |
| --- | --- | --- |
| Système | `Get-CimInstance Win32_OperatingSystem` | **Microsoft Windows Server 2022 Standard**, version 10.0.20348, build **20348** |
| La fonctionnalité existe-t-elle au catalogue ? | `Get-WindowsOptionalFeature -Online \| Select -Expand FeatureName` | **`Client-ProjFS` présent** |
| Est-elle activée ? | `Get-WindowsOptionalFeature -Online -FeatureName Client-ProjFS` | **`State : Disabled`**, `RestartRequired : Possible` |
| La bibliothèque est-elle là ? | `Test-Path C:\Windows\System32\ProjectedFSLib.dll` | **`False`** |
| Le pilote de filtre est-il là ? | `Test-Path C:\Windows\System32\drivers\PrjFlt.sys` | **`False`** |
| Existe-t-elle comme *rôle serveur* ? | `Get-WindowsFeature \| Select -Expand Name` filtré sur `Proj` | **aucune correspondance** |

**Trois faits, et pas un de plus.**

1. **ProjFS est disponible sur cette VM, et il est DÉSACTIVÉ.** Ce n'est ni
   « absent » ni « impossible » : le paquet est au catalogue, la
   fonctionnalité s'active. Le nom retenu par Windows Server 2022 est celui du
   **catalogue des fonctionnalités facultatives** (`Client-ProjFS`,
   `Get-WindowsOptionalFeature`), **pas** celui du gestionnaire de rôles
   (`Get-WindowsFeature` ne le connaît pas). Un script d'activation qui
   emploierait `Install-WindowsFeature Projected-File-System` échouerait en
   silence sur cette machine.
2. **L'absence conjointe de `ProjectedFSLib.dll` et de `PrjFlt.sys` est
   cohérente avec `State: Disabled`** ; elle ne l'établit pas — c'est une
   corroboration, pas une preuve indépendante.
3. **`RestartRequired: Possible` est ce que le catalogue annonce AVANT
   activation, pas ce que l'activation exigera.** Rien ici ne dit qu'un
   redémarrage sera nécessaire ; rien ne dit qu'il ne le sera pas.

### 2.1 Ce que l'activation exige — préalable **F0**

```powershell
# Élévation requise (administrateur local de la VM).
Enable-WindowsOptionalFeature -Online -FeatureName Client-ProjFS -NoRestart
# Puis, si et seulement si l'appel rend RestartNeeded : $true
Restart-Computer
```

- **Droits** : administrateur local. Le compte `WINDOWS_ADMIN_USERNAME` de
  `.env` en dispose (c'est celui par lequel WinRM administre déjà cette VM,
  `CLAUDE.md` § « Configuration Critique »).
- **Redémarrage** : à décider *sur le retour de la commande*, pas d'avance.
- **Contrôle d'acceptation de F0**, et il doit pouvoir échouer :
  `Get-WindowsOptionalFeature -Online -FeatureName Client-ProjFS` rend
  `State : Enabled` **et** `Test-Path C:\Windows\System32\ProjectedFSLib.dll`
  rend `True`. **ROUGE aujourd'hui** — les deux valeurs relevées au §2 sont
  `Disabled` et `False` : le contrôle est donc vérifié capable de dénoncer
  l'état qu'il existe pour dénoncer, ce qui est la seule façon de savoir qu'il
  contrôle quelque chose.
- ⚠️ **Ce préalable est aussi une exigence de PRODUIT, pas seulement de
  laboratoire.** Toute VM provisionnée par la plateforme (cadrage §5 ⑤) devra
  porter ProjFS activé. Cela appartient à l'image de base ou au
  provisionnement, **pas à l'agent** : un agent qui activerait une
  fonctionnalité Windows et redémarrerait la machine sous l'utilisateur serait
  un comportement que rien dans le cadrage n'autorise.

### 2.2 Ce que ce relevé n'établit PAS

- **Rien du fonctionnement de ProjFS une fois activé** : ni qu'il démarre, ni
  qu'il projette, ni ses latences. Le §2 dit qu'on peut l'allumer, pas ce qu'on
  verra ensuite.
- **Rien de la version de ProjFS** : l'API a gagné des entrées entre 1809 et
  aujourd'hui (`PrjWritePlaceholderInfo2`, `PrjFillDirEntryBuffer2`). Build
  20348 les porte selon la documentation Microsoft ; **cela n'a pas été
  vérifié sur cette machine**, et la conception n'emploie que les entrées de
  la première génération (§4.3) pour n'en pas dépendre.
- **Rien d'une autre VM.** Ce relevé porte sur une machine, un jour.

---

## 3. Décisions tranchées

Aucune question n'est laissée ouverte. Chaque ligne porte sa justification et
son coût.

| # | Décision | En un mot |
| --- | --- | --- |
| D1 | **Liaison ProjFS : `windows-rs`, aucune crate tierce, et les entrées résolues à l'EXÉCUTION** | §3.1 |
| D2 | **Le pont vit dans un TROISIÈME type de processus (`PONT=1`), un par VM, lancé et surveillé par le superviseur** | §3.2 |
| D3 | **Le répertoire est détenu par la PAGE-SHELL, pas par une fenêtre d'application** | §3.3 |
| D4 | **Le canal est une `RTCPeerConnection` DÉDIÉE, sans média, page-shell ⇆ pont** | §3.4 |
| D5 | **Périmètre v1 : lister, lire, écrire, créer, renommer, SUPPRIMER** — la suppression est ajoutée à la liste du cadrage §10, et §3.5 dit pourquoi elle n'est pas optionnelle | §3.5 |
| D6 | **Aucune lettre de lecteur : une racine de virtualisation dans le profil utilisateur** | §3.6 |
| D7 | **Le write-back est POUSSÉ À LA FERMETURE DE CHAQUE HANDLE, pas à la fin de session** ; la fenêtre de perte est nommée et ne se referme pas | §6 |

### 3.1 D1 — la liaison ProjFS

**Trois voies pesées.**

**(a) Une crate tierce.** Écartée sans appel. C'est exactement ce qui a tué
l'ancien pont : `fuse-native` est nommé « non maintenu » dans le cadrage §1, et
sa chaîne native est la cause probable du `double free`. Adopter une seconde
enveloppe tierce autour d'une API Windows peu employée reproduirait la même
prise de risque, sur le même sous-système, dans le produit qui existe pour
l'avoir éliminée.

**(b) `windows-rs` avec la fonctionnalité, et appel direct des enveloppes.**
`agent/Cargo.toml` déclare déjà `windows = "0.62"` avec trente-six
fonctionnalités. Le crate **porte** ProjFS : la fonctionnalité
`Win32_Storage_ProjectedFileSystem` existe (relevé dans
`windows-0.62.2/Cargo.toml:554`) et le module correspondant
(`.../Win32/Storage/ProjectedFileSystem/mod.rs`, **621 lignes**) expose les
**19** entrées `Prj*`, les **8** types de rappel et les structures nécessaires
— tout ce dont cette conception a besoin, **rien n'est à écrire à la main**.

⚠️ **Mais l'appel direct des enveloppes est refusé, pour une raison
structurelle.** Chacune est un `#[inline] fn` contenant
`windows_core::link!("projectedfslib.dll" …)`, et ce `link!` se développe en
`#[link(name = …, kind = "raw-dylib", modifiers = "+verbatim", import_name_type = "undecorated")]`
(`windows-link-0.2.1/src/lib.rs:9`, **relevé**). Un `raw-dylib` produit une
entrée d'**import statique** dans le PE. **Inférence, déclarée comme telle** :
un import statique se résout au chargement, donc un `agent.exe` qui
référencerait ne serait-ce qu'une de ces entrées ne se chargerait plus du tout
sur une machine où `ProjectedFSLib.dll` est absent — c'est-à-dire dans l'état
exact où se trouve la VM aujourd'hui (§2). *Ceci est déduit du fonctionnement
des tables d'import PE, pas mesuré ici.*

**Ce que cela coûterait est disproportionné et il faut le dire en toutes
lettres** : `agent.exe` est **un seul binaire** pour les ~~trois~~ **QUATRE**
modes (superviseur, capteur, enfant, **et le pont depuis la tâche 9 de F1** —
`agent/src/main.rs:449` pour la branche `PONT` ; ⚠️ *le renvoi
`agent/src/main.rs:303-327` publié ici ne désigne plus l'aiguillage mais le
`#[cfg(test)] mod tests` d'`analyser_hwnd`*). Le fond du raisonnement est
INCHANGÉ, et le pont l'a même renforcé : un import
non résolu ne tuerait donc pas « le pont » : il tuerait **la capture, la vidéo
et l'input** sur toute VM sans ProjFS. Le cadrage §4 principe 4 exige
l'inverse, et §7 exige « vidéo intacte ».

**(c) Retenue — `windows-rs` pour les TYPES, résolution des ENTRÉES à
l'exécution.**

- La fonctionnalité `Win32_Storage_ProjectedFileSystem` est ajoutée à
  `agent/Cargo.toml`. Elle apporte `PRJ_CALLBACKS`, `PRJ_CALLBACK_DATA`,
  `PRJ_PLACEHOLDER_INFO`, `PRJ_FILE_BASIC_INFO`,
  `PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT`, `PRJ_STARTVIRTUALIZING_OPTIONS`,
  `PRJ_NOTIFICATION_MAPPING`, `PRJ_COMPLETE_COMMAND_EXTENDED_PARAMETERS`, les
  huit `PRJ_*_CB` et les constantes `PRJ_NOTIFICATION_*` / `PRJ_NOTIFY_*` /
  `PRJ_FLAG_*` (tous **relevés** dans le module cité). **Ce sont des données et
  des types de pointeur de fonction : les déclarer n'émet aucun import.**
- **Aucune enveloppe `Prj*` de `windows-rs` n'est appelée.** Un module
  `agent/src/pont/projfs/chargement.rs` fait `LoadLibraryW("ProjectedFSLib.dll")`
  puis `GetProcAddress` sur les **treize** entrées nécessaires (§4.3), et les
  transcrit en `transmute` vers des `unsafe extern "system" fn` dont les
  signatures sont recopiées **du module `windows-rs` lui-même**, qui reste
  ainsi la source de vérité.
- **Bénéfice direct, et il n'est pas théorique** : l'absence de ProjFS devient
  une condition d'exécution que le pont **rapporte** (`ERROR_MOD_NOT_FOUND` au
  journal, un message à la page-shell), et le reste du produit ne s'en aperçoit
  pas. C'est le §7 du cadrage rendu structurel plutôt que défendu.
- **Bénéfice second, gratuit** : `cargo check --target x86_64-pc-windows-gnu`,
  le contrôle que ce dépôt fait tourner sur l'hôte Linux depuis D3, continue de
  couvrir tout ce code sans dépendre du comportement de `raw-dylib` sous la
  chaîne `gnu`.
- **Coût, écrit** : treize signatures recopiées à la main, donc treize
  occasions de se tromper d'ABI — et le compilateur ne peut plus rien en dire.
  **Mitigation** : un test d'hôte compare, pour chaque entrée, la signature
  transcrite au type `PRJ_*_CB` correspondant quand il en existe un, et un
  contrôle de revue impose de citer le numéro de ligne du module `windows-rs`
  en regard de chaque transcription. Ce n'est pas une garantie ; c'est ce qu'on
  peut faire.

### 3.2 D2 — qui exécute le pont

**Le cadrage veut un lecteur « Mes Fichiers » par UTILISATEUR** (§5 ③), donc
un par VM. Le modèle multi-fenêtres livré par le chantier D en compte trois
sortes de processus (`agent/src/main.rs:303-327`) :

- le **superviseur** (`SUPERVISEUR=1`) : détecte les fenêtres, lance les
  autres, ne capture rien ;
- le **capteur** (`CAPTEUR=1`) : tient les N duplications DXGI et les N
  encodeurs, sert les enfants par tube nommé ;
- les **N enfants** : un par fenêtre, WebRTC + entrée + audio.

**Quatre voies pesées.**

| Voie | Pourquoi elle est écartée, ou retenue |
| --- | --- |
| **Un pont par ENFANT** (une fenêtre = un lecteur) | Écartée. N lecteurs pour un utilisateur contredit le cadrage ; N racines de virtualisation sur le même profil ; et surtout **le lecteur mourrait avec la fenêtre qui l'a ouvert**, ce que §3.3 examine |
| **Dans le SUPERVISEUR** | Écartée. Le superviseur est le seul processus dont la mort emporte tout (`superviseur/lanceur.rs:95-106` : job object `KILL_ON_JOB_CLOSE`). Y loger des rappels ProjFS — qui s'exécutent sur des fils que **le système** possède, où une panique Rust devient un `abort` de processus — reviendrait à faire du pont fichiers un risque pour la capture entière. C'est la faute de l'ancien pont, transposée |
| **Dans le CAPTEUR** | Écartée pour la même raison, aggravée : le capteur porte les huit encodeurs de tout le produit depuis D4 |
| **Un TROISIÈME type de processus, `PONT=1`** | **Retenue** |

**Le pont est donc un processus `agent.exe` de plus**, lancé avec `PONT=1`,
rattaché au même job object que le capteur et les enfants, et surveillé
exactement comme le capteur l'est déjà.

**Le mécanisme existe et n'est pas à écrire.**
`superviseur/lanceur.rs:156-188` (`lancer_capteur`) pose `CAPTEUR=1`, retire
`SUPERVISEUR`, `TEST_FILE` et `WINDOW_TITLE` de l'environnement de l'enfant, et
l'attache au job (`:174`). `superviseur/boucle/surveillance_capteur.rs` (148
lignes) le relance à sa mort (`:120-127`, trace `capteur mort, relancé`, les
relances suivantes silencieuses). **Un `lancer_pont` et une
`surveillance_pont` en sont la transposition littérale**, et le §9 leur donne
leur budget de lignes.

⚠️ **Le piège de symétrie d'environnement est nommé d'avance, parce que ce
dépôt l'a payé** : un processus `PONT=1` qui hériterait de `CAPTEUR` se
prendrait pour un capteur (`agent/src/main.rs:315` teste `CAPTEUR` **avant**
`SUPERVISEUR`), et un capteur qui hériterait de `PONT` ne serait jamais
devenu capteur. `lancer_pont` doit donc `env_remove("CAPTEUR")` et
`env_remove("SUPERVISEUR")`, et `lancer_capteur`/le lanceur d'enfants doivent
gagner `env_remove("PONT")`. La convention de valeur est celle du dépôt et
d'aucune autre : **`PONT=0` DÉSACTIVE, et une simple présence n'active pas**
(même forme que `agent/src/main.rs:315` pour `CAPTEUR`, et pour la même raison
d'exploitation — sans quoi écrire `PONT=0` pour couper le pont l'allumerait).

⚠️ **`scripts/run-agent.sh` doit transmettre `PONT` explicitement.** Ce piège a
été payé en D1 (`SUPERVISEUR`), en D2 (`MULTIFENETRE_REPRISE`) et évité de
justesse en D6 (`BUDGET_BPS`, par une tâche dédiée à cette seule ligne). Il
vaut pour les variables de **produit** comme pour celles de banc. **Une tâche
dédiée, avant celle qui en a besoin.**

**Coût de D2, écrit** : un processus de plus (≈ 8 Mo de binaire déjà présent
sur disque, un espace d'adressage de plus), une connexion de signaling de plus,
et une seconde `RTCPeerConnection` par utilisateur. En échange, le principe 4
du cadrage — « une panne du canal fichiers ne touche jamais le flux vidéo » —
cesse d'être une propriété à défendre à chaque revue et devient une propriété
de l'ordonnancement du système.

**Ce que D2 ne rend PAS étanche, et qu'il ne faut pas surestimer** : le pont
partage avec le reste le CPU de la VM, sa RAM, son disque (la racine de
virtualisation grossit à mesure que des fichiers sont hydratés — §6.4) et son
lien réseau (le budget de débit de D6, `BUDGET_BPS`, ne connaît pas ce
trafic-ci — §10, risque R5). Et **un rappel ProjFS qui ne se termine jamais
fige l'APPLICATION qui lit le fichier**, pas la vidéo : le flux continue de
couler, la fenêtre montre une application gelée. §5.3 en fait une contrainte
de délai, parce que c'est la seule réponse.

### 3.3 D3 — qui détient le répertoire

`showDirectoryPicker()` rend un `FileSystemDirectoryHandle` qui appartient à
**un document**. Le produit a N fenêtres navigateur, plus la page-shell
(`client/src/shell.ts`).

**La page-shell le détient, et elle seule.**

- **Elle est déjà la page unique par utilisateur.** Son en-tête le dit et en
  donne la raison (`client/src/shell.ts:1-6`) : « c'est elle qui ouvre une
  fenêtre navigateur par fenêtre Windows, et elle seule […] sans elle, fermer
  cette première page couperait la capacité d'ouvrir toutes les suivantes ».
  Le lecteur « par utilisateur » du cadrage a donc déjà sa page.
- **Elle est le seul endroit où le geste utilisateur a un sens.**
  `showDirectoryPicker()` exige une activation utilisateur transitoire ; un
  bouton « Choisir mon dossier » dans le bureau est une place naturelle, là où
  le même bouton dans une fenêtre d'application ne dirait pas de quoi il
  parle.
- **Aucune fenêtre d'application n'est spéciale**, ce qui est l'invariant que
  la page-shell existe pour tenir.

**Ce qui se passe quand une fenêtre se ferme — la question posée, répondue :**

| Ce qui se ferme | Effet sur le lecteur |
| --- | --- |
| Une **fenêtre d'application** (n'importe laquelle, toutes) | **Aucun.** Elle n'a jamais détenu le répertoire ni le canal |
| La **page-shell** | **Le lecteur se démonte.** Le canal `fichiers` se ferme, le pont appelle `PrjStopVirtualizing`, et la racine est retirée. Toute opération en vol se termine en `ERROR_IO_DEVICE` (§5) |
| Le **pont** (crash) | Le superviseur le relance ; la page-shell rétablit sa `RTCPeerConnection` et **réouvre le lecteur avec le même `FileSystemDirectoryHandle`**, qu'elle n'a jamais perdu. §6.3 traite ce qui était en cours d'écriture |
| Le **superviseur** | Tout meurt, pont compris (job object, `superviseur/lanceur.rs:95-106`) |

⚠️ **Fermer la page-shell est donc un geste destructeur**, et rien dans
l'interface actuelle ne le dit. **La page-shell doit poser un
`beforeunload`** tant qu'un lecteur est monté, et **davantage** tant qu'une
écriture n'a pas été poussée (§6.2). C'est un livrable de **F2**, pas une
recommandation.

⚠️ **Le handle n'est PAS persisté en v1.** `FileSystemDirectoryHandle` est
sérialisable en IndexedDB, mais au rechargement de la page la permission doit
être re-accordée par `requestPermission()`, **qui exige à son tour une
activation utilisateur**. Persister n'économiserait donc que la traversée de
l'arborescence dans le sélecteur, jamais le geste. Le gain ne paie pas le
stockage d'un handle sur une machine partagée : **un clic par chargement de la
page-shell**, et c'est tout.

### 3.4 D4 — le canal

**Une `RTCPeerConnection` dédiée, sans piste média, entre la page-shell et le
processus pont, portant un unique data channel `fichiers`, fiable et
ordonné.**

```
page-shell ──WS signaling(session « <id>#fichiers »)──┐
     │                                                 │
     └──RTCPeerConnection (data only) ─────────────────┴──> processus pont (VM)
                canal « fichiers » : ordered, fiable
```

**Ce qui existe déjà et se réemploie tel quel** :
`agent/src/signaling.rs:53` (`run_signaling(url, session)`) est générique — il
ne connaît que l'URL et l'identifiant de session ; et
`agent/src/transport/initialisation.rs:27` (`construire_rtc`) montre la
construction complète d'un `Rtc` str0m en **80 lignes**, dont tout ce qui est
média (`enable_h264`, `enable_opus`, `enable_bwe`, `set_stats_interval`,
`:56-70`) tombe pour un point d'accès données seul.

**Deux voies écartées, avec leur raison.**

- **Un troisième data channel sur la `RTCPeerConnection` d'un ENFANT.** C'est
  la voie la moins chère côté client — une ligne, à côté de
  `client/src/webrtc.ts:210-214`. Elle est écartée pour trois raisons dont la
  troisième suffit : (i) elle attacherait le lecteur à une fenêtre, ce que D3
  refuse ; (ii) elle ferait passer les octets des fichiers de l'utilisateur par
  la même `PeerConnection` que la vidéo, donc **par le même contrôleur de
  congestion** — un `set_desired_bitrate` (`agent/src/transport/part.rs`) et
  un budget de session `BUDGET_BPS` que D6 a réglés pour la vidéo seule ; (iii)
  **elle rendrait faux le principe 4 du cadrage** : SCTP et RTP partagent alors
  le même transport DTLS, la même file, la même estimation.
  ⚠️ **Fait de code à connaître avant toute tentative de la reprendre**, parce
  qu'il est invisible depuis le client : ❌ **CE FAIT DE CODE N'EN EST PLUS UN**
  — la tâche 17 de F1 (commit `df7fd4d`) a corrigé le défaut :
  `dispatch_channel_data` appelle désormais
  `destination(data.id, self.input_channel, self.control_channel)`, donc il
  regarde le canal. ⚠️ *Et la plage `:168-189` publiée ci-dessous n'a jamais
  désigné cette fonction — elle vivait en `:219-239` avant la tâche 17.* Texte
  d'origine, conservé pour son diagnostic :
  `agent/src/transport/evenements.rs:168-189`
  (`dispatch_channel_data`) **ne regarde jamais le label du canal** — il aiguille
  sur le seul `data.binary`, tout ce qui est binaire allant à
  `InputMessage::decode`. Un canal `fichiers` binaire ajouté à cette
  `PeerConnection` verrait ses trames décodées comme des entrées souris, et le
  seul symptôme serait un `WARN "message d'entrée invalide"` par trame. Le
  label est pourtant disponible (`evenements.rs:51-63`, `Event::ChannelOpen(id,
  label)`) : il est simplement inutilisé.
- **Le WebSocket de signaling, réemployé comme transport de données.** C'est ce
  que faisait l'ancien pont. Écartée : les octets des fichiers de l'utilisateur
  transiteraient alors **par la plateforme**, qui devient à la fois un goulot
  de débit et un dépositaire en clair de contenus qu'elle n'a aucune raison de
  voir. Un data channel SCTP/DTLS est chiffré de bout en bout entre le
  navigateur et la VM. Le cadrage §4 ne dessine d'ailleurs que des data
  channels.

**Réglages du canal, et leur raison.**

| Réglage | Valeur | Pourquoi |
| --- | --- | --- |
| `ordered` | `true` | Une réponse hors d'ordre demanderait une file de réassemblage pour rien : le débit d'un pont fichiers n'est pas contraint par la latence de tête de ligne comme l'est l'entrée |
| fiabilité | **par défaut, donc fiable** | Une plage d'octets perdue est un fichier corrompu. C'est l'exact opposé du canal `input` (`client/src/webrtc.ts:210-213`, `maxRetransmits: 0`), et la raison inverse |
| `bufferedAmountLowThreshold` | posé | Le contrôle de flux du pont s'y adosse (§7.3), plutôt que d'inonder SCTP |

### 3.5 D5 — le périmètre d'opérations v1

Le cadrage §10 borne la v1 à « read/write/list/rename ».

> ⚠️ **Cette conception AJOUTE la suppression (fichier et répertoire) à cette
> liste, et le déclare comme une addition, pas comme une lecture.**

**Pourquoi ce n'est pas un confort.** L'idiome d'enregistrement dominant des
applications Windows n'est pas « écrire sur place » : c'est **écrire un
fichier temporaire, puis renommer, puis supprimer l'ancien** (ou
`ReplaceFile`, qui fait les trois). Refuser la suppression — ce que
`PRJ_NOTIFICATION_PRE_DELETE` permet de faire proprement — casserait donc
l'enregistrement d'une grande partie des applications, c'est-à-dire **le
chemin d'écriture entier**, alors que l'écriture est explicitement dans le
périmètre. Un périmètre « write sans delete » est un périmètre qui ne
fonctionne pas.

**Le code de l'ancien pont corrobore** : `unlink` (`src/file.js:309`) et
`rmdir` (`src/file.js:297`) y sont implémentés, dans un pont dont l'inventaire
montre par ailleurs qu'il a laissé de côté tout ce qui n'était pas
indispensable (`truncate`, `chmod`, `chown`, `utimens`, `flush`, `fsync`,
`access`, les liens, les attributs étendus sont **tous absents**). La
suppression appartient au noyau irréductible ; onze opérations en tout ont été
jugées nécessaires, et elle en fait partie.

**Coût de l'addition** : un appel de plus côté navigateur
(`FileSystemDirectoryHandle.removeEntry(name, { recursive })`, **standard**,
contrairement à `FileSystemHandle.remove()` que l'ancien pont emploie à
`web/index.js:605,612,639`) et un type de message de plus. C'est tout.

#### Le périmètre v1, énoncé positivement

| Opération | Rappel / notification ProjFS | Appel navigateur |
| --- | --- | --- |
| **Lister** un répertoire | `PRJ_START_DIRECTORY_ENUMERATION_CB`, `PRJ_GET_DIRECTORY_ENUMERATION_CB`, `PRJ_END_DIRECTORY_ENUMERATION_CB` | `for await (… of dir.values())` |
| **Attributs** d'une entrée | `PRJ_GET_PLACEHOLDER_INFO_CB` | `getFileHandle`/`getDirectoryHandle`, puis `getFile()` pour `size`/`lastModified` |
| **Lire** une plage | `PRJ_GET_FILE_DATA_CB` | `file.slice(o, o+n).arrayBuffer()` |
| **Existence** (chemin nié) | `PRJ_QUERY_FILE_NAME_CB` | résolu par le cache négatif (§7.4), sinon `getFileHandle` |
| **Créer** un fichier / un répertoire | `PRJ_NOTIFICATION_NEW_FILE_CREATED` | `getFileHandle(n,{create:true})` / `getDirectoryHandle(n,{create:true})` |
| **Écrire** | `PRJ_NOTIFICATION_FILE_HANDLE_CLOSED_FILE_MODIFIED`, `…FILE_OVERWRITTEN` | `createWritable()` + `write({type:'write',position,data})` + `close()` |
| **Renommer** | `PRJ_NOTIFICATION_PRE_RENAME`, `PRJ_NOTIFICATION_FILE_RENAMED` | `handle.move(dir, nom)` si disponible, sinon copie + `removeEntry` (§3.5.1) |
| **Supprimer** | `PRJ_NOTIFICATION_PRE_DELETE`, `PRJ_NOTIFICATION_FILE_HANDLE_CLOSED_FILE_DELETED` | `dir.removeEntry(nom, { recursive })` |

#### 3.5.1 Le renommage, et le fait qu'il n'existe pas en standard

`FileSystemHandle.move()` **n'est pas dans la norme du File System Access
API** ; c'est une extension Chromium. L'ancien pont s'en sert
(`web/index.js:628` pour le récursif, `:644` pour un fichier) — et son
renommage de répertoire **ne fonctionne pas** : `web/index.js:631` déclare
`const newDir = await newDir.getDirectoryHandle(...)` à l'intérieur du bloc où
`newDir` est déjà le paramètre, ce qui lève une `ReferenceError` par zone morte
temporelle. **Le renommage d'un répertoire contenant un sous-répertoire échoue
donc systématiquement dans le produit historique**, et personne ne l'a relevé.

**Décision** : `move()` est employé **quand il existe**, détecté par
`typeof handle.move === 'function'` ; sinon le pont retombe sur
**copie plage par plage puis `removeEntry`**.
**Coût, écrit et non négligeable** : le repli n'est **pas atomique** (une
coupure au milieu laisse deux copies partielles, dont l'une porte le nom
cible), et il fait transiter **tout le contenu du fichier deux fois** sur le
canal. Renommer un fichier de 1 Gio sur le chemin de repli coûte donc 2 Gio de
canal, là où `move()` coûte un message. **Le repli est instrumenté** (une trace
`renommage par copie` avec la taille), pour que la mesure de F4 puisse dire ce
qu'il coûte réellement plutôt que de l'estimer.

#### 3.5.2 Ce qui n'est PAS couvert en v1, et ce qu'une application Windows verra

C'est la liste que le cadrage §10 appelle « le reste ensuite ». Elle est ici
**explicite, avec l'effet observable**, parce qu'un périmètre dont on ne
connaît pas les bords n'est pas un périmètre.

| Non couvert | Ce que voit l'application Windows |
| --- | --- |
| **Attributs Windows** (`readonly`, `hidden`, `system`, `archive`) | Tout est `FILE_ATTRIBUTE_NORMAL` (ou `_DIRECTORY`). `attrib +r` **échoue** en `ERROR_NOT_SUPPORTED` (0x80070032). La FSA n'expose aucun attribut ; il n'y a rien à transporter |
| **Horodatages en écriture** (`SetFileTime`) | Réussit sur le fichier hydraté **localement**, et n'est **jamais** répercuté côté navigateur. La FSA ne permet pas d'écrire `lastModified`. **Divergence silencieuse assumée** |
| **ACL NTFS, propriétaire, audit** | Une ACL héritée de la racine, identique pour tout. Toute écriture d'ACL échoue |
| **Flux de données alternatifs (ADS)** | `ERROR_NOT_SUPPORTED`. Conséquence concrète : la *Marque du Web* (`Zone.Identifier`) **ne suit pas** un fichier téléchargé — un exécutable copié depuis le poste local arrivera sans son avertissement SmartScreen. **À dire à l'utilisateur, pas à taire** |
| **Liens symboliques et durs** | `PRJ_NOTIFICATION_PRE_SET_HARDLINK` et `HARDLINK_CREATED` sont **refusés** ; `PRJ_EXT_INFO_TYPE_SYMLINK` n'est jamais émis. `ERROR_NOT_SUPPORTED` |
| **Verrous d'octets, oplocks, partage entre machines** | NTFS verrouille correctement le fichier **hydraté**, donc entre applications de la VM. **Rien ne verrouille côté poste local** : une modification faite sur le poste pendant qu'une application de la VM écrit produit une divergence que rien ne détecte. Un seul utilisateur par VM borne le risque ; il ne l'annule pas |
| **Notification de changement côté POSTE LOCAL** | La FSA n'a pas d'observateur de répertoire. Un fichier ajouté sur le poste local **n'apparaît pas** avant que le cache d'énumération n'expire ou que l'utilisateur ne rafraîchisse (F5 pose un rafraîchissement explicite, §7.4) |
| **Renommage entre RÉPERTOIRES** | **Couvert**, mais par le chemin de repli non atomique de §3.5.1 dès que `move()` manque. `move(destinationDirectory, name)` accepte un répertoire de destination différent |
| **Fichiers > `TAILLE_MAX_FICHIER`** | Lecture : **couverte** — ProjFS demande des plages, le pont les découpe (§7.3), la taille du fichier n'entre jamais en mémoire. Écriture (write-back) : **refusée** au-delà du plafond, `ERROR_DISK_FULL` (0x80070070), et le fichier reste sur le disque de la VM avec son chemin dit à l'utilisateur. ⚠️ **`TAILLE_MAX_FICHIER` n'est PAS calibrée** — elle rejoint `BPP_MIN`, `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`, `HYSTERESIS`, `REPIT_APRES_ECHEC` et `TAILLE_MAX_SORTIE` dans la liste des constantes de ce dépôt qu'aucune mesure n'a jugées |
| **Quota, espace disque annoncé** | La v1 annonce l'espace **du volume de la VM**, pas celui du poste local, qu'aucune API ne lui donne. ⚠️ L'ancien pont annonçait `1000000000` en dur (`web/index.js:517`) et `104857600` blocs (`src/file.js:173-185`) : **des chiffres inventés**. Ici le chiffre est vrai, mais il décrit **le mauvais disque** — c'est une limite, dite, pas un mensonge |
| **Corbeille** | Une suppression est définitive côté poste local. `removeEntry` ne passe par aucune corbeille |

### 3.6 D6 — la racine de virtualisation, et pas de lettre de lecteur

**ProjFS virtualise un RÉPERTOIRE, jamais un volume.** « Lecteur Mes
Fichiers » (cadrage §5 ③) est donc un nom d'usage, pas une lettre.

**Décision** : la racine est `%USERPROFILE%\Mes Fichiers`, et **aucune lettre
n'est attribuée**.

- Une lettre exigerait `subst`/`DefineDosDevice`, dont la durée de vie est
  celle d'une session d'ouverture — donc un mécanisme de plus à surveiller, à
  reposer après un redémarrage du pont, et à retirer proprement, pour **zéro
  gain fonctionnel** : le besoin réel énoncé par l'amendement du 28/07/2026 est
  que « l'utilisateur y navigue dans le lecteur ProjFS » depuis un dialogue
  `IFileOpenDialog`, et un dossier du profil s'y navigue exactement comme un
  volume.
- Le dossier étant dans le profil, il apparaît dans le volet de navigation de
  l'Explorateur et dans « Accès rapide » sans travail supplémentaire.

⚠️ **Trois contraintes de ProjFS sur cette racine, à respecter dès F1.**

1. **La racine doit être marquée une seule fois**, par
   `PrjMarkDirectoryAsPlaceholder`, avec un GUID d'instance. Re-marquer une
   racine déjà marquée échoue. Le GUID est donc **persisté**, hors de la
   racine.
2. **La racine ne doit pas contenir de données au moment du marquage.**
3. ⚠️ **Le journal de reprise (§6.3) doit vivre EN DEHORS de la racine**, dans
   `%LOCALAPPDATA%\Guacamole\pont\`. S'il vivait dedans, il serait lui-même un
   objet projeté — donc dépendant du pont pour être lu, ce qui est
   circulaire — et il disparaîtrait avec la racine le jour où il faudrait la
   recréer, c'est-à-dire exactement le jour où il sert.

---

## 4. Architecture

### 4.1 Les trois étages

```
┌── Navigateur — PAGE-SHELL (client/src/shell-page.ts) ────────────────┐
│  bouton « Choisir mon dossier » → showDirectoryPicker()              │
│  client/src/fichiers/*.ts : résolution de chemin, lecture par plage, │
│  écriture par plage, énumération, renommage, suppression             │
└──────────────────────┬───────────────────────────────────────────────┘
                       │  RTCPeerConnection DÉDIÉE — canal « fichiers »
                       │  trames binaires versionnées (proto/{src,ts}/fichiers)
┌──────────────────────┴───────────────────────────────────────────────┐
│  VM Windows — PROCESSUS PONT (agent.exe, PONT=1)                     │
│                                                                       │
│   agent/src/pont/transport.rs   signaling + str0m, DONNÉES SEULES     │
│   agent/src/pont/table.rs       PUR — commandes en vol, expiration    │
│   agent/src/pont/decoupe.rs     PUR — plages ProjFS → trames bornées  │
│   agent/src/pont/chemins.rs     PUR — normalisation des chemins       │
│   agent/src/pont/erreurs.rs     PUR — erreur → HRESULT                │
│   agent/src/pont/journal.rs     PUR — ensemble des écritures dues     │
│   agent/src/pont/projfs.rs      #[cfg(windows)] — rappels et racine   │
│   agent/src/pont/projfs/chargement.rs  #[cfg(windows)] — LoadLibraryW │
└──────────────────────┬───────────────────────────────────────────────┘
                       │  ProjectedFSLib.dll → PrjFlt.sys
              %USERPROFILE%\Mes Fichiers   (racine de virtualisation)
```

### 4.2 Le protocole, et son versionnement

**Où il vit** : `proto/src/fichiers.rs` et `proto/ts/fichiers.ts`, déclarés
dans `proto/src/lib.rs` (aujourd'hui 4 lignes, `proto/src/lib.rs:1-4`, qui
porte déjà `control` et `input`). C'est l'endroit que le cadrage §8 nomme
« schéma versionné partagé (source de vérité unique) ».

**Comment il se versionne** : `pub const FICHIERS_VERSION: u8 = 1;`, présent
dans les deux fichiers et **vérifié des deux côtés au décodage**, rejet sur
écart. Le dépôt a deux précédents, et les deux se lisent avant d'écrire le
troisième :

- `proto/src/control.rs:14` (`CONTROL_VERSION: u8 = 3`), vérifié par
  `verifie_version` (`:41-52`), avec une note explicite de `:37-40` disant
  pourquoi le champ **ne** porte **pas** de `default` — un message sans version
  doit être rejeté, pas silencieusement complété ;
- `proto/src/input.rs:14` (`PROTOCOL_VERSION: u8 = 2`), premier octet de la
  trame (`:98`), rejeté à la lecture (`:145`).

**Quelle forme** : celle d'`input`, **binaire**, pas celle de `control`.

> ⚠️ **La raison est un chiffre relevé sur l'ancien pont, pas une préférence.**
> `src/file.js:264` sérialise le buffer d'une écriture par
> `Array.from(buffer.slice(0, length))`, c'est-à-dire en **tableau JSON
> d'entiers décimaux** — de l'ordre de 4 octets transmis par octet utile. Le
> retour est pire encore : `web/index.js:653-657` `JSON.stringify` un
> `Int8Array`, ce qui produit un objet à clés numériques
> (`{"0":12,"1":-3,…}`), reconstruit côté serveur par
> `new Int8Array(Object.values(...))` (`src/file.js:133-134`). **Un protocole
> de fichiers qui encode les octets en JSON paie cet ordre de grandeur sur
> chaque octet de chaque lecture.**

**La trame**, une par message SCTP, pas de réassemblage :

```
octet 0        FICHIERS_VERSION (u8)
octet 1        type (u8)
octets 2..5    identifiant de corrélation (u32, petit-boutiste)
octets 6..9    longueur de l'en-tête JSON (u32, petit-boutiste)
octets 10..    en-tête JSON (UTF-8)  — métadonnées, chemins, erreurs
puis           charge binaire, jusqu'à la fin de la trame
```

- L'en-tête JSON porte ce qui est structuré (chemin, position, longueur,
  taille, code d'erreur) ; la charge binaire porte les octets, **jamais
  encodés**.
- L'identifiant de corrélation est un `u32` **monotone**, pas un UUID. L'ancien
  pont employait un UUID v4 par requête (`src/file.js:94`) avec **un écouteur
  `message` attaché par requête** (`src/file.js:155`) : l'inventaire relève que
  sur le chemin d'erreur, `client.off` n'est jamais appelé — chaque opération
  en échec laisse un écouteur à vie, et tous les écouteurs survivants
  re-analysent chaque message suivant. **Une table indexée par entier, une
  seule, purgée par expiration** (`agent/src/pont/table.rs`, pur, testé) est la
  réponse.
- **Taille maximale d'une trame** : `TAILLE_TRAME_MAX = 64 Kio` de charge
  binaire. Valeur choisie sous le plancher d'interopérabilité usuel des
  messages SCTP (256 Kio) plutôt que mesurée. ⚠️ **Non calibrée**, et sa
  documentation le dira ; F4 est ce qui pourra la juger.

**Types de messages v1** (l'énumération exacte est du ressort de
l'implémentation ; ce qui est tranché est qu'il y en a **deux familles**) :
requêtes pont → navigateur (`Lister`, `Attributs`, `Lire`, `Ecrire`, `Creer`,
`Renommer`, `Supprimer`, `Tronquer`) et réponses navigateur → pont
(`Donnees`, `Entrees`, `Fait`, `Echec { code }`). **Le sens est unique** : le
pont demande, le navigateur répond. Une seule exception, et elle est nécessaire :
`Rafraichir`, poussé par le navigateur, qui vide les caches d'énumération et le
cache négatif (§7.4) — c'est le seul moyen qu'a l'utilisateur de faire
apparaître dans la VM un fichier qu'il vient d'ajouter sur son poste.

### 4.3 Les entrées ProjFS employées, et la correspondance des rappels

**Treize entrées** résolues par `GetProcAddress` (§3.1), toutes **relevées**
présentes dans
`windows-0.62.2/src/Windows/Win32/Storage/ProjectedFileSystem/mod.rs` :

`PrjMarkDirectoryAsPlaceholder`, `PrjStartVirtualizing`, `PrjStopVirtualizing`,
`PrjWritePlaceholderInfo`, `PrjFillDirEntryBuffer`, `PrjWriteFileData`,
`PrjAllocateAlignedBuffer`, `PrjFreeAlignedBuffer`, `PrjCompleteCommand`,
`PrjFileNameMatch`, `PrjFileNameCompare`, `PrjClearNegativePathCache`,
`PrjDeleteFile`.

⚠️ **`PrjWritePlaceholderInfo2` et `PrjFillDirEntryBuffer2` ne sont PAS
employées**, bien qu'elles figurent dans le binding : elles sont apparues
après la première génération de l'API, et le §2.2 dit que leur présence sur
cette machine n'a pas été vérifiée. La v1 n'a besoin d'aucune des deux (elles
servent les liens symboliques, hors périmètre §3.5.2).

#### Le rappel ne bloque JAMAIS — c'est la décision de conception centrale

Chaque rappel ProjFS s'exécute sur un fil que **le système** possède, à
l'intérieur d'une opération de fichier d'une application. Y attendre un
aller-retour navigateur (des dizaines de millisecondes en local, davantage sur
un lien réel) figerait cette application pour toute la durée.

**Donc : tout rappel rend immédiatement
`HRESULT_FROM_WIN32(ERROR_IO_PENDING)` (0x800703E5), inscrit la commande dans
`pont/table.rs` avec son `CommandId`, et la réponse du navigateur est complétée
plus tard par `PrjCompleteCommand`.** Conséquences, toutes obligatoires :

- **`PRJ_CANCEL_COMMAND_CB` doit être implémenté.** Il n'est optionnel que pour
  un fournisseur synchrone. Ici, une application qui abandonne son E/S nous
  laisse une commande orpheline dans la table.
- **L'énumération asynchrone se complète avec des paramètres étendus** :
  `PRJ_COMPLETE_COMMAND_EXTENDED_PARAMETERS` avec
  `CommandType = PRJ_COMPLETE_COMMAND_TYPE_ENUMERATION` et le
  `DirEntryBufferHandle` du rappel (les trois symboles sont **relevés** dans le
  module).
- **`PrjWriteFileData` exige un tampon aligné**, obtenu par
  `PrjAllocateAlignedBuffer` et rendu par `PrjFreeAlignedBuffer`. Ce couple est
  la première source de fuite mémoire d'un fournisseur ProjFS ; il est encapsulé
  dans un garde `Drop` en Rust, jamais appelé à la main.
- ⚠️ **Une panique Rust dans un `extern "system" fn` est un abandon de
  processus.** Chaque rappel enveloppe son corps dans
  `std::panic::catch_unwind` et rend `E_UNEXPECTED` plutôt que de laisser la
  panique traverser la frontière FFI. C'est aussi ce qui rend D2 (le processus
  séparé) utile plutôt que décoratif : le pire cas est la mort du pont, pas
  celle de la capture.

#### Correspondance

| Rappel / notification | Requête émise | Complétion |
| --- | --- | --- |
| `StartDirectoryEnumeration` | *(aucune)* — ouvre une session d'énumération dans la table | synchrone, `S_OK` |
| `GetDirectoryEnumeration` | `Lister { chemin }` puis `PrjFillDirEntryBuffer` par entrée, dans l'ordre de `PrjFileNameCompare` | `ERROR_IO_PENDING` → `PrjCompleteCommand` (paramètres étendus) |
| `EndDirectoryEnumeration` | *(aucune)* | synchrone, `S_OK` |
| `GetPlaceholderInfo` | `Attributs { chemin }` → `PrjWritePlaceholderInfo` | `ERROR_IO_PENDING` → `PrjCompleteCommand` |
| `GetFileData` | `Lire { chemin, position, longueur }`, **découpée** par `pont/decoupe.rs` en trames de `TAILLE_TRAME_MAX` → `PrjWriteFileData` par morceau | `ERROR_IO_PENDING` → `PrjCompleteCommand` après le dernier morceau |
| `QueryFileName` | `Attributs`, si le cache négatif ne tranche pas | `ERROR_IO_PENDING` ou `ERROR_FILE_NOT_FOUND` |
| `Notification` / `NEW_FILE_CREATED` | `Creer { chemin, repertoire }` | poussée, non attendue |
| `Notification` / `FILE_HANDLE_CLOSED_FILE_MODIFIED`, `FILE_OVERWRITTEN` | inscription au journal (§6.3), puis `Ecrire` par plages | poussée |
| `Notification` / `PRE_RENAME` | *(aucune)* — accepte, sauf si la cible sort de la racine | synchrone |
| `Notification` / `FILE_RENAMED` | `Renommer { de, vers }` | poussée |
| `Notification` / `PRE_DELETE` | *(aucune)* — accepte | synchrone |
| `Notification` / `FILE_HANDLE_CLOSED_FILE_DELETED` | `Supprimer { chemin }` | poussée |
| `Notification` / `PRE_SET_HARDLINK`, `HARDLINK_CREATED` | *(aucune)* | **refusé**, `ERROR_NOT_SUPPORTED` |
| `CancelCommand` | *(aucune)* | retire la commande de la table ; la réponse tardive est jetée |

⚠️ **`PRE_RENAME`, `PRE_DELETE` et `PRE_SET_HARDLINK` sont SYNCHRONES et ne
peuvent pas être reportées** : leur seul rôle est d'autoriser ou de refuser, et
la décision doit se prendre sans quitter le fil. Elles ne consultent donc
**jamais** le navigateur — c'est ce qui impose qu'aucune règle d'autorisation
ne dépende d'un état distant.

### 4.4 Ce qui est PUR, et ce qui est `#[cfg(windows)]`

Doctrine constante du dépôt : la logique se teste sur l'hôte Linux, le reste
est vérifié par `cargo check --target x86_64-pc-windows-gnu`.

| Module | Nature | Ce qu'il porte, et ce que ses tests d'hôte doivent voir rouge |
| --- | --- | --- |
| `proto/src/fichiers.rs` | **PUR** | Encodage/décodage de trame, `FICHIERS_VERSION`. Rouge : une version absente ou différente est **acceptée** ; un en-tête JSON dont la longueur déborde la trame ne provoque pas d'erreur |
| `agent/src/pont/table.rs` | **PUR** | Table des commandes en vol, corrélation, expiration, annulation. Rouge : une réponse arrivée après annulation est appliquée ; une commande expirée reste en table |
| `agent/src/pont/decoupe.rs` | **PUR** | Découpe d'une plage `(offset, length)` en trames bornées. Rouge : une plage de taille exactement `TAILLE_TRAME_MAX`, ou de taille nulle, ou débordant la fin du fichier, rend un découpage dont la somme ne fait pas la longueur demandée |
| `agent/src/pont/chemins.rs` | **PUR** | Normalisation ProjFS → chemin logique : contre-obliques, refus des `..`, des `:` (ADS), des noms réservés (`CON`, `PRN`, `NUL`…), de la casse. Rouge : `a\..\..\secret` sort de la racine |
| `agent/src/pont/erreurs.rs` | **PUR** | Table `Erreur → HRESULT` (§5). Rouge : deux causes distinctes rendent le même code, ou un code de succès est rendu pour un échec |
| `agent/src/pont/journal.rs` | **PUR** | Ensemble des écritures dues, sérialisation, relecture. Rouge : un journal tronqué en cours d'écriture fait perdre les entrées **antérieures** |
| `agent/src/pont/transport.rs` | mixte, **sans Windows** | signaling + str0m données seules. Testable sur l'hôte comme `transport/` l'est déjà |
| `agent/src/pont/projfs.rs` + `projfs/chargement.rs` | **`#[cfg(windows)]`** | Les rappels, la racine, `LoadLibraryW`. **Aucun test d'hôte n'est possible** ; c'est déclaré, pas contourné |
| `client/src/fichiers/*.ts` | **PUR sauf l'adaptateur FSA** | La logique de résolution de chemin, de découpe et de corrélation est séparée du `FileSystemDirectoryHandle` et testée par Vitest avec un faux système de fichiers en mémoire |

⚠️ **La part `#[cfg(windows)]` de ce sous-projet est celle qui porte le plus
de risque, et elle est la moins couvrable.** C'est vrai de tout ce dépôt ; ce
qui est propre à ce sous-projet, c'est que le code non testable est **appelé
par le système sur ses propres fils**, pas par notre boucle. La seule
compensation est de rendre `projfs.rs` aussi mince que possible : il traduit,
il ne décide pas. Toute décision qui pourrait vivre dans un module pur doit y
vivre — c'est un critère de revue, pas un souhait.

---

## 5. Sémantique d'erreur

Le cadrage §7 exige « erreurs I/O propres côté Windows ; vidéo intacte ».

**Les valeurs numériques ci-dessous sont relevées** dans
`windows-0.62.2/src/Windows/Win32/Foundation/mod.rs` ; le HRESULT est
`HRESULT_FROM_WIN32(x) = 0x8007_0000 | x`.

| Cause | Rendu au rappel ProjFS | Ce que l'application obtient |
| --- | --- | --- |
| Entrée absente côté navigateur | `ERROR_FILE_NOT_FOUND` (2) → **0x80070002** | « fichier introuvable », le cas nominal d'un chemin qui n'existe pas |
| Répertoire parent absent | `ERROR_PATH_NOT_FOUND` (3) → **0x80070003** | idem, sur le chemin |
| Permission FSA révoquée, ou opération hors périmètre autorisé | `ERROR_ACCESS_DENIED` (5) → **0x80070005** | « accès refusé » |
| **Canal `fichiers` fermé, ou pont sans page-shell** | `ERROR_IO_DEVICE` (1117) → **0x8007045D** | **une erreur d'E/S de périphérique** — c'est la « erreur I/O standard » du cadrage §7, et c'est la classe d'erreur que les applications Windows savent déjà traiter |
| Délai dépassé (§5.3) | `ERROR_SEM_TIMEOUT` (121) → **0x80070079** | E/S expirée |
| Opération abandonnée (`CancelCommand`) | `ERROR_OPERATION_ABORTED` (995) → **0x800703E3** | annulation, silencieuse pour l'application |
| Write-back refusé : plafond de taille, ou disque du poste local plein | `ERROR_DISK_FULL` (112) → **0x80070070** | « disque plein » |
| Lien dur, lien symbolique, ADS, attribut non transportable | `ERROR_NOT_SUPPORTED` (50) → **0x80070032** | « non pris en charge » |
| Répertoire non vide à la suppression | `ERROR_DIR_NOT_EMPTY` (145) → **0x80070091** | |
| Création d'une entrée existante | `ERROR_FILE_EXISTS` (80) → **0x80070050** | |
| Racine ouverte en lecture seule (permission `read` accordée seule) | `ERROR_WRITE_PROTECT` (19) → **0x80070013** | « protégé en écriture » — F1 vit tout entier dans cet état |
| Panique interceptée dans un rappel | `E_UNEXPECTED` | dernier recours, et c'est un défaut à corriger, pas un état nominal |

### 5.1 La règle qui gouverne cette table

**Deux causes distinctes ne partagent jamais un code.** L'ancien pont rendait
`cb(-1)` — soit `EPERM` — pour **tout** : chemin absent, timeout, WebSocket
mort, erreur du navigateur (`src/file.js:189,203,245,267,277,288,299,311,323`,
neuf sites, un seul code). Un utilisateur dont le pont était coupé lisait
« opération non permise ». C'est indiagnosticable, et c'est le contre-exemple
qui fonde cette table.

**Corollaire, tout aussi important** : le message d'erreur du navigateur ne
doit pas se perdre en route. `web/index.js:669` fait
`JSON.stringify(e)` d'une `Error`, ce qui rend `"{}"` — puis
`src/file.js:127` construit `new Error("{}")`. **Toute la cause était
détruite à l'émission.** Le protocole v1 transporte donc un **code
d'énumération** (`Echec { code }`), jamais une chaîne libre : le code décide du
HRESULT, et une chaîne facultative n'est là que pour le journal.

### 5.2 « Vidéo intacte » — ce qui est vrai, et ce qui ne l'est pas

**Vrai, et structurel** : le flux vidéo est porté par les
`RTCPeerConnection` des enfants, dans d'autres processus. Une panne du canal
`fichiers` — coupure, pont mort, page-shell fermée — ne touche ni leur socket,
ni leur encodeur, ni leur contrôleur de congestion.

⚠️ **Faux si on l'étend à l'application** : un rappel ProjFS qui ne se termine
pas fige le fil de l'application qui lisait le fichier. Le flux continue de
couler et la fenêtre montre une application gelée — *les images arrivent, le
contenu ne bouge plus*. « Vidéo intacte » se vérifie sur `framesDecoded`, et ce
compteur ne dit rien de ce que l'utilisateur voit. **La seule réponse est le
§5.3.**

### 5.3 Les délais, et le fait qu'ils ne sont pas calibrés

Toute commande inscrite dans `pont/table.rs` porte une échéance ; à son terme
elle est complétée par `ERROR_SEM_TIMEOUT` et retirée. **Trois budgets
distincts**, parce qu'une lecture de plage et une énumération d'un répertoire
de dix mille entrées n'ont pas la même durée légitime :

| Commande | Budget proposé | Statut |
| --- | --- | --- |
| `Attributs`, `QueryFileName` | 2 s | **non calibré** |
| `Lire` (une trame) | 5 s | **non calibré** |
| `Lister` (une énumération complète) | 20 s | **non calibré** |

⚠️ **Aucun de ces trois nombres n'est mesuré ; ils sont posés.** L'ancien pont
en avait **un seul, 10 s, pour tout** (`src/file.js:89`), non configurable et
jamais différencié — d'où des lectures de gros blocs qui expiraient avant
d'avoir eu le temps d'aboutir, et des `getattr` qui figeaient l'Explorateur dix
secondes pour un chemin inexistant. **C'est F4 qui donnera de quoi les
calibrer**, et jusque-là leur documentation dira qu'ils ne le sont pas.

⚠️ **Un délai n'est pas une politique de reprise.** Une commande expirée n'est
**jamais** rejouée automatiquement : l'application a déjà reçu son erreur, et
rejouer produirait une seconde écriture sans lecteur. La reprise, quand elle
existe, est celle du journal (§6.3), et elle est explicite.

---

## 6. Le write-back, et la perte de données regardée en face

### 6.1 Le cache write-back n'est pas un choix : c'est le fonctionnement de ProjFS

Le cadrage §5 ③ parle d'un « cache write-back flushé avant fin de session ».
**Ce n'est pas une optimisation qu'on pourrait retirer.** ProjFS *projette* :
quand une application ouvre un fichier en écriture, le pilote **hydrate** le
fichier — il en fait un fichier NTFS complet sur le disque de la VM, à
l'intérieur de la racine — et l'écriture va **sur ce fichier local**. Le
fournisseur n'est pas sur le chemin de l'écriture ; il est **notifié après
coup**, à la fermeture du handle
(`PRJ_NOTIFICATION_FILE_HANDLE_CLOSED_FILE_MODIFIED`).

**Il existe donc, par construction, un intervalle pendant lequel les octets
écrits vivent uniquement sur le disque de la VM.** Aucune conception ne peut le
supprimer ; on peut seulement le raccourcir, le rendre observable, et le
rendre récupérable.

### 6.2 Décision D7 — pousser à la fermeture du handle, pas à la fin de session

**« Flushé avant fin de session » est refusé comme politique**, et remplacé par
**« poussé dès que le handle se ferme »**.

- Un « flush de fin de session » ferait porter à un seul instant — celui où
  tout se démonte, où le navigateur est peut-être déjà parti — la totalité du
  travail d'écriture. C'est le pire moment possible.
- La fermeture du handle est le premier instant où la donnée est **cohérente**
  (l'application a fini d'écrire) et où le fournisseur est **prévenu**. Il n'y a
  pas d'instant plus tôt.
- **Effet** : dans le cas nominal, la fenêtre de perte va de la fermeture du
  handle à l'acquittement du navigateur, soit le temps d'un aller-retour et
  d'une écriture FSA. Pas la durée d'une session.

**Ce que la page-shell doit faire, et qui est un livrable de F2** :

- afficher, en permanence, le nombre d'écritures **dues** (non acquittées) ;
- poser un `beforeunload` tant qu'il est non nul, avec un texte qui nomme les
  fichiers ;
- refuser de se dire « fermée proprement » tant que le journal n'est pas vide.

### 6.3 Le journal de reprise

`agent/src/pont/journal.rs` (**pur, testé sur l'hôte**) tient l'ensemble des
chemins écrits dont le navigateur n'a pas acquitté la réception, **persisté
dans `%LOCALAPPDATA%\Guacamole\pont\` — hors de la racine** (§3.6).

Il est relu au démarrage du pont. S'il n'est pas vide :

1. le pont annonce à la page-shell la liste des écritures dues ;
2. chacune est repoussée, dans l'ordre d'inscription ;
3. une entrée poussée avec succès est retirée du journal, jamais avant.

Cela couvre : la mort du pont, la mort de la page-shell suivie d'un
rechargement, et le redémarrage de la VM — **à la condition que le fichier
hydraté soit toujours dans la racine**, ce qui est le cas tant que la racine
n'est pas recréée.

### 6.4 Ce qui reste perdu, et pourquoi cela ne se referme pas

⚠️ **Trois cas de perte subsistent. Aucun n'est masqué.**

1. **L'utilisateur ne revient jamais.** Le journal survit sur le disque de la
   VM, et les octets aussi ; mais si l'utilisateur ne rouvre plus jamais une
   page-shell **avec le même répertoire local**, ils n'ont aucun chemin pour en
   sortir. La VM finira par être détruite ou réinitialisée par la plateforme.
   **La donnée est alors perdue** — elle a existé, elle était complète, elle
   n'est jamais arrivée.
2. **L'utilisateur revient et choisit un AUTRE répertoire.** Le journal
   contient des chemins relatifs à une racine logique qui n'a plus le même
   contenu. **Décision : le pont ne pousse rien** dans ce cas ; il présente la
   liste et propose un enregistrement explicite. Rejouer aveuglément écrirait
   les fichiers d'une session dans le dossier d'une autre.
   ⚠️ **Le pont ne peut PAS détecter avec certitude qu'il s'agit du même
   répertoire** : la FSA ne donne aucun identifiant stable de répertoire
   (`isSameEntry()` compare deux handles vivants, pas un handle à un souvenir).
   La v1 compare donc le **nom** de la racine, ce qui est un indice et non une
   preuve, et **demande à l'utilisateur** dès qu'il y a le moindre doute.
3. **La racine doit être recréée** (marquage corrompu, GUID d'instance perdu,
   §3.6). Les fichiers hydratés partent avec elle. **Le journal survit** — il
   est ailleurs — et il nomme donc précisément ce qui a été perdu. **Savoir ce
   qu'on a perdu n'est pas l'avoir**, et il faut le dire ainsi à l'utilisateur.

⚠️ **Un quatrième effet, qui n'est pas une perte mais qui mord** : chaque
fichier lu est **hydraté**, donc écrit en entier sur le disque de la VM, et
**il y reste**. Un utilisateur qui parcourt 40 Gio de vidéos remplit le disque
de sa VM. `PrjDeleteFile` permet de ramener une entrée hydratée à l'état de
substitut, ce qui rend l'espace — mais **quand ?** La v1 n'en fait rien : elle
mesure l'occupation, la journalise, et **laisse le sujet ouvert** (§10, R4).
Poser une politique d'éviction sans mesure serait exactement le geste que ce
dépôt reproche à ses constantes non calibrées.

---

## 7. Le chemin nominal, et ses points délicats

### 7.1 La forme générale d'une opération

```
application Windows
   └─> PrjFlt.sys ──> rappel dans pont/projfs.rs   (fil du système)
          rend ERROR_IO_PENDING, inscrit dans pont/table.rs
                 └─> trame sur le canal « fichiers »
                        └─> page-shell : appel FSA
                        <── réponse (JSON + octets)
          <── PrjCompleteCommand(commandId, hr)      (fil du pont)
```

**Un seul fil du pont complète les commandes**, jamais un fil de rappel : les
rappels n'écrivent que dans la table, sous verrou, et rendent. C'est ce qui
rend `pont/table.rs` **pur et testable** — la concurrence y est une propriété
de la table, pas du système.

### 7.2 L'énumération, et les deux pièges de ProjFS

- **L'ordre est imposé.** ProjFS exige que les entrées soient remplies dans
  l'ordre de `PrjFileNameCompare`, qui n'est **pas** l'ordre lexicographique
  d'`OsStr` ni celui d'`Ordering::cmp` : c'est la comparaison de noms de
  fichiers de Windows. `dir.values()` de la FSA ne garantit **aucun** ordre.
  Le pont **trie donc lui-même**, par `PrjFileNameCompare`, avant de remplir.
- **Le filtre `searchExpression` est facultatif et il est fourni.** Le rappel
  `GetDirectoryEnumeration` peut recevoir une expression de recherche, à
  appliquer avec `PrjFileNameMatch`. **L'ignorer est une faute silencieuse** :
  un `dir /b *.txt` rendrait tout. C'est pour cela que `PrjFileNameMatch` et
  `PrjFileNameCompare` figurent dans les treize entrées chargées (§4.3), bien
  qu'aucune ne serve au chemin le plus simple.
- **`PRJ_CB_DATA_FLAG_ENUM_RESTART_SCAN`** doit être honoré : il redémarre
  l'énumération en cours. La session d'énumération vit dans `pont/table.rs`,
  indexée par le GUID d'énumération du rappel — pas par le chemin, qui n'est pas
  unique quand deux applications listent le même répertoire en même temps.

### 7.3 La lecture, la découpe, et le contrôle de flux

`GetFileData` peut demander **le fichier entier** en un rappel. Trois règles :

1. **`pont/decoupe.rs` (pur) découpe** `(offset, length)` en morceaux de
   `TAILLE_TRAME_MAX` au plus. C'est le module où vivent les erreurs d'unité, et
   c'est pour cela qu'il est pur et testé isolément (§4.4).
2. **Chaque morceau est écrit par `PrjWriteFileData` dès son arrivée**, dans un
   tampon obtenu par `PrjAllocateAlignedBuffer`. Le fichier entier **n'entre
   jamais en mémoire**.
   ⚠️ C'est exactement l'inverse de l'ancien pont, dont chaque lecture faisait
   `getFile()` puis `arrayBuffer()` puis `.slice(...)`
   (`web/index.js:562-564`) : une lecture séquentielle d'un fichier de 100 Mio
   par blocs de 128 Kio y relisait **100 Mio depuis le disque, huit cents
   fois**.
3. **Le contrôle de flux s'adosse à `bufferedAmount`** du data channel : le
   pont ne demande pas le morceau *n+1* tant que le canal a plus de
   `SEUIL_TAMPON` octets en attente. Sans cela, une lecture de gros fichier
   remplit la file SCTP, et c'est le canal lui-même qui devient la source de
   latence de toutes les autres opérations.

### 7.4 Les deux caches, et le seul qui soit dangereux

**Le cache négatif est activé** : `PRJ_FLAG_USE_NEGATIVE_PATH_CACHE` au
démarrage de la virtualisation. Windows sonde en permanence des chemins qui
n'existent pas — `desktop.ini`, `Thumbs.db`, `folder.jpg`, les manifestes
d'application — et **chacun de ces sondages serait sinon un aller-retour
navigateur**. C'est la première mesure d'économie de latence du pont, et elle
sert directement le critère de réexamen de l'amendement du 28/07/2026 : le
listage d'un dossier volumineux dans un dialogue Windows en dépend
directement.

**Un cache d'énumération** garde le résultat d'un `Lister` pendant
`TTL_ENUMERATION`, pour qu'une application qui liste puis interroge chaque
entrée (le comportement de l'Explorateur) ne paie qu'un aller-retour.
⚠️ **`TTL_ENUMERATION` n'est pas calibrée.**

**Aucun cache de DONNÉES.** C'est une décision, et c'est celle qui distingue ce
pont de l'ancien : `src/file.js:232-241` servait des octets depuis un cache
**sans aucun TTL**, peuplé uniquement quand `position === 0` et invalidé
seulement par une écriture passant par ce même pont (`:270`). Une modification
faite sur le poste local n'était donc **jamais** vue, pour toujours. Ici,
l'hydratation ProjFS **est** le cache de données, elle vit sur le disque, et
c'est le pilote qui en tient la cohérence.

**`Rafraichir`, poussé par le navigateur**, vide le cache d'énumération et
appelle `PrjClearNegativePathCache`. C'est le seul moyen de faire apparaître
dans la VM un fichier ajouté sur le poste local, puisque la FSA n'offre aucun
observateur de répertoire (§3.5.2). Un bouton dans la page-shell, et rien de
plus : deviner le bon instant serait inventer une politique sans mesure.

---

## 8. Découpage en sous-blocs

Chaque sous-bloc énonce ce qu'il livre, son critère de réception, **et l'état
qui le rend ROUGE** — ce dernier point n'est pas une formule. Doctrine du
dépôt, payée trois fois (F1 de D7, la sonde P1 de D8, le confondeur de D9) :
**un contrôle qu'on n'a jamais vu rouge n'est pas un contrôle.** Pour chacun,
la recette doit **provoquer délibérément** l'état à dénoncer et vérifier qu'il
est dénoncé.

**Deux exécutions par critère, pas une.** **Aucun taux ne sera revendiqué.**

### F0 — le préalable (aucun code)

**Livre** : ProjFS activé sur la VM (§2.1), et la ligne correspondante inscrite
dans le provisionnement.
**Reçu si** : `Get-WindowsOptionalFeature -Online -FeatureName Client-ProjFS`
rend `State : Enabled` **et** `Test-Path C:\Windows\System32\ProjectedFSLib.dll`
rend `True`.
**ROUGE aujourd'hui** : les deux valeurs relevées au §2 sont `Disabled` et
`False`. **L'état rouge est donc l'état actuel, constaté, pas supposé.**

### F1 — la tranche verticale minimale : un lecteur en LECTURE SEULE

**Livre** :
- `proto/{src,ts}/fichiers` v1, avec `Lister`, `Attributs`, `Lire` et le
  versionnement ;
- le processus `PONT=1`, son lancement et sa surveillance par le superviseur ;
- la `RTCPeerConnection` dédiée et le canal `fichiers` ;
- les **cinq rappels obligatoires** de ProjFS *(⚠️ « en mode asynchrone » est
  faux, et la table du §4.3 de cette même spec le dit : `StartDirectoryEnumeration`
  et `EndDirectoryEnumeration` sont SYNCHRONES. Livré : cinq implémentés,
  **trois** asynchrones)*, plus
  `QueryFileName` et `CancelCommand` ;
- le bouton de la page-shell, `showDirectoryPicker({ mode: 'read' })`.

**Toute tentative d'écriture rend `ERROR_WRITE_PROTECT` (0x80070013).** C'est
un périmètre, pas une lacune.

> ❌ **CETTE PHRASE EST FAUSSE POUR UN FICHIER CRÉÉ DE TOUTES PIÈCES, et la
> recette de F1 l'a mesuré** (20 août 2026 ; `mesure-exec2.txt` et
> `mesure-exec5.txt`, les **deux** exécutions dont la mesure VM a atteint cette
> phase — `mesure-exec1.txt` s'est arrêtée avant) : créer un fichier dans la
> racine depuis la VM **RÉUSSIT**, et le journal porte le `warn!` prévu pour ce
> cas. **Le code n'a pas dévié — c'est la spec qui promettait trop** :
> `agent/src/pont/notifications.rs` documente que `NEW_FILE_CREATED` est une
> notification **POST**, donc **irrefusable**, et que seuls les trois chemins
> `PRE_` (convert-to-full, rename, delete) sont refusés. Un fichier neuf n'en
> traverse aucun. **Le §3.5 et le §6 ne sont pas touchés** : la formulation
> juste est « toute tentative d'écrire dans un fichier PROJETÉ rend
> `ERROR_WRITE_PROTECT` ; un fichier créé de toutes pièces vit sur la VM et
> n'est jamais poussé ». Refermer ce cas est du ressort de **F2**.

**Reçu si**, sur la VM, avec un répertoire du poste local contenant au moins un
sous-répertoire et un fichier de plus de 10 Mio :
1. `%USERPROFILE%\Mes Fichiers` ouvert dans l'Explorateur montre **exactement**
   l'arborescence choisie côté navigateur, aux deux niveaux ;
2. le fichier de plus de 10 Mio, copié depuis la racine vers `C:\`, a **le même
   condensat SHA-256** que l'original côté poste local ;
3. aucune opération de l'Explorateur ne dépasse les budgets du §5.3 ;
4. **une session vidéo tourne pendant toute la recette et ne perd pas une
   image** — `framesDecoded` croît de façon monotone dans la page
   d'application, et `agent.log` ne porte aucune `clôture de session amorcée`.

**ROUGE** : l'arborescence est vide ou tronquée ; un seul octet du condensat
diffère ; l'Explorateur se fige au-delà du budget ; ou la session vidéo
décroche.
**L'état rouge est atteignable et sera provoqué** : (i) en tuant le processus
pont en pleine lecture — l'Explorateur doit rendre une erreur d'E/S en moins de
`5 s`, jamais se figer, et la vidéo doit continuer ; (ii) en fermant la
page-shell pendant une copie ; (iii) en démarrant le pont **avant** que le
répertoire n'ait été choisi.

> ⚠️ **DE CES TROIS ROUGES, SEUL (i) A ÉTÉ PROVOQUÉ** (deux exécutions, 20 août
> 2026) ; (ii) et (iii) **ne l'ont pas été**, et le document de résultats le
> déclare.
>
> ❌ **Et (iii), tel qu'il est écrit ici, décrit un état que F1 ne peut pas
> atteindre par ce chemin** : « la racine existe et toute lecture rend
> `ERROR_IO_DEVICE` » suppose que le pont monte sa racine à son démarrage. Il
> ne le fait pas — `agent/src/pont.rs::executer` n'appelle
> `Virtualisation::demarrer` qu'**après** avoir accepté l'offre SDP de la
> page-shell, donc après que le répertoire a été choisi. Relevé de recette
> concordant : `racine presente apres nettoyage : False` avant chaque
> exécution, et l'exécution « sans ProjFS » (DLL renommée) ne fait apparaître
> **aucune** racine. Le seul chemin qui produirait cet état est une racine
> **survivante** d'une exécution précédente tuée brutalement — cas que
> `pont.rs` nomme déjà et que F1 ne referme pas.

⚠️ **Le critère (2) — le condensat — est le seul qui ne puisse pas être satisfait
par accident.** Un critère qui se contenterait de « le fichier s'ouvre » serait
vérifié par un fichier tronqué, par un fichier dont les plages sont dans le
désordre, et par un fichier dont la dernière trame manque. Les trois sont des
défauts que `pont/decoupe.rs` peut réellement produire.

### F2 — l'écriture, et sa durabilité

**Livre** : les notifications de ProjFS ; le write-back à la fermeture du
handle (§6.2) ; le journal de reprise (§6.3) ; la création de fichiers et de
répertoires ; le compteur d'écritures dues et le `beforeunload` de la
page-shell ; `showDirectoryPicker({ mode: 'readwrite' })`.

**Reçu si** :
1. un fichier enregistré depuis le **Bloc-notes** apparaît côté poste local
   avec le bon contenu ;
2. un fichier enregistré depuis une application employant l'idiome
   **écrire-temporaire / renommer / supprimer** (LibreOffice ou Word) apparaît
   de même — **c'est ce critère, et lui seul, qui justifie D5** (§3.5) ;
3. le pont tué entre l'écriture et l'acquittement, puis relancé, **pousse
   l'écriture en attente** et le journal redevient vide ;
4. la page-shell affiche un nombre d'écritures dues non nul pendant
   l'opération, et **zéro** après.

**ROUGE** : l'enregistrement échoue ; le fichier n'arrive pas ; son contenu
diffère ; le journal reste non vide après reprise ; ou le compteur reste à zéro
pendant l'opération — ce dernier cas est le plus insidieux, parce qu'un
compteur qui vaut toujours zéro **ressemble à un succès**.
**Provoqué** : en désarmant la poussée (une variable de banc), on doit voir le
compteur croître sans jamais redescendre. Un compteur qu'on n'a jamais vu
monter n'est pas un compteur.

### F3 — renommage, suppression, et robustesse du canal

**Livre** : `Renommer` (avec `move()` et son repli de §3.5.1, le repli
instrumenté), `Supprimer`, la table des HRESULT du §5 dans son intégralité, les
trois budgets de délai du §5.3, et le contrôle de flux du §7.3.

**Reçu si** :
1. renommer un fichier, puis un répertoire **contenant un sous-répertoire**,
   réussit des deux côtés — c'est précisément le cas que l'ancien pont ne sait
   pas faire (`web/index.js:631`, §3.5.1) ;
2. supprimer un fichier, puis un répertoire non vide, se répercute ;
3. **le canal coupé en pleine lecture rend `ERROR_IO_DEVICE` à l'application en
   moins du budget**, l'application ne se fige pas, et la vidéo n'a pas perdu
   une image ;
4. chacun des douze codes du §5 est **observé au moins une fois** au journal du
   pont sur l'ensemble de la recette.

**ROUGE** : un renommage de répertoire échoue ; une suppression n'arrive pas ;
l'application se fige au-delà du budget ; ou **un code du §5 n'apparaît jamais**
— auquel cas soit le cas n'a pas été exercé, soit il n'est pas atteignable, et
il faut dire lequel des deux.

⚠️ Le critère (4) est celui qui **empêche la table du §5 d'être décorative**.
Une table de codes que rien n'exerce est une intention, pas un comportement.

### F4 — la mesure que le cadrage RÉCLAME

C'est le seul livrable de ce sous-projet que le cadrage nomme explicitement :
l'amendement du 28/07/2026 pose « **instrumenter la latence de listage et de
lecture de ProjFS** » comme le **critère de réexamen** de la décision « aucune
interception du sélecteur de fichiers en v1 », et exige que la décision se
prenne « sur mesure, pas sur intuition ».

**Livre** : un banc, piloté par variable d'environnement selon la convention du
dépôt (**`PONT_MESURE`, variable de BANC, jamais une configuration livrée**,
transmise explicitement par `scripts/run-agent.sh`), qui relève :

| Grandeur | Rangs |
| --- | --- |
| Latence d'énumération complète | répertoires de 10, 100, 1 000, 10 000 entrées |
| Latence de `GetPlaceholderInfo` | à froid, et cache d'énumération chaud |
| Latence de première lecture (hydratation) | fichiers de 4 Kio, 1 Mio, 100 Mio |
| Débit soutenu en lecture | même série |
| Coût du repli de renommage par copie (§3.5.1) | 1 Mio, 100 Mio |
| Taux d'occupation du cache négatif | ouverture d'un dossier dans l'Explorateur |

**Reçu si** : le document de résultats porte, pour chaque point, **le nombre
d'exécutions**, et conclut sur la question du cadrage — « le listage d'un
dossier volumineux dégrade-t-il réellement l'expérience ? ».

⚠️ **« Ne tranche rien » est une issue acceptable et prévue**, exactement comme
pour l'A/B de `set_desired_bitrate` en D9 et D10. La spec promet un plan de
mesure, pas un verdict. Ce qui n'est **pas** acceptable est un verdict sans son
nombre d'exécutions.

⚠️ **Ce que F4 NE fait PAS : rouvrir la décision.** L'amendement écarte
**définitivement** le hook d'API par injection de DLL — détecté comme
malveillant par les anti-cheat et les antivirus, donc incompatible avec la cible
jeu, et inopérant sur les sélecteurs propriétaires (Office, Qt, Electron). F4
alimente le seul réexamen que l'amendement autorise, celui de la voie par
détection de classe de fenêtre `#32770`, et **ce réexamen n'est pas dans ce
sous-projet**.

### F5 — la vie longue

**Livre** : `Rafraichir` et son bouton (§7.4) ; la mesure de l'occupation
disque de la racine et sa trace (§6.4) ; la reprise du journal à travers un
redémarrage de la VM ; le comportement quand l'utilisateur revient avec un
autre répertoire (§6.4 cas 2).

**Reçu si** : un fichier ajouté sur le poste local apparaît dans la VM après
`Rafraichir` et **pas avant** ; le journal survit à un redémarrage complet de la
VM et se vide à la reconnexion ; et le retour avec un répertoire différent
**demande** à l'utilisateur au lieu d'écrire.

**ROUGE** : le fichier apparaît **sans** `Rafraichir` — ce qui voudrait dire
qu'aucun cache d'énumération ne fonctionne, donc que chaque listage paie un
aller-retour, donc que la mesure de F4 portait sur autre chose que ce que le
produit livre.

---

## 9. Le plafond de 500 lignes, budgété d'avance

**Relevé par la commande de `CLAUDE.md` le 19 août 2026**, et non recopié.
Aucun fichier de ce sous-projet n'existe encore ; ce qui suit est l'état des
fichiers **existants** que le sous-projet va toucher.

| Fichier existant | Lignes | Marge | Rôle dans ce sous-projet |
| --- | --- | --- | --- |
| `agent/src/main.rs` | **328** | 172 | +1 branche `PONT=1` (`:315` en donne la forme exacte) |
| `agent/src/superviseur/lanceur.rs` | **367** | 133 | `lancer_pont`, transposé de `lancer_capteur` (`:156-188`) |
| `agent/src/superviseur/boucle.rs` | **263** | 237 | câblage de la surveillance |
| `agent/src/superviseur/boucle/surveillance_capteur.rs` | **148** | — | **modèle**, non modifié ; `surveillance_pont.rs` en est le jumeau |
| `proto/src/lib.rs` | **4** | — | +1 `pub mod fichiers;` |
| `client/src/shell-page.ts` | **71** | 429 | le bouton et le câblage |
| `client/src/shell.ts` | **93** | 407 | l'état « lecteur monté / écritures dues » |
| `agent/Cargo.toml` | — | — | +1 fonctionnalité `Win32_Storage_ProjectedFileSystem` |

**Aucune extraction préalable n'est requise** : les six fichiers de code
touchés ont tous une marge supérieure à 130 lignes, la plus étroite étant celle
de `superviseur/lanceur.rs` (133). ⚠️ **Cette aisance ne durera pas** — D9 et
D10 l'ont perdue en une tâche chacun —, et le découpage des modules **neufs**
est donc posé maintenant plutôt que subi plus tard.

**Le découpage des modules neufs**, avec les tailles visées :

| Module neuf | Visé | Pourquoi il est séparé |
| --- | --- | --- |
| `proto/src/fichiers.rs` | ≤ 400 | trames + tests ; si les tests le font franchir, ils partent dans `fichiers/tests.rs` par `#[path]` (précédent : `superviseur/table.rs`) |
| `agent/src/pont.rs` | ≤ 200 | l'assemblage, et rien d'autre |
| `agent/src/pont/transport.rs` | ≤ 300 | signaling + str0m données seules |
| `agent/src/pont/table.rs` | ≤ 350 | pur |
| `agent/src/pont/decoupe.rs` | ≤ 150 | pur |
| `agent/src/pont/chemins.rs` | ≤ 250 | pur |
| `agent/src/pont/erreurs.rs` | ≤ 150 | pur |
| `agent/src/pont/journal.rs` | ≤ 300 | pur |
| `agent/src/pont/projfs.rs` | ≤ 400 | `#[cfg(windows)]` — **s'il approche 500, les rappels d'énumération partent dans `projfs/enumeration.rs`, jamais par compression** |
| `agent/src/pont/projfs/chargement.rs` | ≤ 200 | `#[cfg(windows)]` — les treize transcriptions |
| `agent/src/superviseur/boucle/surveillance_pont.rs` | ≤ 200 | jumeau de `surveillance_capteur.rs` |
| `client/src/fichiers/*.ts` | ≤ 300 chacun | protocole, adaptateur FSA, état — trois fichiers |

⚠️ **Règle du dépôt, rappelée parce qu'elle a été violée deux fois en D9 et
franchie trois fois en D10** : quand un fichier franchit 500, on **extrait**,
jamais on ne comprime. Et l'extraction se place **avant** l'addition qui la
rend nécessaire — c'est le seul geste qui ait fonctionné (D9,
`serveur/instances.rs` : marge rendue de 10 à 65).

⚠️ **Convention de module enfant** (`CLAUDE.md`, § « Convention de module
enfant ») : ces modules-ci n'en relèvent **pas**. Elle ne s'applique qu'aux
modules qu'on extrait d'un parent `#[cfg(windows)]` pour les faire compiler sur
l'hôte. `pont/table.rs`, `pont/decoupe.rs` et les autres sont des enfants
ordinaires d'un `pont.rs` qui n'est **pas** gaté : ils se déclarent par un
simple `mod` à l'intérieur de leur parent, et **aucun `#[path]` n'est requis**.
Seul `pont/projfs.rs` est gaté, et il n'a rien à hisser.

---

## 10. Risques, et ce qui rendrait ce sous-projet non livrable

| # | Risque | Ce qu'on en sait, et la parade |
| --- | --- | --- |
| **R1** | 🔴 **ProjFS ne s'active pas sur cette VM, ou exige un redémarrage impossible à obtenir** | **Éliminatoire, et il est TRAITÉ EN PREMIER** : c'est F0, et son état rouge est l'état constaté d'aujourd'hui (§2). Aucun autre travail n'a de sens avant. Repli s'il échoue : **aucun** — ProjFS est le seul mécanisme de première partie qui projette un système de fichiers en espace utilisateur sous Windows. Le repli serait un pilote de mini-filtre, hors de portée, ou un retour à SMB/WebDAV, que le cadrage a écarté |
| **R2** | 🔴 **La latence d'un aller-retour navigateur rend le lecteur inutilisable en pratique** — l'Explorateur, les dialogues `IFileOpenDialog` et les applications sondent des dizaines de chemins par ouverture de dossier | **C'est le risque n°1 après R1, et c'est celui que l'amendement du 28/07/2026 anticipait.** Parades posées dès F1 : cache négatif (§7.4), cache d'énumération, rappels asynchrones (§4.3) qui ne bloquent jamais. **Non mesuré** : c'est l'objet de F4. Si F4 conclut à l'inutilisabilité, le sous-projet reste livré (le lecteur fonctionne) mais le réexamen du sélecteur devient dû |
| **R3** | **`FileSystemHandle.move()` disparaît, ou n'a jamais existé sur le navigateur cible** | Le repli copie + `removeEntry` (§3.5.1) est **standard** et est implémenté dès F3, pas gardé en réserve. Coût connu et instrumenté |
| **R4** | **Le disque de la VM se remplit d'hydratations** (§6.4) | Mesuré et journalisé dès F5 ; **aucune politique d'éviction en v1**, et c'est déclaré. `PrjDeleteFile` est chargé (§4.3) pour que la politique, quand elle viendra, n'ait pas à rouvrir la couche |
| **R5** | **Le pont concurrence la vidéo pour le lien réseau** — le budget `BUDGET_BPS` de D6 est réparti entre les fenêtres et **ne connaît pas ce trafic-ci** | **Non traité en v1, et nommé.** Une lecture soutenue de gros fichiers prend de la bande passante que le contrôleur de congestion vidéo interprétera comme une dégradation du lien, donc fera descendre la résolution. Le contrôle de flux du §7.3 borne la file SCTP, **pas le débit**. Une part de budget pour le pont est un sujet à part entière, et il appartient à un successeur |
| **R6** | **Une panique dans un rappel abat le processus pont** | `catch_unwind` à chaque frontière FFI (§4.3), et D2 fait que le pire cas est la mort du pont, relancé par le superviseur, journal intact (§6.3) |
| **R7** | **Les treize signatures transcrites à la main divergent de l'ABI réelle** (§3.1) | Test d'hôte de comparaison aux types `PRJ_*_CB`, citation obligatoire du numéro de ligne source en revue. **Ce n'est pas une garantie** — un mauvais `transmute` de fonction est un défaut que rien n'attrape avant l'exécution |
| **R8** | **Le contenu écrit n'arrive jamais** (§6.4) | Trois cas, tous nommés, aucun refermé. C'est une **propriété du montage**, pas un défaut à corriger : la donnée peut naître sur une machine dont l'autre bout est déjà parti |
| **R9** | **Deux applications de la VM et le poste local écrivent le même fichier** | Aucun verrou inter-machine (§3.5.2). Un seul utilisateur par VM borne le risque ; il ne l'annule pas |

**Ce qui rendrait le sous-projet NON LIVRABLE** : **R1 seul.** Tous les autres
dégradent, bornent ou reportent ; R1 supprime le mécanisme. C'est pourquoi F0
est un sous-bloc à lui seul, placé avant tout code, et pourquoi son critère est
la première chose à jouer.

---

## 11. Ce que ce sous-projet n'établira PAS

Écrit d'avance, pour qu'aucun document de résultats n'ait à le découvrir.

- **Aucun taux.** Deux exécutions par critère est la règle de ce dépôt ; deux
  exécutions ne font pas une fréquence.
- **Aucune constante de ce sous-projet ne sera calibrée** :
  `TAILLE_TRAME_MAX`, `TAILLE_MAX_FICHIER`, `TTL_ENUMERATION`, `SEUIL_TAMPON`
  et les trois budgets de délai du §5.3 sont **posés**, pas mesurés. Elles
  rejoignent `BPP_MIN`, `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`, `HYSTERESIS`,
  `REPIT_APRES_ECHEC`, `TAILLE_MAX_SORTIE`, `REPIT_REARMEMENT_AUDIO` et
  `REARMEMENTS_MAX`. **F4 donnera de quoi en calibrer trois** (les délais) ;
  les autres resteront non jugées.
- **Aucun jugement d'usage** : personne n'aura dit si le lecteur est
  *agréable*. F4 mesure des latences, pas une expérience — même lacune que
  `BPP_MIN` traîne depuis le chantier C volet 1.
- **Rien du comportement à travers une reconnexion WebRTC** : le canal
  `fichiers` sera rétabli par une nouvelle `PeerConnection`, jamais par
  renégociation ; le chemin de renégociation n'existe pas plus ici que dans le
  reste du produit (`agent/src/demarrage.rs` journalise « aucune renégociation
  possible pour cette session »).
- **Rien d'un client réel** : le montage de recette de ce dépôt est un Chrome
  sans interface à décodage logiciel, sur l'hôte qui porte la VM. Aucun
  navigateur autre que Chromium n'aura été essayé — et la File System Access
  API n'existe ni sur Firefox ni sur Safari, ce qui n'est pas une limite du
  pont mais une limite du produit, héritée du cadrage.
- **Rien de plusieurs utilisateurs** : une VM, un utilisateur, une racine.
- **Rien du sélecteur de fichiers Windows** : F4 mesure ; il ne décide pas, et
  le réexamen appartient à un autre chantier (§8 F4).
- **Rien de l'ancien pont** : il n'est ni modifié, ni retiré, ni comparé
  chiffre à chiffre. §13 l'inventorie ; il ne le mesure pas.
- **Aucun test d'hôte ne couvrira `pont/projfs.rs`**, qui est
  `#[cfg(windows)]` et appelé par le système. `cargo check --target
  x86_64-pc-windows-gnu` en vérifiera les types, les emprunts et les durées de
  vie — **jamais le comportement**.

---

## 12. Hors périmètre v1, explicitement

- **Tout ce que liste le §3.5.2** : attributs Windows, horodatages en écriture,
  ACL, ADS et Marque du Web, liens, verrous inter-machines, corbeille,
  notification de changement côté poste local.
- **La troncature explicite** (`SetEndOfFile` sur un fichier projeté) est
  **couverte** par l'hydratation ProjFS et le write-back qui suit ; **le message
  `Tronquer` du protocole n'existe que pour le repli de renommage** (§3.5.1) et
  pour l'écriture d'un fichier devenu plus court.
  ⚠️ **Piège hérité, à ne pas rejouer** : l'ancien pont n'avait **aucune**
  opération `truncate` (relevé §13) et employait
  `createWritable({ keepExistingData: true })` (`web/index.js:572`) — un fichier
  réécrit plus court **conservait sa queue d'octets**. Le pont v1 appelle
  `createWritable()` **sans** `keepExistingData` quand il réécrit un fichier
  entier, et `truncate()` quand il n'en réécrit qu'une partie.
- **Le montage d'un second répertoire**, ou d'un répertoire par application.
- **La persistance du `FileSystemDirectoryHandle`** (§3.3).
- **Une lettre de lecteur** (§3.6).
- **Une politique d'éviction des hydratations** (§6.4, R4).
- **Une part du budget de débit pour le pont** (R5).
- **L'interception du sélecteur de fichiers Windows** — écartée par
  l'amendement du 28/07/2026, et ce sous-projet ne la rouvre pas.
- **L'upload d'installeurs** : c'est le sous-projet ④, il a son propre chemin
  (navigateur → plateforme → agent), et il ne passe pas par ce pont.

---

## 13. Annexe — ce que l'ancien pont enseigne

Inventaire relevé ligne à ligne dans `src/file.js` (414 lignes) et
`web/index.js` (partie filesystem : **427-675**). Il n'est pas là pour être
imité ; il est là parce que **trois décisions de cette spec en sortent
directement**.

**Onze opérations FUSE seulement** (`src/file.js:170-336`) : `statfs`,
`readdir`, `getattr`, `open`, `release`, `read`, `write`, `create`, `mkdir`,
`rmdir`, `unlink`, `rename`. **Absentes** : `truncate`, `chmod`, `chown`,
`utimens`, `flush`, `fsync`, `access`, `opendir`, les liens, les attributs
étendus. Deux des onze sont des souches : `open` rend inconditionnellement
`cb(0, 42)` (`:220-222`) et `release` rend `cb(0)` sans rien faire
(`:224-226`) — **aucun flush n'est donc déclenché à la fermeture d'un
fichier**, et le descripteur `42` est le même pour tous les fichiers, ce qui
rend `O_TRUNC`, `O_APPEND` et `O_EXCL` inopérants.

**Ce que cette spec en tire, point par point :**

| Fait relevé | Décision de cette spec |
| --- | --- |
| `unlink` (`:309`) et `rmdir` (`:297`) figurent parmi les onze opérations jugées indispensables | **D5** : la suppression entre dans le périmètre v1 (§3.5) |
| Les octets sont encodés en JSON, à l'aller (`:264`, `Array.from`) comme au retour (`web/index.js:653-657`, `JSON.stringify` d'un `Int8Array`) | **Trames binaires** (§4.2) |
| Un écouteur `message` par requête (`:155`), jamais retiré sur le chemin d'erreur (`:126-129`) | **Une table unique indexée par entier, purgée par expiration** (§4.2, `pont/table.rs`) |
| Neuf sites rendent `cb(-1)` = `EPERM` pour toutes les causes | **Une table de codes distincts** (§5), et un critère de recette qui exige que chacun soit observé (F3) |
| `JSON.stringify(e)` d'une `Error` rend `"{}"` (`web/index.js:669`), reconstruit en `new Error("{}")` (`:127`) | **Un code d'énumération transporté, jamais une chaîne libre** (§5.1) |
| Chaque `read` fait `getFile()` + `arrayBuffer()` + `.slice()` — le fichier entier par requête (`web/index.js:562-564`) | **Lecture par plage, jamais le fichier entier en mémoire** (§7.3) |
| Cache de données **sans TTL**, invalidé seulement par une écriture du pont lui-même (`:232-241`, `:270`) | **Aucun cache de données** ; l'hydratation ProjFS en tient lieu (§7.4) |
| `size: 1000000000` en dur pour la racine (`web/index.js:517`), et un `statfs` entièrement constant (`:173-185`) | **La taille annoncée est vraie** — celle du volume de la VM, ce qui est une limite dite, pas un chiffre inventé (§3.5.2) |
| Un timeout unique de 10 s pour toutes les opérations (`:89`) | **Trois budgets distincts**, et ils sont déclarés non calibrés (§5.3) |
| `keepExistingData: true` (`web/index.js:572`) sans `truncate` FUSE | **`createWritable()` sans `keepExistingData` pour une réécriture entière** (§12) |
| `newDir` masqué par lui-même (`web/index.js:631`) : le renommage d'un répertoire contenant un sous-répertoire **échoue toujours** | **Critère de réception F3 (1)**, écrit pour exercer exactement ce cas |
| `FileSystemHandle.remove()` (`:605`, `:612`, `:639`) et `handle.move()` (`:628`, `:644`) sont **non standard** | **`removeEntry()` standard**, et `move()` détecté avec repli (§3.5.1) |
| L'arrêt forcé n'appelle **pas** les callbacks FUSE en attente (`src/file.js:359-365`) : le noyau n'obtient jamais de réponse | **Toute commande en table a une échéance** (§5.3), et `PrjStopVirtualizing` est précédé de la complétion en erreur de tout ce qui reste |
| Aucun heartbeat serveur → navigateur, aucune détection de pair mort (`web/index.js:442-454`, ignoré par `src/file.js:121`) | **SCTP/DTLS détecte la mort du pair** ; le pont n'a pas de battement à inventer |

⚠️ **Ce qu'il ne faut PAS en conclure.** Cet inventaire ne dit pas que le pont
historique était mal écrit : il dit qu'un pont fichiers a une surface de
défauts très large, que **chacun de ces défauts est silencieux**, et qu'il n'y
en a pas un seul que cette spec puisse déclarer impossible chez elle. Les
décisions ci-dessus les rendent **atteignables par un test** ; elles ne les
rendent pas absents.
