//! Tests d'hôte des deux SEULES parties pures de ce module.
//!
//! ⚠️ Le reste — la connexion, l'écriture, la lecture de la réponse — est
//! `#[cfg(windows)]` par son parent et n'est vérifié que par la compilation
//! croisée et par la recette. C'est dit plutôt que dissimulé.

use super::*;

/// 🔴 TLS EST REFUSÉ PAR SON NOM, JAMAIS TENTÉ EN CLAIR.
#[test]
fn tls_est_refuse_explicitement() {
    for url in ["wss://plateforme.exemple:443", "https://plateforme.exemple"] {
        let erreur = base_http(url).expect_err("TLS doit être refusé");
        assert!(
            erreur.to_string().contains("AUCUNE pile TLS"),
            "le refus doit NOMMER sa cause : {erreur}"
        );
    }
}

#[test]
fn l_autorite_se_derive_de_l_url_du_canal() {
    assert_eq!(base_http("ws://192.168.3.1:8080").unwrap(), "192.168.3.1:8080");
    assert_eq!(base_http("http://127.0.0.1:8080/").unwrap(), "127.0.0.1:8080");
    // Un chemin éventuel est retiré : on ne garde que l'autorité.
    assert_eq!(base_http("ws://hote:9/base/x").unwrap(), "hote:9");
    assert_eq!(base_http("  ws://hote:9  ").unwrap(), "hote:9");
    // 🔴 UNE URL SANS HÔTE EST REFUSÉE. Sans le retrait du schéma AVANT la
    // coupe des barres finales, `ws://` devenait `ws:` — pris pour une
    // autorité, donc une connexion qui échouait loin de sa cause. Trouvé par
    // l'exécution, pas par la relecture.
    assert!(base_http("ws://").is_err());
    assert!(base_http("").is_err());
    assert!(base_http("http:///chemin").is_err());
}

/// 🔴 LE STATUT EST LU, PAS SUPPOSÉ. Sans cette lecture, un `413` ou un
/// `400 {refus:'empreinte'}` passerait pour un succès et l'agent
/// retéléverserait indéfiniment sans jamais savoir pourquoi.
#[test]
fn le_statut_se_lit_dans_la_premiere_ligne() {
    assert_eq!(statut_http(b"HTTP/1.1 204 No Content\r\n\r\n"), Some(204));
    assert_eq!(statut_http(b"HTTP/1.1 413 Payload Too Large\r\n\r\n{}"), Some(413));
    assert_eq!(statut_http(b"HTTP/1.1 400 Bad Request\r\n"), Some(400));
    // Une réponse qui n'en est pas une ne rend PAS un statut de complaisance.
    assert_eq!(statut_http(b""), None);
    assert_eq!(statut_http(b"pas du http\r\n"), None);
    assert_eq!(statut_http(b"HTTP/1.1\r\n"), None);
}
