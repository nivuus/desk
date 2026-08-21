//! Ce que la plateforme demande d'INSTALLER, et pourquoi cela ne passe pas par
//! [`super::Ordre`].
//!
//! 🔴 UNE SECONDE FILE, ET NON UNE TROISIÈME VARIANTE D'`Ordre` : C'EST LE
//! CONSOMMATEUR QUI DÉCIDE, PAS LE GOÛT.
//!
//! `Ordre` est drainé par la boucle de découverte, qui tourne sur **un vrai
//! fil COM dédié** (`apps.rs` : « l'appartement COM appartient à SON fil ;
//! `IShellLinkW` et `ShellExecuteExW` doivent tous deux courir sur celui qui a
//! appelé `CoInitializeEx` »). Une installation, elle, TÉLÉCHARGE plusieurs
//! centaines de mégaoctets en `tokio` puis attend un processus pendant des
//! minutes. La poser dans `Ordre` ferait faire ce travail au fil COM, qui
//! cesserait alors de réconcilier — c'est-à-dire que le catalogue se figerait
//! **pendant exactement l'installation dont on attend qu'il rende compte**.
//!
//! ⚠️ **L'EN-TÊTE D'`ordre.rs` ARGUMENTE CONTRE LES DEUX FILES, et il a raison
//! du risque qu'il nomme** : « deux files obligeraient à interroger les deux à
//! chaque tour, avec le risque qu'un ajout futur en oublie une ». C'est vrai
//! d'un consommateur unique — et il n'y en a pas un ici, il y en a deux. Le
//! risque est traité autrement : cette file n'a **qu'un seul type de message**,
//! donc rien à oublier ; et `Ordre` reste exhaustif pour son propre
//! consommateur, exactement comme G2 l'a écrit.
//!
//! ⚠️ **Sa phrase « les DEUX messages descendants » est devenue fausse** — il
//! y en a trois depuis le sous-bloc G3. Annotée à sa place.
//!
//! ⚠️ La décision D11 du plan de G3 argumentait contre « un tuple élargi » :
//! `ordres()` rendait alors un `(String, String)`. G2 l'a remplacé par un enum
//! entre-temps. **La prémisse a changé ; la conclusion tient pour une autre
//! raison**, celle qui est écrite ci-dessus.

/// L'ordre d'installer un logiciel téléversé.
///
/// ⚠️ IL NE PORTE AUCUN OCTET, seulement une **URL** : le canal est en JSON,
/// il porte le battement de cœur, et une tranche de 8 Mio y coûterait +33 % en
/// base64 tout en bloquant ce battement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installation {
    pub id: String,
    pub url: String,
    pub nom: String,
    pub taille: u64,
    pub sha256: String,
}
