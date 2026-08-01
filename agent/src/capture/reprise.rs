//! Classer un échec d'acquisition DXGI, et borner les reprises.
//!
//! **Pur à dessein.** `capture.rs` est `#![cfg(windows)]` dans son ensemble :
//! un module ENFANT n'y serait pas compilable sur l'hôte Linux, donc pas
//! testable. Ce fichier est donc déclaré en module FRÈRE dans `main.rs`
//! (`#[path = "capture/reprise.rs"] mod capture_reprise;`), hors de tout
//! `cfg` — le même montage que `windows_source/sortie.rs`, et pour la même
//! raison.
//!
//! Il ne connaît que des `i32` : les codes DXGI nus. Aucun type `windows-rs`
//! ne franchit cette frontière, sans quoi elle ne tiendrait pas.

/// `DXGI_ERROR_ACCESS_LOST`. DXGI révoque l'accès à une duplication quand la
/// topologie d'affichage change — et **la création d'une sortie virtuelle en
/// est un cas**, relevé par le sous-bloc D1 sur trois exécutions sur trois.
/// La documentation Desktop Duplication décrit cet état comme récupérable :
/// relâcher l'`IDXGIOutputDuplication` et en créer une nouvelle.
pub const ACCES_PERDU: i32 = 0x887A0026u32 as i32;

/// `DXGI_ERROR_DEVICE_REMOVED`. Le périphérique lui-même est perdu : rouvrir
/// la seule duplication ne servirait à rien. Reste définitif.
pub const DEVICE_REMOVED: i32 = 0x887A0005u32 as i32;

/// `DXGI_ERROR_WAIT_TIMEOUT`. Pas un échec : le bureau n'a simplement pas
/// changé. Traité en amont de la classification, mais nommé ici pour que le
/// test puisse vérifier qu'il n'est PAS pris pour une perte d'accès.
pub const ATTENTE_EXPIREE: i32 = 0x887A0027u32 as i32;

/// Nombre de reprises CONSÉCUTIVES tolérées.
///
/// Trois, et non une : deux fenêtres ouvertes coup sur coup produisent deux
/// remaniements de topologie rapprochés, et une reprise interrompue par la
/// suivante ne doit pas condamner la session. Au-delà, ce n'est plus une
/// rafale mais un état durable, et insister ne ferait que masquer la cause.
pub const REPRISES_MAX: u32 = 3;

pub fn est_acces_perdu(code: i32) -> bool {
    code == ACCES_PERDU
}

/// Compte les reprises CONSÉCUTIVES. Une image qui passe remet le compteur à
/// zéro : ce budget mesure une rafale, pas une usure.
pub struct BudgetReprises {
    consommees: u32,
}

impl BudgetReprises {
    pub fn nouveau() -> Self {
        Self { consommees: 0 }
    }

    /// Consomme une reprise. Rend `false` si le budget est épuisé — auquel cas
    /// rien n'est consommé et l'appelant doit abandonner.
    pub fn consommer(&mut self) -> bool {
        if self.consommees >= REPRISES_MAX {
            return false;
        }
        self.consommees += 1;
        true
    }

    /// Une image est passée : la rafale est finie.
    pub fn succes(&mut self) {
        self.consommees = 0;
    }

    pub fn consommees(&self) -> u32 {
        self.consommees
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seule_la_perte_d_acces_est_recuperable() {
        assert!(est_acces_perdu(ACCES_PERDU));
        assert!(!est_acces_perdu(DEVICE_REMOVED), "périphérique perdu : rouvrir ne sert à rien");
        assert!(!est_acces_perdu(ATTENTE_EXPIREE), "attente expirée n'est même pas un échec");
        assert!(!est_acces_perdu(0), "S_OK");
        assert!(!est_acces_perdu(0x80070057u32 as i32), "E_INVALIDARG");
    }

    #[test]
    fn le_budget_se_consomme_puis_s_epuise() {
        let mut budget = BudgetReprises::nouveau();
        for attendu in 1..=REPRISES_MAX {
            assert!(budget.consommer(), "reprise {attendu} doit être accordée");
            assert_eq!(budget.consommees(), attendu);
        }
        assert!(!budget.consommer(), "au-delà du plafond, refus");
        assert_eq!(
            budget.consommees(),
            REPRISES_MAX,
            "un refus ne consomme rien"
        );
    }

    /// Le cœur du choix : le budget mesure une RAFALE. Une session longue qui
    /// reprend une fois par heure ne doit jamais s'épuiser.
    #[test]
    fn une_image_qui_passe_remet_le_budget_a_zero() {
        let mut budget = BudgetReprises::nouveau();
        assert!(budget.consommer());
        assert!(budget.consommer());
        budget.succes();
        assert_eq!(budget.consommees(), 0);
        for _ in 0..REPRISES_MAX {
            assert!(budget.consommer(), "le plafond entier est de nouveau disponible");
        }
    }
}
