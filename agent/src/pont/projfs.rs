//! La racine de virtualisation ProjFS. **`#[cfg(windows)]`, et appelé par le
//! SYSTÈME** : aucun test d'hôte n'est possible ici, et c'est déclaré, pas
//! contourné (spec §4.4). La seule compensation est qu'il soit mince — il
//! traduit, il ne décide pas ; toute décision qui peut vivre dans un module
//! pur y vit.
//!
//! ⚠️ **Squelette de la tâche 12** : il ne porte encore que le chargement des
//! treize entrées. La racine, la virtualisation et les rappels arrivent en
//! tâche 13.

pub mod chargement;
