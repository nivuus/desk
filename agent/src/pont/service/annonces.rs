//! **La QUATRIÈME famille du protocole** : les annonces qui REMONTENT, du
//! navigateur vers le pont — `Bonjour` et `Rafraichir`.
//!
//! # Pourquoi cette extraction, et pourquoi MAINTENANT
//!
//! `service.rs` était à **463** lignes, marge **37**, après que F5 y a posé son
//! aiguillage et ces deux fonctions (+122). La revue transverse qui clôt toute
//! branche de ce dépôt **ajoute du commentaire** — S3 y a mis 48 lignes réparties
//! sur neuf fichiers, et S2 a vu la marge d'un fichier tomber de 30 à 17 par ce
//! seul geste. **L'extraction se fait AVANT l'addition, jamais après**, et
//! `CLAUDE.md` interdit nommément de compresser pour repasser sous la ligne.
//!
//! ⚠️ **L'AIGUILLAGE, LUI, RESTE DANS [`super`]**, et ce n'est pas un oubli :
//! il doit courir **avant `table.resoudre`**, et le déplacer ici ferait de ce
//! module l'endroit où l'on croit qu'il vit. *Ce module porte ce que les
//! annonces FONT ; le fait qu'elles soient aiguillées tôt est une propriété de
//! `traiter`, et elle se lit là.*

use super::Etat;
use proto::fichiers::entetes;

/// L'annonce `Rafraichir` : le pont oublie ce qu'il croyait savoir.
///
/// **Deux caches, et ils ne sont pas à nous tous les deux.** Le premier est le
/// nôtre ([`crate::pont::cache`]) ; le second appartient à ProjFS — le **cache
/// négatif**, qui mémorise les chemins dont le fournisseur a dit qu'ils
/// n'existaient pas. Vider l'un sans l'autre laisserait un fichier créé sur le
/// poste local rester introuvable, **par ProjFS et non par nous**.
///
/// 🔵 **`PrjClearNegativePathCache` GAGNE ICI SON PREMIER APPELANT DE
/// PRODUCTION.** Elle est chargée depuis F1, et
/// `grep -rn vider_cache_negatif agent/src/` ne rendait jusqu'ici que sa
/// déclaration. **R7 se referme d'UNE entrée sur cinq** — pas « R7 est fermé » :
/// `PrjDeleteFile` et trois autres restent sans jumeau `PRJ_*_CB`.
///
/// 🔵 **Et son résultat est TRACÉ, ce qui rend le cache négatif observable pour
/// la première fois.** F4 n'a pu en mesurer que le différentiel, **nul**, parce
/// qu'aucun sondage de l'Explorateur n'atteignait le fournisseur. *Si ce nombre
/// est toujours zéro, c'est un FAIT et non une panne* — et il faudra le
/// rapporter comme F4 a rapporté son différentiel nul.
pub(super) fn rafraichir(etat: &Etat) {
    let memorises = match etat.cache.lock() {
        Ok(mut cache) => {
            let n = cache.taille();
            cache.vider();
            n
        }
        Err(_) => 0,
    };
    let purgees = vider_le_cache_negatif(etat);
    tracing::info!(
        repertoires_oublies = memorises,
        cache_negatif_purge = purgees,
        cache_arme = etat.cache_arme,
        "rafraichissement demande par le navigateur"
    );
}

/// Vide le cache négatif de ProjFS et rend le nombre d'entrées purgées.
///
/// ⚠️ **`None` se distingue de `Some(0)` dans la trace** : le premier dit que
/// le contexte de virtualisation n'était pas là, le second que le cache était
/// vide. *Les confondre ferait lire une absence de mesure comme une mesure
/// nulle* — c'est le piège que ce dépôt a payé sur `grep` sans `-a`.
fn vider_le_cache_negatif(etat: &Etat) -> i64 {
    let Some(contexte) = etat.contexte() else {
        return -1;
    };
    let mut total: u32 = 0;
    // SÛRETÉ : le contexte vient de `PrjStartVirtualizing` et vit tant que la
    // virtualisation tourne ; `total` est une pile locale valide pour l'appel.
    let hr = unsafe { (etat.projfs.vider_cache_negatif)(contexte.0, &mut total) };
    if hr.is_err() {
        tracing::warn!(code = hr.0, "PrjClearNegativePathCache a echoue");
        return -1;
    }
    i64::from(total)
}

/// L'annonce `Bonjour` : le navigateur dit sur quelle racine il est monté.
///
/// **Le pont n'a rien poussé jusqu'ici**, et c'est tout le point : la reprise
/// des écritures dues a quitté `Fil::demarrer` pour attendre cette annonce.
pub(super) fn bonjour(etat: &Etat, entete: &[u8]) {
    let annonce = match serde_json::from_slice::<entetes::Bonjour>(entete) {
        Ok(annonce) => annonce,
        Err(erreur) => {
            // 🔴 **Un `warn!`, jamais un `debug!`.** Une annonce illisible veut
            // dire qu'aucune écriture due ne partira jamais — un silence, donc
            // pire que les trente secondes que F2 a mesurées.
            tracing::warn!(%erreur, "annonce Bonjour illisible : aucune ecriture due ne partira");
            return;
        }
    };
    tracing::info!(
        racine = %annonce.racine,
        forcer = annonce.forcer,
        "bonjour du navigateur"
    );
    let _ = etat.vers_ecriture.send(crate::pont::ecriture::fil::Ordre::Bonjour {
        racine: annonce.racine,
        forcer: annonce.forcer,
    });
}
