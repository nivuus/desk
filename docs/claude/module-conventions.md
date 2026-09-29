# Conventions de code — convention de module enfant (`#[path]` ou racine nue)

> Extrait de [`CLAUDE.md`](../../CLAUDE.md) (lignes 135-228 de l'original), déplacé tel quel. Liens relatifs à ce dossier.

### Convention de module enfant : `#[path]` chez le parent, ou racine nue

**Tranché le 7 août 2026 (tâche 17, sous-bloc D10), après que le sous-bloc
précédent a relevé `survie_verdict.rs` comme une « déviation » puis s'est
lui-même trompé en la corrigeant (voir le leg n°9 de D9, plus bas). Ce dépôt
avait deux conventions pour un module hors du `#[cfg(windows)]` de son parent
logique, sans jamais avoir écrit la règle qui les départage.

**Portée de la règle** : elle ne s'applique QU'aux modules qu'on extrait d'un
fichier `#[cfg(windows)]` (ou autrement non portable) pour que leur logique
*pure* compile et se teste sur l'hôte Linux, et qui doivent de ce fait devenir
des **frères de premier niveau** de ce parent, déclarés dans `main.rs`. Un
module `#[cfg(windows)]` ordinaire qui n'a pas besoin d'exister sur l'hôte
reste un enfant normal, déclaré par un simple `mod` **à l'intérieur** de son
parent gaté (`capture.rs::mod enumeration;`, `capture.rs::mod types;`,
`windows_source.rs::mod redimensionnement;`) : il ne se pose jamais la
question ci-dessous, faute d'avoir jamais besoin de sortir de l'arbre de son
parent. Est également hors de portée l'usage de `#[path]` pour scinder un
module de *tests* trop long À L'INTÉRIEUR d'un fichier par ailleurs portable
(`superviseur/table.rs` déclare ainsi `#[path = "table/tests.rs"] mod tests;`
et `#[path = "table/tests_relance.rs"] mod tests_relance;`, tous deux
`#[cfg(test)]`, tous deux internes à `table.rs`, sans rapport avec une
frontière `#[cfg(windows)]`) : c'est le même mécanisme Rust, employé pour une
raison différente (la règle des 500 lignes), et il ne suit pas la convention
ci-dessous.

**La règle, pour les modules dans cette portée : le NOM du module tranche.**

- **Le nom du module s'écrit `<parent>_<enfant>`**, où `<parent>` nomme un
  module de premier niveau existant (déclaré dans `main.rs`) : le fichier
  reste physiquement chez ce parent (`src/<parent>/<enfant>.rs`), et se
  déclare dans `main.rs` par
  `#[path = "<parent>/<enfant>.rs"] mod <parent>_<enfant>;` — c'est la
  déclaration qui franchit le `#[cfg(windows)]` du parent, le fichier
  physique, lui, n'a pas bougé de sous son parent. Exemples :
  `capture_reprise` (`capture/reprise.rs`), `windows_source_sortie`
  (`windows_source/sortie.rs`), `windows_source_telemetrie`
  (`windows_source/telemetrie.rs`).
  ⚠️ **Si plusieurs modules de premier niveau sont chacun un préfixe valide du
  nom** (cas non encore rencontré, mais qui existe dès aujourd'hui :
  `windows_source_sortie` est LUI-MÊME un module de premier niveau depuis D1,
  donc un futur `windows_source_sortie_conversion` préfixerait à la fois
  `windows_source` et `windows_source_sortie`), **c'est le préfixe le PLUS
  LONG — le plus spécifique — qui l'emporte.** Le fichier physique suit :
  `windows_source_sortie_conversion` se rangerait sous
  `windows_source/sortie/conversion.rs` (enfant de `windows_source_sortie`,
  lui-même à `windows_source/sortie.rs`), pas sous
  `windows_source/sortie_conversion.rs`. Cette clause ne change le
  classement d'aucun des six cas relevés ci-dessous : aucun n'a de second
  préfixe candidat plus court.
  **Une égalité de longueur entre deux préfixes candidats ne peut pas se
  produire** : un préfixe valide s'arrête toujours sur une frontière de
  tiret bas (`<parent>_`). Si deux noms de module de premier niveau
  DIFFÉRENTS étaient chacun un préfixe de la MÊME longueur du nom à ranger,
  ils seraient la même sous-chaîne — donc le même nom. L'égalité est
  structurellement exclue, pas seulement absente des cas rencontrés à ce
  jour ; il n'y a donc rien à trancher au-delà de « le plus long l'emporte ».
- **Le nom du module se comprend SANS référence à un parent** — il ne porte
  le préfixe d'aucun module de premier niveau existant (`geometry`,
  `sortie_dxgi`, `survie_verdict`) : il vit à la racine nue, `mod <nom>;`
  ordinaire dans `main.rs`, fichier `src/<nom>.rs`. **La profondeur du module
  dont on l'extrait ne change rien** : `survie_verdict` vient de
  `diagnostics::multifenetre::mode_sortie::persistance`, quatre niveaux plus
  bas que `main.rs`, et n'a pourtant aucun nom de parent court et unique à
  préfixer — la règle le range à la racine comme `geometry` et `sortie_dxgi`,
  extraits pour la même raison (compiler sur l'hôte) d'un parent tout aussi
  gaté (`capture.rs`, `window.rs`).

**Cas du parent qui n'existe pas encore quand on écrit le module** : la règle
se résout mécaniquement vers la racine nue — un nom ne peut préfixer un
module de premier niveau qui n'est pas encore déclaré dans `main.rs`. **Effet
de bord non traité** : si un module homonyme du préfixe apparaît plus tard
(un futur `mod windows_source_sortie_conversion` créé avant que
`windows_source_sortie` existe, par exemple), rien ne force à re-hisser le
premier sous le second après coup — aucune règle de re-hissage n'est posée
ici, à écrire le jour où le cas se présente réellement.

**Vérifiée sur les six cas existants au 7 août 2026**
(`grep -rn '#\[path' agent/src/`, `ls agent/src/*.rs`, depuis `agent/`) : les
trois noms préfixés sont TOUS déclarés par `#[path]` chez leur parent, les
trois noms autonomes sont TOUS à la racine nue. **Aucune exception**, y
compris sous la clause du préfixe le plus long ci-dessus.
`survie_verdict.rs` s'y conforme déjà — il n'a jamais eu besoin de bouger.

**Preuve, portée ici plutôt que dans un rapport de tâche gitignoré, que la
clause du préfixe le plus long ne change le classement d'aucun des trois
noms préfixés** : pour chacun, aucun AUTRE module de premier niveau n'en est
un préfixe plus court et valide.
- `capture_reprise` : seul `capture` le préfixe.
- `windows_source_sortie` : seul `windows_source` le préfixe.
- `windows_source_telemetrie` : seul `windows_source` le préfixe —
  `windows_source_sortie` n'en est PAS un préfixe : le nom continue par
  `_telemetrie`, pas par `_sortie`.
