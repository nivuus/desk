//! L'échantillonnage de la progression : à quel rythme une installation a le
//! droit de parler.
//!
//! 🔴 **C'EST UNE CONTRAINTE DE SÛRETÉ, PAS DE CONFORT.** La file montante de
//! l'agent vers la plateforme est **bornée à 32 messages**
//! (`crate::plateforme`, `FILE_EMISSION`) et **abandonne ce qui déborde en le
//! journalisant**. Une progression émise à chaque tranche de 64 Kio la
//! saturerait — un fichier de 800 Mo en produirait plus de douze mille — et
//! noierait au passage le journal partagé par le superviseur et tous ses
//! enfants. C'est la doctrine que le chantier TURN a payée le 30 juillet 2026,
//! quand une trace par `Transmit` a écrit 18 619 lignes en quelques secondes
//! sur un partage CIFS et **a fait échouer la session qu'elle mesurait** :
//! *compter ou échantillonner, jamais tracer par paquet.*
//!
//! 🔴 **LA DERNIÈRE PROGRESSION D'UNE PHASE EST TOUJOURS ÉMISE**, quel que
//! soit le cadencement. Sans cette clause, une phase qui s'achève à 3 ms de la
//! précédente émission perdrait son dernier point, et la barre du hub
//! resterait à 97 % **pour l'éternité** — un gel silencieux, c'est-à-dire le
//! mode de panne que ce dépôt combat partout ailleurs. Elle est donc un
//! **paramètre de [`doit_emettre`]**, et non une seconde fonction : un
//! appelant peut oublier d'appeler une fonction de plus, il ne peut pas
//! oublier de renseigner un argument que le compilateur exige.
//!
//! **Pur, aucun `cfg`, et l'horloge est un PARAMÈTRE** — comme
//! `agents/fraicheur.ts` et `identite/jeton.ts` le font déjà côté plateforme,
//! et pour la même raison : c'est ce qui rend la frontière de décision
//! observable **à la milliseconde près** sur l'hôte, au lieu de dépendre du
//! temps qu'un test met à s'exécuter.

use std::time::Duration;

/// Entre deux progressions d'une même phase.
///
/// **NON CALIBRÉE.** Ce qui la borne par le bas est mesuré — la file de 32 et
/// la durée d'une installation réelle —, mais **rien n'a été mesuré du confort
/// qu'elle donne** : personne n'a regardé une barre avancer à ce rythme. Elle
/// rejoint la liste que ce dépôt tient depuis `BPP_MIN` : aucune constante
/// n'y a jamais été calibrée par un jugement d'usage.
pub const PERIODE_PROGRESSION: Duration = Duration::from_secs(1);

/// La même, en millisecondes, DÉRIVÉE et non recopiée : deux nombres écrits
/// séparément divergeraient au premier changement de l'un des deux.
const PERIODE_MS: u64 = PERIODE_PROGRESSION.as_millis() as u64;

/// Cette progression doit-elle partir sur le fil ?
///
/// - `dernier_ms` vaut `None` tant que la phase n'a rien émis : **la première
///   progression d'une phase passe toujours**, sans quoi une phase courte
///   n'existerait pour personne et le hub afficherait un saut de `transfert` à
///   `reconciliation` sans rien entre les deux ;
/// - `derniere_de_la_phase` force l'émission — voir l'en-tête de ce module ;
/// - sinon, il faut au moins [`PERIODE_PROGRESSION`] depuis la dernière.
///
/// ⚠️ **UNE HORLOGE QUI RECULE N'AUTORISE RIEN.** L'écart se calcule en
/// `saturating_sub`, donc un `maintenant_ms` antérieur à `dernier_ms` rend
/// zéro et **refuse** l'émission au lieu de l'accorder. C'est le sens sûr : ce
/// que ce module protège est une file bornée, et le prix du refus — une barre
/// qui se fige un instant — est lui-même borné par la clause de fin de phase,
/// qui passe quoi qu'il arrive.
pub fn doit_emettre(dernier_ms: Option<u64>, maintenant_ms: u64, derniere_de_la_phase: bool) -> bool {
    if derniere_de_la_phase {
        return true;
    }
    match dernier_ms {
        None => true,
        Some(dernier) => maintenant_ms.saturating_sub(dernier) >= PERIODE_MS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 🔴 LA ROUGE DU CADENCEMENT. Une boucle de téléchargement appelle ce
    /// prédicat à chaque tranche ; cent tranches écrites dans la même
    /// milliseconde ne doivent produire **qu'une** émission, sans quoi la file
    /// de 32 déborde en un clin d'œil.
    #[test]
    fn cent_appels_dans_la_meme_milliseconde_ne_produisent_qu_une_emission() {
        let mut dernier = None;
        let mut emissions = 0;
        for _ in 0..100 {
            if doit_emettre(dernier, 1_000, false) {
                emissions += 1;
                dernier = Some(1_000);
            }
        }
        assert_eq!(emissions, 1);
    }

    /// 🔴 LA CLAUSE DE FIN DE PHASE, ÉPROUVÉE SEULE — sinon elle serait vraie
    /// par hasard. Le même instant, le même `dernier`, et **seul le drapeau
    /// change** : c'est ce qui prouve que c'est lui qui décide, et non le
    /// temps écoulé.
    #[test]
    fn la_derniere_progression_d_une_phase_passe_meme_a_zero_milliseconde() {
        assert!(!doit_emettre(Some(1_000), 1_000, false), "le témoin doit refuser");
        assert!(doit_emettre(Some(1_000), 1_000, true));
        // Et même une horloge qui recule ne la retient pas : une phase achevée
        // doit être annoncée quoi qu'il arrive.
        assert!(doit_emettre(Some(9_000), 1_000, true));
    }

    #[test]
    fn la_premiere_progression_d_une_phase_passe_toujours() {
        assert!(doit_emettre(None, 0, false));
        assert!(doit_emettre(None, u64::MAX, false));
    }

    /// La frontière est assiégée des deux côtés, à la milliseconde : c'est
    /// pour cela que l'horloge est un paramètre.
    #[test]
    fn la_frontiere_de_la_periode_est_assiegee_des_deux_cotes() {
        let dernier = Some(5_000);
        assert!(!doit_emettre(dernier, 5_000 + PERIODE_MS - 1, false));
        assert!(doit_emettre(dernier, 5_000 + PERIODE_MS, false));
        assert!(doit_emettre(dernier, 5_000 + PERIODE_MS + 1, false));
    }

    #[test]
    fn une_horloge_qui_recule_ne_fait_pas_emettre() {
        assert!(!doit_emettre(Some(5_000), 4_999, false));
        assert!(!doit_emettre(Some(5_000), 0, false));
    }

    /// Une phase longue émet à cadence régulière, et une seule fois par
    /// période : le compte est écrit en dur, sans quoi la boucle passerait
    /// sans rien éprouver.
    #[test]
    fn une_phase_longue_emet_une_fois_par_periode() {
        let mut dernier = None;
        let mut emissions = 0;
        // Dix secondes, échantillonnées toutes les 10 ms comme le ferait une
        // boucle d'écriture par tranches.
        for tour in 0..1_000_u64 {
            let maintenant = tour * 10;
            if doit_emettre(dernier, maintenant, false) {
                emissions += 1;
                dernier = Some(maintenant);
            }
        }
        assert_eq!(emissions, 10);
    }

    #[test]
    fn la_periode_declaree_et_sa_forme_en_millisecondes_ne_divergent_pas() {
        assert_eq!(PERIODE_MS, 1_000);
        assert_eq!(PERIODE_PROGRESSION, Duration::from_millis(PERIODE_MS));
    }
}
