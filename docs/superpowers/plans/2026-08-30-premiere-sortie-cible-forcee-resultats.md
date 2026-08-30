# Lot 32 — la première sortie virtuelle et la cible forcée : la conception du remède

Date : 30 août 2026. Branche `package-nivuus`. Dépôt `packages/desk`.

🔴 **CE DOCUMENT NE MODIFIE AUCUNE LIGNE DU PRODUIT.** Il rend une rédaction
proposée, avec ses `fichier:ligne`, et demande l'autorisation. Toutes les
lignes citées ont été relues sur l'arbre au commit `25fd8f6`, après lecture,
et aucune édition de ce lot ne les déplace — puisqu'il n'en fait aucune.

🔴 **AUCUNE MESURE SUR LA VM N'A ÉTÉ FAITE PAR CE LOT**, à dessein : la VM ne
lui appartient pas. Tout ce qui suit est une **lecture de code** et une
**relecture de pièces déjà versionnées**, et c'est dit comme tel partout. La
seule mesure qui trancherait est nommée au § 7, et elle est **demandée**, pas
jouée.

---

## 0. Ce qui est pris pour acquis

Du lot 30, sans le remesurer :

- sans le VGA QEMU, la session 1 démarre avec **0 moniteur PnP mais 1 écran**
  (`\\.\DISPLAY5`), cible active à `statusFlags=0x11`
  (`IN_USE | FORCED_AVAILABILITY_SYSTEM`) — une **cible forcée** que Windows
  fabrique quand il ne reste aucun affichage ;
- **le premier moniteur virtuel créé REMPLACE cette cible forcée sur la MÊME
  source** : il hérite du même nom GDI/DXGI, il n'« apparaît » donc pas ;
- le produit n'apparie que **parmi les sorties APPARUES**, donc `parues=[]`,
  refus, et la sortie est rendue au pilote ;
- la deuxième sortie s'attache très bien : **4 créées → 4 attachées** sans VGA ;
- l'hypothèse « registre pollué » est **RÉFUTÉE** ;
- c'est un **défaut de code de `desk`**, pas une limite de Windows ni de
  SudoVDA.

---

## 1. Le chemin exact, relu

### 1.1 `agent/src/superviseur/boucle/creation_sortie.rs` (295 lignes)

C'est le seul appelant de la règle en cause. La séquence, dans l'ordre :

| Ligne | Ce qui s'y passe |
| --- | --- |
| `:59-60` | `borner_a_la_taille_max` plafonne le viewport à `TAILLE_MAX_SORTIE` |
| `:71` | `let avant = … noms_attaches(&…)` — **le relevé d'AVANT création**, un `Vec<String>` de noms GDI attachés |
| `:86` | `sorties.creer(largeur, hauteur, 60)` → `id_pilote: u32` |
| `:100` | `let apparues = attendre_une_sortie_neuve(pilote, &avant, LIMITE_RATTACHEMENT)` |
| `:102` | `let Some(cible) = placement::sortie_pour_viewport(&apparues, largeur, hauteur, prises) else { … refus … }` |
| `:132` | `taille_retenue` |
| `:139` | `table.sortie_creee(&session, id_pilote, nom, retenue)` |
| `:231-269` | `attendre_une_sortie_neuve`, dont la ligne qui décide tout : |

```rust
// creation_sortie.rs:242-246
let apparues: Vec<_> = toutes
    .iter()
    .filter(|s| s.attachee_au_bureau && !avant.contains(&s.nom_sortie))
    .cloned()
    .collect();
```

🔴 **`!avant.contains(&s.nom_sortie)` est le défaut, et il est là, pas à
`:102`.** `:102` ne fait que consommer un vecteur déjà vidé. Le lot 30 a
raison de nommer `:100-102` comme le lieu du refus ; **le prédicat fautif est
`:244`**, et c'est lui qu'une rédaction doit toucher.

**Ce que la règle protège — la raison est ÉCRITE, à `:62-70`, et il faut la
nommer avant de la relâcher :**

> `sortie_pour_viewport` ne filtre que sur « attachée, ASSEZ GRANDE, pas déjà
> prise » : rien n'y exclut les sorties PRÉEXISTANTES. Or le viewport annoncé
> par le navigateur peut parfaitement égaler, ou même être plus petit que, la
> résolution d'un moniteur physique […]. Sans ce relevé, la fenêtre serait
> posée sur l'écran RÉEL de la VM et la sortie virtuelle qu'on vient de créer
> deviendrait orpheline.

Donc : **le relevé `avant` existe pour empêcher qu'une fenêtre soit posée sur
l'écran physique de la VM, avec la sortie neuve laissée orpheline** (une place
sur dix, tenue jusqu'à l'arrêt du superviseur). Ce n'est pas une précaution
décorative : depuis D10, `sortie_assez_grande` est une **inégalité**, si bien
qu'un moniteur 4K convient à *n'importe quel* viewport — `placement.rs:82-88`
le dit en toutes lettres.

### 1.2 `placement::sortie_pour_viewport` — `agent/src/superviseur/placement.rs:90-105`

```rust
pub fn sortie_pour_viewport(sorties: &[SortieDxgi], largeur: u32, hauteur: u32,
                            deja_prises: &[String]) -> Option<SortieDxgi> {
    sorties.iter().find(|s| {
        s.attachee_au_bureau
            && sortie_assez_grande((s.rect.width, s.rect.height), (largeur, hauteur))
            && !deja_prises.contains(&s.nom_sortie)
    }).cloned()
}
```

Trois filtres, aucun n'est une identité : **attachée**, **assez grande**
(inégalité + `TOLERANCE_PX = 4`, `placement.rs:36`), **pas déjà prise**. Le
module est **pur et testé sur l'hôte** (`placement.rs:200-335`), sa moitié
Win32 est dans un `#[cfg(windows)] mod win` interne. `placement` n'est **pas**
gaté (`superviseur.rs:13`).

### 1.3 Le « jumeau » — ⚠️ ce n'est pas `montee.rs:400`, et la distinction compte

Le brief cite `agent/src/superviseur/boucle/montee.rs:400`. **Ce fichier
n'existe pas.** Le seul `montee.rs` de l'arbre est
`agent/src/diagnostics/multifenetre/montee.rs`, et sa ligne 400 est bien la
règle jumelle :

```rust
// diagnostics/multifenetre/montee.rs:390-400
let disparues = manquants(&noms_connus, &noms);
let parues    = manquants(&noms, &noms_connus);
…
if !disparues.is_empty() || parues.len() != 1 {
```

🔴 **C'est une SONDE DE BANC, pas le produit** (`MULTIFENETRE_VDD=1` et ses
voisines — `diagnostics::aiguiller`). Sa règle est plus dure encore
(`parues.len() != 1` et `disparues` vide), et elle a une **raison propre**,
écrite à `:378-389` : détecter qu'Apollo remplace une sortie
(`ensure_only_display`). Une sonde qui échoue bruyamment sur un remplacement
est **correcte** : c'est son travail de le voir.

**Conséquence pour la rédaction : le produit et la sonde ne doivent PAS être
corrigés du même geste.** Le produit doit désigner sa sortie ; la sonde doit
continuer de dénoncer un remplacement. Les corriger ensemble ferait taire
l'instrument qui a servi à trouver le défaut. **Je ne propose donc AUCUNE
modification de `montee.rs`** — au plus, une ligne de commentaire, et je ne la
propose même pas : ce serait de la croissance gratuite.

⚠️ **Ce que la sonde rendrait aujourd'hui dans le cas du lot 30** : `parues`
vide et `disparues` vide → `parues.len() != 1` → `arret = "topologie non
suivie"`. La sonde **verrait donc le remplacement**, et le dirait. Elle n'a
jamais été jouée sans le VGA : c'est une prédiction de lecture, pas une mesure,
et elle est dite comme telle.

### 1.4 `pilote.rs:269` et ce que le pilote rend à l'appelant

`agent/src/moniteurs_virtuels/pilote.rs`, 463 lignes, `#[cfg(windows)]`
(`moniteurs_virtuels.rs:22-24`).

À la création (`:249-410`), l'agent **choisit** :

```rust
// pilote.rs:263-270
let demande = DemandeAjout {
    largeur, hauteur, hertz,
    guid_moniteur,                                   // choisi par nous (`guid_pour(numero)`)
    nom_peripherique: en_champ_14("Guacamole"),
    numero_serie: en_champ_14(&format!("mesure{numero}")),   // :269 — la série EDID
};
```

Et le pilote **rend** (`sudovda.rs:84-90`) :

```rust
#[repr(C)] #[derive(Default)]
pub(super) struct SortieAjoutee {
    pub(super) adaptateur_bas: u32,     // LUID.LowPart
    pub(super) adaptateur_haut: i32,    // LUID.HighPart
    pub(super) identifiant_cible: u32,  // « C'est lui qui devient l'`IdSortie` du trait. »
}
```

🔴 **Le pilote rend donc TROIS nombres, et le produit n'en garde qu'UN.**
`pilote.rs:358-361` retient `id = ajoutee.identifiant_cible` dans
`apparies: Vec<(IdSortie, GUID)>` (`:82`) ; le LUID n'est **que journalisé**
(`:366-367`), puis **jeté**. `IdSortie` est un simple `u32`
(`moniteurs_virtuels.rs:44`), et `Sorties::creer`
(`moniteurs_virtuels.rs:74-78`) ne fait que le repasser.

**Réponse à la question posée** : le pilote ne rend **ni handle ni nom** — il
rend un **couple (adaptateur LUID, identifiant de cible)**, dont le produit ne
conserve aujourd'hui que la moitié.

Le commentaire de `placement.rs:4-7` affirme depuis D1 :

> Le premier rend un identifiant de cible qui lui appartient, le second énumère
> par `(index_adaptateur, index_sortie)`. **Aucune correspondance n'est
> exposée** : l'appariement se fait donc par dimensions et par élimination.

🔴 **Cette phrase est le cœur du défaut, et elle est FAUSSE.** Une
correspondance est exposée, par une API que ce dépôt n'appelle nulle part.

---

## 2. La voie C est OUVERTE — et pas par la série EDID

### 2.1 Ce que la question du brief demandait, et ce qu'elle trouve

La question posée est : « le nom GDI/DXGI est-il rattachable à la **série
EDID** depuis le code existant ? »

**Par la série EDID : non, pas raisonnablement.** Il faudrait remonter
EDID → blob `Device Parameters\EDID` du registre → chemin d'instance du
moniteur → cible CCD → source → nom GDI. C'est trois indirections, dont une
lecture de registre et un décodage EDID, pour retrouver une information que
l'on a déjà en main sous une forme exacte.

**Mais la voie C est ouverte par mieux que la série EDID :
`identifiant_cible` EST déjà l'identifiant de cible du système d'affichage.**
La série EDID n'est pas nécessaire.

### 2.2 Le chaînage, et l'API

`identifiant_cible` + `(adaptateur_bas, adaptateur_haut)` sont exactement la
paire `(id, adapterId)` d'un `DISPLAYCONFIG_PATH_TARGET_INFO` de l'API CCD
(*Connecting and Configuring Displays*) :

```
windows-0.62.2/src/Windows/Win32/Devices/Display/mod.rs
  :376  GetDisplayConfigBufferSizes
  :506  QueryDisplayConfig
  :57   DisplayConfigGetDeviceInfo
  :1344 DISPLAYCONFIG_PATH_INFO    { sourceInfo, targetInfo, flags }
  :1356 DISPLAYCONFIG_PATH_SOURCE_INFO { adapterId: LUID, id: u32, … }
  :1385 DISPLAYCONFIG_PATH_TARGET_INFO { adapterId: LUID, id: u32, …, statusFlags }
  :1548 DISPLAYCONFIG_SOURCE_DEVICE_NAME { header, viewGdiDeviceName: [u16; 32] }
  :1210 DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME
  :3925 QDC_ONLY_ACTIVE_PATHS
```

Le chaînage tient en trois pas :

1. `QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS, …)` → un tableau de chemins ;
2. retenir le chemin dont `targetInfo.adapterId == notre LUID` **et**
   `targetInfo.id == identifiant_cible` ;
3. `DisplayConfigGetDeviceInfo(DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME)` sur
   `sourceInfo` → **`viewGdiDeviceName`**, qui est littéralement `\\.\DISPLAYn`.

Or `SortieDxgi::nom_sortie` (`sortie_dxgi.rs:20`) est peuplé par
`capture/enumeration.rs:69` depuis `DXGI_OUTPUT_DESC.DeviceName` — **le même
nom GDI**. Les deux bouts se rejoignent donc sur une chaîne, pas sur un rang.

### 2.3 Pourquoi c'est le bon remède, précisément dans le cas du lot 30

Dans le cas fautif, la cible forcée est remplacée **sur la même source**.
Après remplacement, le chemin actif est `{ source S, target T_nôtre }`. Le
chaînage rend donc `S → \\.\DISPLAY5` : **exactement le nom que le produit
doit retenir**, et exactement celui que la différence d'ensembles écarte. La
voie C n'a pas besoin d'un cas particulier pour la cible forcée : elle est
**structurellement insensible** à savoir si la sortie est apparue ou a
remplacé quelque chose — et, accessoirement, à toute course (une sortie
créée par Apollo pendant l'attente ne peut plus être prise pour la nôtre,
ce que la différence d'ensembles ne garantit pas).

### 2.4 Ce sur quoi la voie C repose, et la pièce qui le rend crédible

Elle repose sur **une hypothèse** : `identifiant_cible` rendu par SudoVDA est
l'`id` de cible CCD sur l'adaptateur `(adaptateur_bas, adaptateur_haut)`.

⚠️ `sudovda.rs:96` dit lui-même que la disposition de `VIRTUAL_DISPLAY_ADD_OUT`
est « non confirmée ». **L'hypothèse n'est donc pas gratuite, et je ne
l'affirme pas.**

🔵 **Mais une pièce versionnée la rend crédible**, et elle n'a pas été
fabriquée pour l'occasion — c'est le lot 22,
`docs/superpowers/plans/2026-08-30-lot22-hub-session-resultats.md:157-160` :

> **Sorties orphelines comptées** […] : **dix** nœuds de moniteur fantômes
> `DISPLAY\SMKD1CE\…UID256` à `UID265` […]. Ils ne bloquent rien — **les
> identifiants du pilote (256…265)** tournent en rond […]

Le suffixe `UIDnnnn` d'un chemin d'instance de moniteur Windows **est** l'`id`
de cible CCD de ce moniteur. Les identifiants rendus par le pilote sont
256…265 ; les UID des moniteurs qu'il a créés sont 256…265. **La
correspondance est donc déjà observée sur cette VM**, par une mesure d'un
autre lot, faite pour une autre raison.

🔴 **Ce n'est pas une preuve, et je ne l'écris pas comme telle** : elle
établit que les nombres coïncident, pas que le LUID rendu est celui que CCD
emploie. La mesure qui trancherait est au § 7.

🔵 **Et si l'hypothèse est fausse, le coût est nul** : la recherche ne trouve
aucun chemin, la fonction rend `None`, et le produit retombe **exactement** sur
la règle d'aujourd'hui. C'est la propriété qui rend cette rédaction sûre à
livrer avant même de l'avoir mesurée sur la VM — et c'est la raison pour
laquelle le repli doit être **conservé**, jamais remplacé.

---

## 3. La rédaction proposée — voie C

Quatre éditions, plus un fichier neuf. **Aucune n'est appliquée.**

### 3.1 `agent/Cargo.toml` — une entrée de plus dans `features`

La liste alphabétique des `features` du crate `windows` (`agent/Cargo.toml:44`)
ne contient **pas** `Win32_Devices_Display`. Il l'y faut, inséré après
`"Win32_Devices_DeviceAndDriverInstallation"` :

```diff
     "Win32_Devices_DeviceAndDriverInstallation",
+    # `QueryDisplayConfig` / `DisplayConfigGetDeviceInfo` : la seule
+    # correspondance EXPOSÉE entre l'identifiant de cible que rend le pilote
+    # d'affichage virtuel et le nom GDI (`\\.\DISPLAYn`) que DXGI énumère.
+    # Voir `moniteurs_virtuels/config_affichage.rs`.
+    "Win32_Devices_Display",
```

La feature existe bien : `windows-0.62.2/Cargo.toml:397`
(`Win32_Devices_Display = ["Win32_Devices"]`).

### 3.2 `agent/src/moniteurs_virtuels.rs:44` — le type qui manque

```diff
 /// Identifiant d'une sortie virtuelle, tel que le pilote le rend.
 pub type IdSortie = u32;
+
+/// L'adaptateur sur lequel le pilote a créé une sortie : un `LUID` Win32,
+/// écrit en deux moitiés comme `sudovda::SortieAjoutee` l'écrit déjà.
+///
+/// **Le pilote rend TROIS nombres et le produit n'en gardait qu'un.** Le
+/// couple `(adaptateur, IdSortie)` est ce qui désigne une cible d'affichage
+/// sans ambiguïté ; l'`IdSortie` seul n'est unique que PAR adaptateur, et
+/// cette VM en a plus d'un (SudoVDA, plus le VGA de QEMU quand il est là).
+pub type Adaptateur = (u32, i32);
```

### 3.3 `agent/src/moniteurs_virtuels/pilote.rs` — conserver le LUID

Cinq touches, toutes mécaniques :

| Ligne | Aujourd'hui | Proposé |
| --- | --- | --- |
| `:82` | `apparies: Vec<(IdSortie, GUID)>,` | `apparies: Vec<(IdSortie, GUID, Adaptateur)>,` |
| `:172` | `.retain(\|(_, connu)\| *connu != guid_moniteur)` | `.retain(\|(_, connu, _)\| *connu != guid_moniteur)` |
| `:361` | `etat.apparies.push((id, guid_moniteur));` | `etat.apparies.push((id, guid_moniteur, (ajoutee.adaptateur_bas, ajoutee.adaptateur_haut)));` |
| `:385` | `.position(\|(connu, _)\| *connu == id)` | `.position(\|(connu, _, _)\| *connu == id)` |
| `:387` | `let (_, guid_moniteur) = etat.apparies[rang];` | `let (_, guid_moniteur, _) = etat.apparies[rang];` |
| `:409` | `.retain(\|(_, connu)\| *connu != guid_moniteur)` | `.retain(\|(_, connu, _)\| *connu != guid_moniteur)` |
| `:443` | `.map(\|(_, guid)\| *guid)` | `.map(\|(_, guid, _)\| *guid)` |

plus **un accesseur**, à poser juste après `a_purger` (`:246`) :

```rust
/// L'adaptateur sur lequel une sortie appariée a été créée.
///
/// `None` pour un identifiant que ce pilote n'a pas créé, ou dont le retrait
/// a échoué (il a alors quitté `apparies` — voir la doc d'`EtatSorties`).
///
/// ⚠️ **Ce couple ne sert QU'À DÉSIGNER, jamais à détruire** : le pilote
/// retire par GUID, et rien d'autre.
pub(crate) fn adaptateur_de(&self, id: IdSortie) -> Option<Adaptateur> {
    self.etat().apparies.iter().find(|(connu, _, _)| *connu == id).map(|(_, _, a)| *a)
}
```

⚠️ **La doc d'`EtatSorties` (`:69-82`) devient partiellement fausse** : elle
dit « une entrée d'`apparies` sert à traduire un identifiant en GUID ». Elle
en portera deux rôles. **Elle doit être reprise dans le même geste** — c'est
le patron « le naufrage du 487 », et ce fichier commente ses invariants.

🔴 **RÈGLE DES 500 LIGNES.** `pilote.rs` est à **463** lignes
(`wc -l`, 30 août 2026). La rédaction ci-dessus ajoute de l'ordre de **+15 à
+22** lignes (accesseur, doc, reprise de la doc d'`EtatSorties`), soit
**478 à 485**. C'est **sous** le plafond, mais la marge devient dérisoire, et
ce dépôt a payé six fois « la marge regagnée se reperd ». **Je propose donc
une tâche d'extraction PRÉALABLE et DÉDIÉE, avant celle qui ajoute** :
sortir `EtatSorties` (et lui seul) vers `agent/src/moniteurs_virtuels/pilote/etat.rs`
— le répertoire `pilote/` existe déjà et porte déjà `controle.rs`, extrait
pour cette exacte raison (`pilote.rs:56-59`). **Si tu préfères l'éviter**, la
rédaction tient telle quelle sous 500 ; c'est ton arbitrage, je le signale
plutôt que de le prendre.

### 3.4 Fichier neuf — `agent/src/moniteurs_virtuels/config_affichage.rs`

Déclaré par `mod config_affichage;` dans `moniteurs_virtuels.rs`, **hors
`#[cfg(windows)]`** — le patron de `placement.rs` : une règle **pure** en tête,
une moitié Win32 dans un `#[cfg(windows)] mod win` interne. Environ 150 lignes
avec ses commentaires et ses tests.

La partie pure — c'est elle qui se fige par un test d'hôte :

```rust
/// Un chemin d'affichage actif, réduit à ce dont l'appariement a besoin.
///
/// Un type À NOUS, et non `DISPLAYCONFIG_PATH_INFO` : c'est ce qui permet à la
/// règle ci-dessous d'être pure, testée sur l'hôte Linux, et de ne pas faire
/// entrer un type Win32 dans un module que `moniteurs_virtuels` compile
/// partout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheminActif {
    pub adaptateur_cible: Adaptateur,
    pub id_cible: u32,
    pub nom_gdi: String,
}

/// Le nom GDI de la sortie que le pilote vient de créer, désignée par ce
/// qu'on lui a donné plutôt que par une différence d'ensembles.
///
/// 🔴 **UNE AMBIGUÏTÉ REFUSE DE TRANCHER, elle ne prend pas le premier** —
/// précédent d'`AUDIO_PERIPHERIQUE` (`wasapi/peripherique.rs`) et leçon des
/// index DXGI de D1 : prendre le premier serait retomber sur un rang
/// d'énumération par la porte de derrière. Deux chemins portant la même paire
/// `(adaptateur, id)` est un état que Windows ne devrait pas produire ; s'il
/// le produit, l'appelant retombe sur son repli, qui est le produit d'hier.
pub fn nom_gdi_de_la_cible(
    chemins: &[CheminActif],
    adaptateur: Adaptateur,
    id_cible: u32,
) -> Option<&str> {
    let mut trouves = chemins
        .iter()
        .filter(|c| c.adaptateur_cible == adaptateur && c.id_cible == id_cible);
    let premier = trouves.next()?;
    if trouves.next().is_some() {
        return None;      // ambigu : on ne tranche pas
    }
    Some(&premier.nom_gdi)
}
```

et la moitié Win32, `#[cfg(windows)] mod win`, qui n'expose qu'une fonction :

```rust
/// Les chemins d'affichage ACTIFS, tels que le système les voit.
///
/// `QDC_ONLY_ACTIVE_PATHS` et non tous les chemins : une cible inactive n'a
/// aucune source, donc aucun nom GDI, donc rien à apparier.
///
/// ⚠️ **Silencieuse.** Elle est appelée dans une boucle de scrutation à 10 Hz
/// (`creation_sortie::attendre_une_sortie_neuve`), et ce dépôt a payé deux
/// fois une trace émise à la cadence d'une boucle. L'appelant journalise une
/// fois, sur le chemin d'échec.
pub fn chemins_actifs() -> anyhow::Result<Vec<CheminActif>> { … }
```

### 3.5 `agent/src/superviseur/boucle/creation_sortie.rs` — le geste central

**`:100-102`**, la désignation entre dans l'attente :

```diff
-    // Attend le FAIT — qu'une sortie neuve apparaisse dans la topologie DXGI —
-    // plutôt qu'un délai plat, tout en continuant de battre le chien de garde
-    // du pilote (voir la doc d'`attendre_une_sortie_neuve`).
-    let apparues = attendre_une_sortie_neuve(pilote, &avant, LIMITE_RATTACHEMENT);
+    // Attend le FAIT — que NOTRE sortie soit là — plutôt qu'un délai plat, et
+    // sans cesser de battre le chien de garde (voir la doc de la fonction).
+    let candidates =
+        attendre_notre_sortie(pilote, id_pilote, &avant, LIMITE_RATTACHEMENT);
 
-    let Some(cible) = placement::sortie_pour_viewport(&apparues, largeur, hauteur, prises)
+    let Some(cible) = placement::sortie_pour_viewport(&candidates, largeur, hauteur, prises)
     else {
```

**`:231-249`**, le prédicat fautif est doublé, jamais remplacé :

```diff
 fn attendre_une_sortie_neuve(
     pilote: &PiloteParIoctl,
+    id_pilote: IdSortie,
     avant: &[String],
     limite: std::time::Duration,
 ) -> Vec<SortieDxgi> {
+    // Le couple que le pilote nous a rendu, relu UNE fois : il ne bouge pas
+    // pendant l'attente, et l'aller-retour sous verrou n'a rien à faire dans
+    // une boucle à 10 Hz.
+    let adaptateur = pilote.adaptateur_de(id_pilote);
     let echeance = std::time::Instant::now() + limite;
     loop {
         if let Err(erreur) = pilote.pinguer() { … }
         let toutes = enumerer_sorties_silencieux().unwrap_or_default();
+
+        // ① DÉSIGNER — par ce qu'on a DONNÉ au pilote, pas par ce qui a
+        // changé autour. C'est le seul chemin qui survive au cas où la sortie
+        // neuve REMPLACE une cible forcée sur la même source : elle porte
+        // alors le nom GDI d'avant, n'« apparaît » donc jamais, et la
+        // différence d'ensembles ci-dessous rend un vecteur vide sur une
+        // sortie parfaitement utilisable (mesuré, lot 30).
+        if let Some(adaptateur) = adaptateur {
+            if let Ok(chemins) = config_affichage::chemins_actifs() {
+                if let Some(nom) =
+                    config_affichage::nom_gdi_de_la_cible(&chemins, adaptateur, id_pilote)
+                {
+                    if let Some(notre) = toutes
+                        .iter()
+                        .find(|s| s.attachee_au_bureau && s.nom_sortie == nom)
+                    {
+                        return vec![notre.clone()];
+                    }
+                }
+            }
+        }
+
+        // ② REPLI — le produit d'hier, MOT POUR MOT. Il court quand la
+        // désignation ne rend rien : pilote sans adaptateur connu, CCD muette
+        // ou en erreur, cible pas encore dans un chemin actif, ou paire
+        // ambiguë. 🔴 Il n'est PAS mort et ne doit pas être retiré : c'est lui
+        // qui garantit qu'une hypothèse fausse sur `identifiant_cible` dégrade
+        // vers le comportement connu au lieu de casser (voir l'en-tête de
+        // `sudovda::SortieAjoutee`, « disposition non confirmée »).
         let apparues: Vec<_> = toutes.iter()
             .filter(|s| s.attachee_au_bureau && !avant.contains(&s.nom_sortie))
             .cloned().collect();
         if !apparues.is_empty() { return apparues; }
```

(et le nom de la fonction devient `attendre_notre_sortie`, sa doc reprise en
conséquence — elle n'attend plus « une sortie neuve ».)

### 3.6 Ce que cette rédaction NE change PAS, et c'est le point

- **`sortie_pour_viewport` n'est pas touchée.** Ses trois filtres continuent de
  courir sur la sortie désignée : attachée, assez grande, **pas déjà prise**.
- 🔵 **`deja_prises` reste un GARDE, et il le faut.** Si la désignation nomme
  une sortie déjà attribuée à une session vivante, le produit refuse — comme
  aujourd'hui — et l'`ERROR` de `:118-127` porte désormais le **nom désigné**,
  ce qui rend ce cas diagnosticable au lieu d'être muet.
- **`avant` reste calculé et reste utilisé** (`:71`). Le relevé n'est pas
  retiré ; il descend au rang de repli.
- **`montee.rs` n'est pas touchée** (§ 1.3).
- **La protection de `:62-70` n'est PAS relâchée** : la voie C ne desserre
  aucun filtre. Elle **resserre** — d'un ensemble « tout ce qui est apparu » à
  un singleton « celle que le pilote nous a rendue ». Un moniteur physique ne
  peut pas être désigné, puisqu'il ne porte pas notre identifiant de cible.

🔴 **Ce que la voie C cesse de protéger : RIEN, et c'est sa qualité
principale.** Elle **ajoute** une hypothèse (§ 2.4), dont l'échec retombe sur
le comportement d'aujourd'hui. C'est la différence de nature avec la voie B.

---

## 4. La voie B, si la voie C était fermée

Elle ne l'est pas. Je la rédige quand même, parce que le brief la demande et
parce qu'il faut pouvoir comparer.

**Rédaction.** `creation_sortie.rs:242-246`, après l'échec du chemin ① :

```rust
// ③ RELÂCHEMENT — n'existe QUE si ① est fermée.
let deja_tenues: Vec<&String> = table.noms_de_sortie_tenus();   // n'existe pas encore
let repechees: Vec<_> = toutes.iter()
    .filter(|s| s.attachee_au_bureau
        && !prises.contains(&s.nom_sortie)
        && !deja_tenues.contains(&&s.nom_sortie)
        && placement::sortie_assez_grande((s.rect.width, s.rect.height), (largeur, hauteur))
        && placement::sortie_assez_grande((largeur, hauteur), (s.rect.width, s.rect.height)))
    .cloned().collect();
```

La double inégalité est une **égalité à `TOLERANCE_PX` près** : on n'accepte
en repêchage qu'une sortie dont la taille est celle qu'on vient de DEMANDER.
C'est la seule chose qui distingue encore, faiblement, notre sortie d'un écran
physique.

🔴 **Ce que la voie B cesse de protéger, nommément.** Exactement ce que le
commentaire de `:62-70` protège : **une fenêtre peut être posée sur l'écran
physique de la VM**, et la sortie virtuelle qu'on vient de créer devient
**orpheline** — une place sur les dix du pilote, tenue jusqu'à l'arrêt du
superviseur. Le garde de taille ne referme pas ce trou : il suffit que
l'écran de la VM ait la taille du viewport demandé, ce qui est le cas
**banal** en plein écran, et c'est précisément le scénario que `:62-70`
décrit. La voie B échange donc un défaut certain contre un défaut
conditionnel — un mauvais échange quand la voie C n'en demande aucun.

⚠️ **Elle exige en plus un accesseur neuf sur la table** (`noms_de_sortie_tenus`)
pour ne pas voler la sortie d'une session vivante, `prises` ne couvrant que ce
que la boucle courante a attribué. C'est du code de plus, dans un fichier de
plus, pour une règle plus faible.

---

## 5. La voie A — son coût réel, et sa nécessité

**Ce qu'elle est** : une sortie d'amorce créée au démarrage du superviseur et
jamais relâchée, qui absorbe la cible forcée. La première sortie *de session*
devient alors la deuxième sortie du processus — et le lot 30 a établi que la
deuxième s'attache très bien (**4 créées → 4 attachées**).

**Son coût réel, chiffré :**

| Ce que ça coûte | Combien |
| --- | --- |
| Vivier du pilote | **1 sur 10** (le vivier de dix est mesuré ; `pilote.rs:255-259` refuse au-delà) |
| Plafond réel de fenêtres | **aucun effet pratique** — les plafonds qui mordent sont **4** processus tenant une duplication et **8** encodeurs (chantier D), tous deux plus bas que 9 |
| Complexité | faible : une création au démarrage, un `Sorties` qui la tient |
| Ce qu'elle laisse ouvert | 🔴 **la règle d'appariement reste fausse** |

🔵 **Une propriété qu'il faut lui reconnaître, et qui n'est pas dans le
brief** : parce que l'amorce n'est **jamais** relâchée, le bureau ne retombe
jamais à zéro sortie, donc Windows ne refabrique **jamais** de cible forcée —
y compris quand la dernière fenêtre se ferme. La voie A est donc *cohérente*,
pas seulement un pansement de démarrage.

🔴 **Est-elle nécessaire si C tient ? NON.** Et il y a plus : **elle est
inutile si C tient**, parce que C rend le cas « la sortie remplace une cible
forcée » indiscernable du cas nominal. Poser l'amorce **en plus** de C
coûterait une place sur dix pour ne rien acheter, et donnerait un remède qu'on
ne pourrait plus voir rouge — l'amorce masquerait le défaut que C corrige, et
la recette de C ne pourrait plus être jouée. **Les deux ne doivent pas être
livrées ensemble.**

⚠️ **Elle reste le repli** si le § 7 réfute l'hypothèse de C : sa mise en œuvre
est courte, et son verdict (la 2ᵉ sortie s'attache) est **déjà mesuré**, là où
celui de C ne l'est pas.

---

## 6. Le contrôle — le test qui fige, et comment on le voit ROUGE

🔴 **UN CONTRÔLE QU'ON N'A JAMAIS VU ROUGE N'EST PAS UN CONTRÔLE**, et la règle
est en deux temps : l'exécuter, **puis provoquer l'état qu'il doit dénoncer**.

Le défaut n'est pas dans la règle CCD : il est dans la **sélection**. Un test
sur `nom_gdi_de_la_cible` seule serait vert avant comme après le remède, donc
sans valeur pour ce défaut. **La sélection doit donc devenir pure.**

### 6.1 L'extraction qui rend la rouge possible

`creation_sortie.rs` est `#[cfg(windows)]` (par `superviseur.rs:17-18`) : rien
de ce qu'il contient ne peut être éprouvé par `cargo test --workspace` sur
l'hôte. Je propose donc de sortir le **choix** — et lui seul — dans un module
pur, `agent/src/superviseur/designation.rs` (`pub mod designation;` dans
`superviseur.rs`, **non gaté**, comme `placement`) :

```rust
/// Les sorties parmi lesquelles apparier, dans l'ordre des deux chemins.
///
/// `notre_nom` : ce que la désignation a rendu, `None` si elle n'a rien rendu.
pub fn candidates(
    toutes: &[SortieDxgi],
    notre_nom: Option<&str>,
    avant: &[String],
) -> Vec<SortieDxgi> {
    if let Some(nom) = notre_nom {
        if let Some(notre) = toutes.iter().find(|s| s.attachee_au_bureau && s.nom_sortie == nom) {
            return vec![notre.clone()];
        }
    }
    toutes.iter()
        .filter(|s| s.attachee_au_bureau && !avant.contains(&s.nom_sortie))
        .cloned().collect()
}
```

⚠️ **Ne PAS le poser dans `placement.rs`** : ce fichier est à **441** lignes,
et +55 le mettrait à ~496 — la marge que ce dépôt a reperdue six fois. Un
fichier neuf, ~110 lignes avec ses tests.

### 6.2 Les trois tests, et lequel est la rouge

**① `la_sortie_qui_remplace_une_cible_forcee_est_retenue`** — *c'est la rouge.*

Le montage reproduit le relevé du lot 30, à la lettre :

```rust
let avant = vec!["\\\\.\\DISPLAY5".to_string()];   // la cible FORCÉE était déjà là
let toutes = vec![sortie("\\\\.\\DISPLAY5", 1860, 1080, true)];  // la nôtre, MÊME nom
assert_eq!(candidates(&toutes, Some("\\\\.\\DISPLAY5"), &avant).len(), 1);
```

🔴 **Comment on le voit ROUGE sur le produit d'AUJOURD'HUI** — et c'est
vérifiable sans écrire une ligne du remède : le produit d'aujourd'hui n'a pas
de paramètre `notre_nom`, il court **toujours** la branche de repli. Passer
`None` à `candidates` **est** le produit d'aujourd'hui, ligne pour ligne
(`creation_sortie.rs:242-246`). Donc :

```rust
// La ROUGE, dans le même fichier de test, sur le MÊME montage :
assert!(candidates(&toutes, None, &avant).is_empty(),
        "le produit d'avant ce lot rend un vecteur VIDE sur cette topologie \
         — c'est le défaut mesuré par le lot 30");
```

**Les deux assertions vivent dans DEUX tests séparés, jamais dans un seul** :
`expect`/`assert` s'arrête au premier échec, et ce dépôt a payé « trois
assertions dans un test ne prouvent que la première ». Le second test est le
**témoin négatif** qui rend le premier interprétable : il montre, dans le même
relevé, que le montage est bien celui qui échouait.

**② `la_designation_ne_court_pas_circuit_le_filtre_des_prises`**

```rust
let cible = placement::sortie_pour_viewport(
    &candidates(&toutes, Some("\\\\.\\DISPLAY5"), &avant),
    1860, 1080, &["\\\\.\\DISPLAY5".to_string()]);
assert!(cible.is_none());
```

Sa rouge se provoque par mutation ciblée : retirer `&& !deja_prises.contains(…)`
de `placement.rs:102`, restaurer **depuis une copie nommée** — jamais par
`git checkout --`, qui restaure HEAD et a déjà effacé du travail dans ce dépôt.

**③ `sans_designation_un_ecran_preexistant_reste_refuse`** — le garde de ce
que la voie B aurait relâché :

```rust
let avant = vec!["\\\\.\\DISPLAY1".to_string()];              // l'écran physique
let toutes = vec![sortie("\\\\.\\DISPLAY1", 3840, 2160, true)];
assert!(candidates(&toutes, None, &avant).is_empty());
```

Il fige que le repli n'a **rien** relâché. Sa rouge : c'est très exactement la
voie B — l'écrire fait passer ce test au rouge, ce qui **est** la démonstration
de ce que B cesse de protéger.

### 6.3 Ce que ces tests n'établissent PAS

🔴 **Ils n'établissent pas que `identifiant_cible` est un `id` de cible CCD.**
C'est une frontière que `cargo test` ne franchit pas : ils figent la
**sélection**, pas la **désignation**. La désignation ne se mesure que sur la
VM (§ 7), et écrire « le remède est prouvé » sur la foi de trois tests d'hôte
serait la fabrication d'une pièce.

### 6.4 Les commandes

```bash
cargo test --workspace                      # les trois tests ci-dessus
cargo check --target x86_64-pc-windows-gnu  # tout le code #[cfg(windows)], SUR L'HÔTE
scripts/build-agent-croise.sh               # l'agent.exe refabricable
```

⚠️ `cargo check --target x86_64-pc-windows-gnu` est **la seule chose qui
éprouve la moitié Win32** (`config_affichage::win`, la feature Cargo, les
signatures CCD) sans la VM. Il n'a **pas** été lancé par ce lot — aucune ligne
n'a été écrite.

---

## 7. La mesure que je DEMANDE, et que je n'ai pas jouée

🔴 **La VM ne m'appartient pas pour ce lot** : je n'ai pas touché à sa
définition libvirt, je ne l'ai pas redémarrée, je n'ai lancé aucun agent, et
je n'ai fait **aucune** sonde WinRM. Tout le § 1 et le § 2 sont de la lecture
de code et de pièces déjà versionnées.

**La mesure qui tranche la voie C**, à autoriser :

1. VM **sans** le VGA QEMU (la condition du lot 30) ;
2. lancer l'agent en superviseur, **une seule fenêtre** ;
3. dans `agent.log`, apparier trois lignes :
   - `sortie virtuelle créée id=<T> adaptateur_bas=<L> adaptateur_haut=<H>`
     (`pilote.rs:364-372`, **déjà émise aujourd'hui**) ;
   - la ligne neuve du chemin ① nommant `\\.\DISPLAYn` pour ce `(L,H,T)` ;
   - `sortie virtuelle rendue au pilote` **ABSENTE**, et la fenêtre annoncée.

🔵 **Chiffre-juge, et son témoin** : **1** fenêtre servie sur la première
sortie du processus (aujourd'hui : **0**), et le bras témoin **avec** le VGA
qui doit continuer de rendre **1** — sans quoi la mesure ne dirait pas si le
remède agit ou si le VGA agit.

⚠️ **Il n'existe aucune variable d'environnement pour désarmer le chemin ① :**
la ROUGE de cette recette est le **binaire d'avant** le remède, sur la même VM
et la même topologie. Si tu veux une rouge jouable sur le binaire d'après, il
faut une variable de banc (`SORTIE_DESIGNEE=0`, convention `=0` désarme —
celle de `PLEIN_ECRAN`), **posée par une tâche DÉDIÉE dans
`scripts/run-agent.sh`**, avec le contrôle qui vaut : lire la ligne dans le
`run-agent.ps1` **GÉNÉRÉ**. **Je ne la propose pas d'office** — c'est du code
de banc pour un remède dont le repli ② est déjà, exactement, le bras rouge.
**Dis-moi si tu la veux.**

---

## 8. Ce que ce document n'établit PAS

- **Que la voie C fonctionne.** Elle est *ouverte* — l'API existe, la feature
  existe, les deux bouts se rejoignent sur un nom GDI, et les nombres
  coïncident dans une pièce du lot 22. Elle n'est **pas mesurée**.
- **Que la rédaction compile.** Aucune ligne n'a été écrite ; ni
  `cargo test --workspace` ni `cargo check --target x86_64-pc-windows-gnu`
  n'ont été lancés par ce lot.
- **Que `montee.rs` verrait le remplacement.** C'est une prédiction de lecture
  (§ 1.3) : cette sonde n'a jamais été jouée sans le VGA.
- **Que la voie A est superflue en toutes circonstances.** Elle l'est *si C
  tient* ; elle redevient le repli si le § 7 réfute l'hypothèse de C.
- **Les tailles après édition.** `pilote.rs` est à 463 et `placement.rs` à 441
  **aujourd'hui** ; les chiffres de croissance du § 3.3 et du § 6.1 sont des
  **estimations**, et la règle du dépôt veut qu'on relève les tailles **après**
  la dernière édition. Aucune édition n'a eu lieu.
