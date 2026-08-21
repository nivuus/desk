//! La QUEUE du journal d'un installeur, bornée — et pourquoi elle est PURE.
//!
//! 🔴 CE MODULE EST NÉ D'UN DÉFAUT DE CE SOUS-BLOC MÊME, TROUVÉ EN RELISANT.
//! La borne était écrite DEUX FOIS — une fois sur des octets dans
//! `execution.rs`, une fois sur une `String` dans `fil.rs` — et la seconde
//! **PANIQUAIT** : `&texte[texte.len() - N..]` sur un `&str` exige que
//! l'indice tombe sur une frontière de caractère UTF-8, sinon Rust panique.
//!
//! 🔴 ET CE CHEMIN ÉTAIT ATTEIGNABLE, ce qui est le point. `String::from_utf8_lossy`
//! **AGRANDIT** : chaque octet invalide devient un U+FFFD de TROIS octets. Un
//! journal d'installeur coupé à 64 Kio d'octets bruts peut donc rendre une
//! `String` de plus de 64 Kio — et la seconde borne, croyant n'avoir rien à
//! faire, coupait alors au milieu d'un caractère. **Un installeur qui écrit du
//! Latin-1 sur sa sortie standard aurait suffi**, et le symptôme aurait été un
//! fil d'installation qui meurt sans rapporter d'issue.
//!
//! ⚠️ LA LEÇON N'EST PAS « ATTENTION À L'UTF-8 » : c'est qu'une borne écrite
//! deux fois est une borne qui diverge. Elle est écrite ici, une fois, PURE, et
//! **les deux appelants s'en servent** — celui qui a des octets et celui qui a
//! une chaîne.

/// La queue du journal de l'installeur qu'on remonte.
///
/// ⚠️ **NON CALIBRÉE**, elle rejoint la liste que ce dépôt tient depuis
/// `BPP_MIN`.
pub const JOURNAL_MAX_OCTETS: usize = 64 * 1024;

/// La FIN d'un journal, et non sa tête.
///
/// 🔴 LA FIN, PARCE QUE C'EST LÀ QUE VIT LE MESSAGE D'ERREUR d'un installeur
/// qui a échoué. Le second membre dit `tronqué`, ce qui distingue « coupé » de
/// « vide » — sans quoi un utilisateur lirait les derniers 64 Kio en croyant
/// lire tout.
///
/// ⚠️ **UN JOURNAL VIDE EST LE CAS NORMAL**, pas un échec : la plupart des
/// installeurs Windows sont graphiques et n'écrivent rien sur les flux
/// standard. L'interface ne doit pas le présenter comme une panne.
pub fn queue(texte: &str) -> (&str, bool) {
    if texte.len() <= JOURNAL_MAX_OCTETS {
        return (texte, false);
    }
    // 🔴 ON AVANCE JUSQU'À LA PROCHAINE FRONTIÈRE DE CARACTÈRE, on ne recule
    // pas : reculer rendrait plus que la borne, et la borne est ce qu'on
    // promet. Au pire on rend trois octets de moins.
    let mut coupe = texte.len() - JOURNAL_MAX_OCTETS;
    while coupe < texte.len() && !texte.is_char_boundary(coupe) {
        coupe += 1;
    }
    (&texte[coupe..], true)
}

/// La même règle, sur des octets bruts — le cas de celui qui vient de lire un
/// fichier.
///
/// ⚠️ ELLE COUPE AVANT DE CONVERTIR, et l'ordre compte : convertir d'abord
/// obligerait à tenir en mémoire un journal d'installeur de taille inconnue,
/// que rien ne borne côté Windows.
pub fn queue_octets(octets: &[u8]) -> (String, bool) {
    if octets.len() <= JOURNAL_MAX_OCTETS {
        return (String::from_utf8_lossy(octets).into_owned(), false);
    }
    let texte = String::from_utf8_lossy(&octets[octets.len() - JOURNAL_MAX_OCTETS..]).into_owned();
    // ⚠️ ON REBORNE APRÈS LA CONVERSION : `from_utf8_lossy` AGRANDIT — un octet
    // invalide devient un U+FFFD de trois octets —, si bien que couper
    // `JOURNAL_MAX_OCTETS` octets bruts peut rendre une chaîne PLUS LONGUE que
    // la borne. C'est précisément ce que le second appelant supposait faux.
    let (queue, _) = queue(&texte);
    (queue.to_string(), true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_journal_court_passe_entier_et_n_est_pas_tronque() {
        assert_eq!(queue("bonjour"), ("bonjour", false));
        assert_eq!(queue(""), ("", false));
        let pile = "a".repeat(JOURNAL_MAX_OCTETS);
        assert_eq!(queue(&pile), (pile.as_str(), false));
    }

    #[test]
    fn un_journal_long_rend_sa_FIN_et_se_declare_tronque() {
        let long = format!("{}FIN", "a".repeat(JOURNAL_MAX_OCTETS));
        let (q, tronque) = queue(&long);
        assert!(tronque);
        assert!(q.ends_with("FIN"), "c'est la FIN qu'on garde, pas la tête");
        assert!(q.len() <= JOURNAL_MAX_OCTETS);
    }

    /// 🔴 LA ROUGE DU DÉFAUT QUI A FAIT NAÎTRE CE MODULE.
    ///
    /// L'implémentation d'origine faisait `&texte[texte.len() - N..]` sans
    /// vérifier la frontière de caractère : sur un journal dont l'octet à cette
    /// position est au milieu d'un caractère multi-octet, **Rust PANIQUE**. Le
    /// fil d'installation serait mort sans rapporter d'issue, et le hub aurait
    /// affiché « en cours » pour l'éternité.
    #[test]
    fn ne_panique_JAMAIS_au_milieu_d_un_caractere_multi_octet() {
        // « é » fait deux octets ; en répéter assez place la coupe au milieu
        // d'un caractère une fois sur deux, quel que soit le rembourrage.
        for rembourrage in 0..4 {
            let texte = format!("{}{}", "x".repeat(rembourrage), "é".repeat(JOURNAL_MAX_OCTETS));
            let (q, tronque) = queue(&texte);
            assert!(tronque);
            assert!(q.len() <= JOURNAL_MAX_OCTETS);
            // Et le résultat est du texte VALIDE : c'est ce que le typage
            // garantit, et ce que la panique remplaçait.
            assert!(q.chars().all(|c| c == 'é' || c == 'x'));
        }
    }

    /// 🔴 LE CHEMIN QUI REND LA PANIQUE ATTEIGNABLE : `from_utf8_lossy` AGRANDIT.
    ///
    /// Chaque octet invalide devient un U+FFFD de trois octets. Couper
    /// `JOURNAL_MAX_OCTETS` octets BRUTS peut donc rendre une chaîne bien plus
    /// longue que la borne — et c'est exactement ce que l'appelant supposait
    /// faux quand il rebornait sans vérifier la frontière.
    #[test]
    fn from_utf8_lossy_AGRANDIT_donc_on_reborne_apres_la_conversion() {
        // Que des octets invalides : chacun coûte trois octets une fois converti.
        let octets = vec![0xFFu8; JOURNAL_MAX_OCTETS + 10];
        let brut = String::from_utf8_lossy(&octets[octets.len() - JOURNAL_MAX_OCTETS..]);
        assert!(
            brut.len() > JOURNAL_MAX_OCTETS,
            "la conversion doit AGRANDIR, sinon ce test n'éprouve rien"
        );
        let (q, tronque) = queue_octets(&octets);
        assert!(tronque);
        assert!(
            q.len() <= JOURNAL_MAX_OCTETS,
            "la borne est ce qu'on promet : {} octets",
            q.len()
        );
    }
}
