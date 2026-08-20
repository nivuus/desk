//! Le magasin d'icônes de l'agent : des octets ADRESSÉS PAR LEUR CONTENU.
//!
//! **PUR, sans aucun `cfg`.** Il ne connaît ni Windows, ni COM, ni le système
//! de fichiers : il tient une table `empreinte -> octets` pour le catalogue
//! COURANT, et il sait dire lesquelles d'un ensemble annoncé manquent à un
//! autre.
//!
//! 🔴 POURQUOI L'ADRESSAGE PAR CONTENU PAIE, ET CE N'EST PAS UNE CONJECTURE.
//! Mesuré le 20 août 2026 sur le corpus réel de la VM de développement :
//! **153 applications rendent 99 PNG DISTINCTS**, soit **54 téléversements
//! évités (35,3 %)**. Douze empreintes sont partagées, dont une par **vingt-sept**
//! applications — un même `runcmdu.exe` visé par vingt-sept raccourcis
//! d'arguments différents. La décision D4 de la spécification sépare bien ces
//! vingt-sept APPLICATIONS ; elles partagent UNE icône, et c'est exactement ce
//! que ce module existe pour ne pas payer vingt-sept fois.
//!
//! ⚠️ **Le poids DÉDUPLIQUÉ n'a PAS été mesuré** : la sonde somme les 153 PNG
//! (4 576 398 octets, 29 911 o/icône), jamais les 99 distincts. Ne pas le
//! déduire d'une règle de trois — les icônes n'ont pas la même taille.

use std::collections::{BTreeMap, BTreeSet};

use crate::apps::sha256;

/// L'empreinte SHA-256 d'un PNG, en hexadécimal minuscule.
///
/// 🔴 C'EST L'EMPREINTE DES OCTETS PNG, ET NON CELLE DES PIXELS, et le maillon
/// suivant est ce qui décide. La plateforme RECALCULE l'empreinte de ce
/// qu'elle reçoit (« aucun saut ne fait confiance au précédent ») : adresser
/// par les PIXELS l'obligerait à DÉCODER le PNG pour vérifier, c'est-à-dire à
/// embarquer un décodeur PNG en TypeScript — une dépendance de production
/// neuve, que ce sous-bloc refuse. Adresser par les octets rend la
/// vérification exacte et gratuite : `sha256(corps) === :sha256`.
///
/// ⚠️ **LE PRIX DE CE CHOIX EST LA DÉTERMINATION DE L'ENCODEUR**, et il est
/// nommé. Si l'encodeur n'écrivait pas deux fois les mêmes octets pour la même
/// image — un chunk `tIME`, un `tEXt` de logiciel —, l'empreinte changerait à
/// chaque réconciliation et l'agent retéléverserait tout, indéfiniment. **Ce
/// n'est PAS une porte éliminatoire** : le catalogue resterait juste et les
/// icônes resteraient servies ; seul le coût monterait. Le remède est nommé
/// d'avance et vit **dans ce module-ci, qui est pur** — n'empreindre que les
/// chunks `IHDR`/`PLTE`/`IDAT`/`IEND`, en écartant les auxiliaires.
///
/// 🔵 AUCUNE LIGNE DE CRYPTOGRAPHIE NEUVE : `apps::sha256` est déjà écrit,
/// pur, et éprouvé sur les vecteurs de réponse connue de FIPS 180-4.
pub fn empreinte(png: &[u8]) -> String {
    sha256::hex(png)
}

/// Les octets du catalogue COURANT, une entrée par empreinte distincte.
#[derive(Debug, Default)]
pub struct Magasin {
    par_empreinte: BTreeMap<String, Vec<u8>>,
}

impl Magasin {
    pub fn new() -> Self {
        Self::default()
    }

    /// Ajoute un PNG et rend son empreinte. **Idempotent.**
    ///
    /// 🔴 DEUX AJOUTS DU MÊME CONTENU NE FONT QU'UNE ENTRÉE, et c'est
    /// l'essentiel : sur ce corpus, l'accumulation coûterait 153 entrées là où
    /// 99 suffisent.
    pub fn ajouter(&mut self, png: Vec<u8>) -> String {
        let e = empreinte(&png);
        self.par_empreinte.entry(e.clone()).or_insert(png);
        e
    }

    pub fn contient(&self, empreinte: &str) -> bool {
        self.par_empreinte.contains_key(empreinte)
    }

    pub fn octets(&self, empreinte: &str) -> Option<&[u8]> {
        self.par_empreinte.get(empreinte).map(Vec::as_slice)
    }

    pub fn empreintes(&self) -> BTreeSet<String> {
        self.par_empreinte.keys().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.par_empreinte.len()
    }

    pub fn is_empty(&self) -> bool {
        self.par_empreinte.is_empty()
    }

    /// 🔴 JETTE TOUT LE CONTENU PRÉCÉDENT. Le magasin porte le catalogue
    /// COURANT, jamais l'histoire : fusionner le ferait croître sans terme
    /// d'une réconciliation à l'autre, sur un processus qui vit des jours.
    pub fn remplacer(&mut self, neuf: Magasin) {
        self.par_empreinte = neuf.par_empreinte;
    }
}

/// Celles des `annoncees` que `connues` ne porte pas, **dans l'ordre
/// d'annonce** et sans doublon.
///
/// ⚠️ **ELLE N'A AUCUN APPELANT DE PRODUCTION DANS L'AGENT, ET C'EST DÉCLARÉ
/// PLUTÔT QUE DISSIMULÉ.** C'est la PLATEFORME qui décide ce qui lui manque —
/// en interrogeant son DISQUE, jamais une table —, et l'agent ne fait
/// qu'honorer la liste qu'elle lui pousse. Cette fonction est le jumeau
/// HÔTE-TESTABLE de cette règle : elle existe pour que la règle soit éprouvée
/// là où elle est pure, et pour que le jour où l'agent devra filtrer lui-même,
/// il n'ait pas à la réécrire.
///
/// ⚠️ Ce dépôt n'a pas de doctrine sur le code orphelin — le sous-bloc D10 a
/// SUPPRIMÉ `taille_compatible` et CONSERVÉ `rafraichir_taille_sortie` sans
/// énoncer de règle. Le choix est fait ici dans le sens de la conservation, et
/// il est écrit.
///
/// C'est la règle que la plateforme applique aussi, écrite une fois du côté où
/// elle est PURE.
///
/// ⚠️ L'ORDRE D'ANNONCE EST PRÉSERVÉ plutôt que trié : c'est celui du
/// catalogue, donc celui dans lequel l'utilisateur verra les icônes arriver.
pub fn manquantes(annoncees: &[String], connues: &BTreeSet<String>) -> Vec<String> {
    let mut vues = BTreeSet::new();
    annoncees
        .iter()
        .filter(|e| !connues.contains(*e) && vues.insert((*e).clone()))
        .cloned()
        .collect()
}

#[cfg(test)]
#[path = "magasin/tests.rs"]
mod tests;
