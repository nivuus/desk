//! Host tests of the ONLY two pure parts of this module.
//!
//! ⚠️ The rest — connecting, writing, reading the response — is
//! `#[cfg(windows)]` through its parent and is only checked by the
//! cross-compilation and by the acceptance run. It is said rather than hidden.

use super::*;

/// 🔴 TLS IS REFUSED BY NAME, NEVER ATTEMPTED IN CLEAR TEXT.
#[test]
fn tls_est_refuse_explicitement() {
    for url in ["wss://plateforme.exemple:443", "https://plateforme.exemple"] {
        let error = base_http(url).expect_err("TLS must be refused");
        assert!(
            error.to_string().contains("NO TLS stack"),
            "the refusal must NAME its cause: {error}"
        );
    }
}

#[test]
fn l_autorite_se_derive_de_l_url_du_canal() {
    assert_eq!(
        base_http("ws://192.168.3.1:8080").unwrap(),
        "192.168.3.1:8080"
    );
    assert_eq!(
        base_http("http://127.0.0.1:8080/").unwrap(),
        "127.0.0.1:8080"
    );
    // Any path is removed: only the authority is kept.
    assert_eq!(base_http("ws://hote:9/base/x").unwrap(), "hote:9");
    assert_eq!(base_http("  ws://hote:9  ").unwrap(), "hote:9");
    // 🔴 A URL WITHOUT A HOST IS REFUSED. Without removing the scheme BEFORE
    // cutting the trailing slashes, `ws://` became `ws:` — taken for an
    // authority, hence a connection that failed far from its cause. Found by
    // running it, not by re-reading.
    assert!(base_http("ws://").is_err());
    assert!(base_http("").is_err());
    assert!(base_http("http:///chemin").is_err());
}

/// 🔴 THE STATUS IS READ, NOT ASSUMED. Without this read, a `413` or a
/// `400 {refus:'empreinte'}` would pass for a success and the agent
/// would re-upload indefinitely without ever knowing why.
#[test]
fn the_status_is_read_from_the_first_line() {
    assert_eq!(statut_http(b"HTTP/1.1 204 No Content\r\n\r\n"), Some(204));
    assert_eq!(
        statut_http(b"HTTP/1.1 413 Payload Too Large\r\n\r\n{}"),
        Some(413)
    );
    assert_eq!(statut_http(b"HTTP/1.1 400 Bad Request\r\n"), Some(400));
    // A response that is not one does NOT return a convenient status.
    assert_eq!(statut_http(b""), None);
    assert_eq!(statut_http(b"pas du http\r\n"), None);
    assert_eq!(statut_http(b"HTTP/1.1\r\n"), None);
}
