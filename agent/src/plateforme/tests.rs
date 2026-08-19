//! Les tests du client de canal, contre un VRAI serveur WebSocket local.
//!
//! Extraits dans un fichier voisin sur le précédent de
//! `superviseur/table.rs` : le client tient déjà 285 lignes, et ces tests en
//! ajoutent autant. Même mécanisme, même raison.
//!
//! 🔴 **LA COUPURE EST RÉELLE, ET C'EST TOUT L'INTÉRÊT.** Le faux canal lâche
//! son socket sans trame de fermeture — un câble arraché, pas un `Close`
//! poli. Un test qui n'exercerait que le chemin nominal ne dirait RIEN de la
//! reprise, qui est le seul comportement neuf de ce fichier.

use super::*;

use tokio::net::TcpListener;
use tokio::sync::mpsc;

/// Ce que le faux canal joue sur une connexion donnée.
enum Scenario {
    /// Enrôle, puis COUPE net.
    EnroleEtCoupe(&'static str),
    /// Enrôle et tient la connexion ouverte indéfiniment.
    EnroleEtTient(&'static str),
    /// Refuse, avec son motif, puis ferme.
    Refuse(MotifCanal),
}

/// Un faux canal `/agent`. Rend son URL et la file des messages
/// d'enrôlement reçus — un par connexion, ce qui rend le NOMBRE de
/// connexions observable, et donc la reprise assertable.
async fn faux_canal(mut scenarios: Vec<Scenario>) -> (String, mpsc::UnboundedReceiver<String>) {
    let ecoute = TcpListener::bind("127.0.0.1:0").await.expect("écoute locale");
    let port = ecoute.local_addr().expect("adresse locale").port();
    let (tx, rx) = mpsc::unbounded_channel();
    scenarios.reverse();
    tokio::spawn(async move {
        loop {
            let Ok((flux, _)) = ecoute.accept().await else { return };
            let scenario = scenarios.pop();
            let tx = tx.clone();
            tokio::spawn(async move {
                let mut ws = tokio_tungstenite::accept_async(flux)
                    .await
                    .expect("montée WebSocket");
                if let Some(Ok(Message::Text(texte))) = ws.next().await {
                    let _ = tx.send(texte);
                }
                match scenario {
                    Some(Scenario::EnroleEtCoupe(prefixe)) => {
                        envoyer_enrole(&mut ws, prefixe).await;
                        // Laisser l'octet partir avant d'arracher le câble :
                        // sans cette pause, le test mesurerait une course de
                        // TCP et non la reprise.
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        drop(ws);
                    }
                    Some(Scenario::EnroleEtTient(prefixe)) => {
                        envoyer_enrole(&mut ws, prefixe).await;
                        std::future::pending::<()>().await;
                    }
                    Some(Scenario::Refuse(motif)) => {
                        let texte = serde_json::to_string(&DepuisLaPlateforme::refus(motif))
                            .expect("sérialisation du refus");
                        let _ = ws.send(Message::Text(texte)).await;
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        drop(ws);
                    }
                    // Connexion en trop : le test l'observe par la file, et
                    // c'est justement ce qu'un refus de version ne doit
                    // jamais produire.
                    None => std::future::pending::<()>().await,
                }
            });
        }
    });
    (format!("ws://127.0.0.1:{port}"), rx)
}

async fn envoyer_enrole<S>(ws: &mut tokio_tungstenite::WebSocketStream<S>, prefixe: &str)
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let texte = serde_json::to_string(&DepuisLaPlateforme::enrole(prefixe, "jeton-jwt", 1_787_136_774_000))
        .expect("sérialisation de l'enrôlement");
    ws.send(Message::Text(texte)).await.expect("envoi de l'enrôlement");
}

/// Attend que l'identité prenne une valeur différente de celle déjà lue.
async fn prochain_prefixe(canal: &mut Canal) -> String {
    loop {
        canal
            .identite
            .changed()
            .await
            .expect("le fil de reprise a renoncé au lieu de reprendre");
        let courante = canal.identite.borrow_and_update().clone();
        if let Some(identite) = courante {
            return identite.prefixe;
        }
    }
}

#[test]
fn l_url_du_canal_ne_double_jamais_la_barre() {
    assert_eq!(url_du_canal("ws://h:8080"), "ws://h:8080/agent");
    assert_eq!(url_du_canal("ws://h:8080/"), "ws://h:8080/agent");
}

/// 🔴 LE TEST DE LA REPRISE. Mutation qui le rougit : remplacer la boucle de
/// `ouvrir` par un seul appel à `une_session` — l'agent perdrait son canal à
/// la première coupure, `vu_a` cesserait d'avancer, et la plateforme
/// déclarerait la VM `injoignable` **définitivement**, alors que le réseau
/// est revenu depuis longtemps.
#[tokio::test]
async fn une_coupure_reelle_du_socket_fait_reprendre_le_canal() {
    let (url, mut connexions) = faux_canal(vec![
        Scenario::EnroleEtCoupe("PREMIER"),
        Scenario::EnroleEtTient("SECOND"),
    ])
    .await;
    let mut canal = ouvrir(&url, "vm-1".into(), "chut".into());

    let premiere = tokio::time::timeout(Duration::from_secs(5), canal.attendre_identite())
        .await
        .expect("aucune identité en 5 s")
        .expect("la boucle a renoncé avant tout enrôlement");
    assert_eq!(premiere.prefixe, "PREMIER");

    let seconde = tokio::time::timeout(Duration::from_secs(5), prochain_prefixe(&mut canal))
        .await
        .expect("aucune reprise en 5 s après la coupure : le canal ne se reprend PAS");
    assert_eq!(seconde, "SECOND");

    // Et c'est bien une SECONDE connexion, avec le même enrôlement : la
    // reprise se re-présente, elle ne se contente pas de battre.
    let premier = connexions.recv().await.expect("premier enrôlement");
    let second = connexions.recv().await.expect("second enrôlement");
    assert!(premier.contains(r#""vm":"vm-1""#), "enrôlement reçu : {premier}");
    assert_eq!(premier, second, "la reprise doit re-présenter le MÊME enrôlement");
}

/// 🔴 L'EXCEPTION DE D4, sens 1. Mutation qui le rougit : rendre
/// `Fin::Reprenable` sur `MotifCanal::Version` — une incompatibilité de
/// version se déguiserait alors en boucle de reconnexion, qui est le mode de
/// panne le plus coûteux à diagnostiquer de tout ce dépôt.
///
/// ⚠️ **DEUX TESTS ET NON UN SEUL À DEUX ASSERTIONS** : `expect` interrompt au
/// premier échec, et la seconde moitié — « aucune seconde connexion » — ne
/// serait alors JAMAIS éprouvée seule. C'est la leçon ①A-bis de P2, appliquée
/// d'avance. Mesuré : sous la mutation, les deux rougissent, chacun sur son
/// propre message.
#[tokio::test]
async fn un_refus_de_version_rend_l_attente_vaine() {
    let (url, _connexions) = faux_canal(vec![Scenario::Refuse(MotifCanal::Version)]).await;
    let mut canal = ouvrir(&url, "vm-1".into(), "chut".into());

    let verdict = tokio::time::timeout(Duration::from_secs(3), canal.attendre_identite())
        .await
        .expect("la boucle doit RENONCER, pas attendre indéfiniment");
    assert!(verdict.is_none(), "un refus de version doit rendre l'attente vaine");
}

/// L'autre moitié : le canal ne se rouvre pas.
#[tokio::test]
async fn un_refus_de_version_n_ouvre_aucune_seconde_connexion() {
    let (url, mut connexions) = faux_canal(vec![Scenario::Refuse(MotifCanal::Version)]).await;
    let _canal = ouvrir(&url, "vm-1".into(), "chut".into());

    connexions.recv().await.expect("premier enrôlement");
    // Le repli minimal (500 ms) est écoulé trois fois : s'il devait y avoir
    // une seconde tentative, elle serait là.
    tokio::time::sleep(Duration::from_millis(1_500)).await;
    assert!(
        connexions.try_recv().is_err(),
        "une seconde connexion a eu lieu : le refus de version a été réessayé"
    );
}

/// 🔴 L'EXCEPTION DE D4, sens 2 — et c'est l'autre moitié, sans laquelle
/// « ne pas réessayer » pourrait être obtenu en ne réessayant JAMAIS rien.
/// Un enrôlement refusé cesse de l'être dès que l'exploitant enrôle la VM,
/// sans que personne ne redémarre l'agent.
#[tokio::test]
async fn un_refus_d_enrolement_se_reessaie() {
    let (url, _connexions) = faux_canal(vec![
        Scenario::Refuse(MotifCanal::Enrolement),
        Scenario::EnroleEtTient("APRES-ENROLEMENT"),
    ])
    .await;
    let mut canal = ouvrir(&url, "vm-1".into(), "chut".into());

    let identite = tokio::time::timeout(Duration::from_secs(5), canal.attendre_identite())
        .await
        .expect("aucune reprise en 5 s après un refus d'enrôlement")
        .expect("la boucle a renoncé sur un refus qui n'est PAS `version`");
    assert_eq!(identite.prefixe, "APRES-ENROLEMENT");
}
