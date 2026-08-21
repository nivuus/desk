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
// ⚠️ CES DEUX NOMS ÉTAIENT HÉRITÉS PAR `use super::*`. L'extraction de
// `session.rs` (sous-bloc G3) a sorti du parent les seuls emplois de production
// qui les justifiaient, et le parent a cessé de les importer : le module de
// tests doit donc les nommer lui-même. C'est le prix, minuscule et déclaré,
// d'un `use super::*` — il rend invisible ce dont on dépend.
use futures_util::{SinkExt, StreamExt};
use proto::plateforme::{DepuisLaPlateforme, IssueLancement, MotifCanal};
use tokio_tungstenite::tungstenite::Message;

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
    /// Refuse en écrivant la trame BRUTE, sans passer par nos encodeurs.
    ///
    /// 🔴 C'EST LE SEUL MOYEN DE JOUER UNE PLATEFORME D'UNE AUTRE VERSION QUE
    /// LA NÔTRE. `DepuisLaPlateforme::refus` pose toujours
    /// `PLATEFORME_VERSION` : un scénario qui l'emploierait ne pourrait pas
    /// rougir sur le défaut mesuré en recette G1, où les deux bouts ont
    /// justement des versions différentes.
    RefuseBrut(&'static str),
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
                    Some(Scenario::RefuseBrut(trame)) => {
                        let _ = ws.send(Message::Text(trame.to_string())).await;
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        drop(ws);
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

// ---------------------------------------------------------------------------
// Sous-bloc G1 — le canal devient BIDIRECTIONNEL.
// ---------------------------------------------------------------------------

/// Un scénario qui enrôle, tient, et RENVOIE tout ce que l'agent lui pousse.
///
/// Le harnais ci-dessus ne lit qu'UN message par connexion — celui de
/// l'enrôlement — et ne peut donc rien dire d'un catalogue émis après coup.
/// Celui-ci ouvre en plus un canal d'ordres que le test alimente.
async fn faux_canal_bidirectionnel(
    prefixe: &'static str,
) -> (String, mpsc::UnboundedReceiver<String>, mpsc::UnboundedSender<String>) {
    let ecoute = TcpListener::bind("127.0.0.1:0").await.expect("écoute locale");
    let port = ecoute.local_addr().expect("adresse locale").port();
    let (recus_tx, recus_rx) = mpsc::unbounded_channel();
    let (ordres_tx, mut ordres_rx) = mpsc::unbounded_channel::<String>();
    tokio::spawn(async move {
        let Ok((flux, _)) = ecoute.accept().await else { return };
        let mut ws = tokio_tungstenite::accept_async(flux).await.expect("montée WebSocket");
        if let Some(Ok(Message::Text(texte))) = ws.next().await {
            let _ = recus_tx.send(texte);
        }
        envoyer_enrole(&mut ws, prefixe).await;
        loop {
            tokio::select! {
                ordre = ordres_rx.recv() => match ordre {
                    Some(texte) => { let _ = ws.send(Message::Text(texte)).await; }
                    None => return,
                },
                recu = ws.next() => match recu {
                    Some(Ok(Message::Text(texte))) => { let _ = recus_tx.send(texte); }
                    Some(Ok(_)) => {}
                    _ => return,
                },
            }
        }
    });
    (format!("ws://127.0.0.1:{port}"), recus_rx, ordres_tx)
}

async fn attendre_message(
    recus: &mut mpsc::UnboundedReceiver<String>,
    predicat: impl Fn(&str) -> bool,
) -> String {
    let attente = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let texte = recus.recv().await.expect("le faux canal s'est tu");
            if predicat(&texte) {
                return texte;
            }
        }
    });
    attente.await.expect("aucun message correspondant en 5 s")
}

#[tokio::test]
async fn un_message_pousse_dans_la_file_arrive_au_serveur() {
    // 🔴 LA ROUGE : ne pas drainer la file dans le `select!`. Elle grossirait
    // sans fin, l'agent croirait avoir émis son catalogue, et RIEN ne le
    // dirait — ni erreur, ni trace, la plateforme resterait simplement vide.
    let (url, mut recus, _ordres) = faux_canal_bidirectionnel("PPP").await;
    let mut canal = ouvrir(&url, "w1".into(), "chut".into());
    canal.attendre_identite().await.expect("enrôlement");

    canal.emetteur().emettre(VersLaPlateforme::catalogue(true, Vec::new(), Vec::new()));
    let texte = attendre_message(&mut recus, |t| t.contains("catalogue")).await;
    assert_eq!(
        texte,
        r#"{"type":"catalogue","v":4,"complet":true,"applications":[],"disparues":[]}"#
    );
}

#[tokio::test]
async fn un_ordre_de_lancement_arrive_au_consommateur_et_ne_ferme_pas_la_session() {
    // 🔴 DEUX ROUGES EN UNE. Oublier le bras `Lancer` le ferait tomber dans le
    // bras `Err` (« message illisible »), qui FERME la session : un ordre
    // parfaitement valide déclencherait une reprise en boucle. Le second
    // `assert` mesure que la session survit — sans lui, un bras qui
    // journaliserait puis reviendrait `Fin::Reprenable` passerait le premier.
    let (url, mut recus, ordres) = faux_canal_bidirectionnel("PPP").await;
    let mut canal = ouvrir(&url, "w1".into(), "chut".into());
    canal.attendre_identite().await.expect("enrôlement");
    let mut recu_ordres = canal.ordres().expect("la file d'ordres n'est prise qu'une fois");

    ordres
        .send(r#"{"type":"lancer","v":4,"demande":"d-7","cle":"a1b2"}"#.into())
        .expect("envoi de l'ordre");

    let ordre = tokio::time::timeout(Duration::from_secs(5), recu_ordres.recv())
        .await
        .expect("aucun ordre reçu en 5 s")
        .expect("la file d'ordres est fermée");
    assert_eq!(ordre, Ordre::Lancer { demande: "d-7".into(), cle: "a1b2".into() });

    // La session vit toujours : l'émission suivante arrive.
    canal.emetteur().emettre(VersLaPlateforme::lancee("d-7", IssueLancement::Raccourci));
    let texte = attendre_message(&mut recus, |t| t.contains("lancee")).await;
    assert_eq!(
        texte,
        r#"{"type":"lancee","v":4,"demande":"d-7","issue":"raccourci"}"#
    );
}

#[tokio::test]
async fn un_message_mis_en_file_alors_que_le_socket_est_tombe_est_perdu_sans_tuer_le_canal() {
    // 🔴 C'EST LE COMPORTEMENT VOULU, ET CE TEST L'ASSÈNE. Le rendre bloquant
    // ferait de la file une fuite mémoire sur un canal qui peut rester coupé
    // des heures ; le rendre fatal tuerait le canal sur une coupure réseau
    // ordinaire. La perte est acceptable pour une seule raison, écrite auprès
    // de la file : l'agent renvoie son catalogue COMPLET à chaque
    // réenrôlement, donc toute divergence a un terme.
    let (url, mut recus) = faux_canal(vec![
        Scenario::EnroleEtCoupe("AAA"),
        Scenario::EnroleEtTient("BBB"),
    ])
    .await;
    let mut canal = ouvrir(&url, "w1".into(), "chut".into());
    assert_eq!(canal.attendre_identite().await.expect("1er enrôlement").prefixe, "AAA");

    // La coupure survient ; on pousse pendant qu'il n'y a plus de socket.
    for _ in 0..64 {
        canal.emetteur().emettre(VersLaPlateforme::catalogue(false, Vec::new(), Vec::new()));
    }

    // Le canal reprend malgré tout : c'est la preuve qu'aucune émission n'a
    // été fatale, et le second enrôlement l'atteste.
    assert_eq!(prochain_prefixe(&mut canal).await, "BBB");
    let _ = recus.recv().await;
}

#[tokio::test]
async fn l_identite_est_reannoncee_a_chaque_reenrolement() {
    // 🔴 LA ROUGE : ne pousser l'identité qu'UNE fois. La boucle de découverte
    // observe ce `watch` pour savoir qu'un réenrôlement a eu lieu, et c'est
    // ce qui la fait renvoyer le catalogue COMPLET. Sans ce second envoi, une
    // plateforme redémarrée resterait divergente SANS TERME.
    let (url, _recus) = faux_canal(vec![
        Scenario::EnroleEtCoupe("AAA"),
        Scenario::EnroleEtTient("BBB"),
    ])
    .await;
    let mut canal = ouvrir(&url, "w1".into(), "chut".into());
    assert_eq!(canal.attendre_identite().await.expect("1er").prefixe, "AAA");
    assert_eq!(prochain_prefixe(&mut canal).await, "BBB");
}

#[tokio::test]
async fn un_ordre_d_installation_arrive_dans_SA_file_et_ne_ferme_pas_la_session() {
    // 🔴 DEUX ROUGES EN UNE, exactement comme pour `Lancer` — et c'est la
    // CINQUIÈME fois que ce dépôt paie la leçon du bras manquant (D5 `Sommeil`,
    // D6 `Part`, D7 `Audio`, D8 `PleinEcran`, G2 `IconesManquantes`). Sans le
    // bras `Installer`, un ordre valide tomberait dans le bras `Err` (« message
    // illisible »), qui FERME la session : le canal se reprendrait à chaque
    // installation demandée, et la trace accuserait une divergence de version
    // qui n'existe pas.
    //
    // ⚠️ ET IL VÉRIFIE QUE L'ORDRE ARRIVE DANS **L'AUTRE** FILE. Un bras qui
    // l'aurait poussé dans `ordres` passerait un test qui ne regarde que « la
    // session survit » : la file des ordres est drainée par le fil COM de la
    // découverte, qui se figerait alors pendant tout le téléchargement.
    let (url, mut recus, ordres) = faux_canal_bidirectionnel("PPP").await;
    let mut canal = ouvrir(&url, "w1".into(), "chut".into());
    canal.attendre_identite().await.expect("enrôlement");
    let mut recu_ordres = canal.ordres().expect("la file d'ordres n'est prise qu'une fois");
    let mut recu_install = canal
        .installations()
        .expect("la file d'installations n'est prise qu'une fois");

    ordres
        .send(
            r#"{"type":"installer","v":4,"installation":"i-1","url":"http://h:8080/t/c","nom":"setup.exe","taille":42,"sha256":"ab"}"#
                .into(),
        )
        .expect("envoi de l'ordre");

    let ordre = tokio::time::timeout(Duration::from_secs(5), recu_install.recv())
        .await
        .expect("aucune installation reçue en 5 s")
        .expect("la file d'installations est fermée");
    assert_eq!(
        ordre,
        crate::plateforme::Installation {
            id: "i-1".into(),
            url: "http://h:8080/t/c".into(),
            nom: "setup.exe".into(),
            taille: 42,
            sha256: "ab".into(),
        }
    );
    // 🔴 ET LA FILE DES ORDRES N'A RIEN REÇU. C'est l'assertion qui distingue
    // « routé correctement » de « routé n'importe où ».
    assert!(recu_ordres.try_recv().is_err(), "l'installation ne doit PAS aller aux ordres");

    // La session vit toujours : l'émission suivante arrive.
    canal.emetteur().emettre(VersLaPlateforme::lancee("d-7", IssueLancement::Raccourci));
    let texte = attendre_message(&mut recus, |t| t.contains("lancee")).await;
    assert!(texte.contains(r#""demande":"d-7""#));
}

#[tokio::test]
async fn la_file_d_installations_ne_se_prend_qu_une_fois() {
    // Même propriété qu'`ordres()`, et pour la même raison : deux consommateurs
    // se voleraient les ordres l'un à l'autre, et le symptôme serait « une
    // installation sur deux ne part pas ».
    let (url, _recus, _ordres) = faux_canal_bidirectionnel("PPP").await;
    let mut canal = ouvrir(&url, "w1".into(), "chut".into());
    canal.attendre_identite().await.expect("enrôlement");
    assert!(canal.installations().is_some());
    assert!(canal.installations().is_none());
}

#[tokio::test]
async fn la_file_d_ordres_ne_se_prend_qu_une_fois() {
    // Deux consommateurs se voleraient les ordres l'un à l'autre, et chacun
    // n'en verrait qu'une partie — un défaut dont le symptôme serait « un
    // lancement sur deux ne part pas ».
    let (url, _recus, _ordres) = faux_canal_bidirectionnel("PPP").await;
    let mut canal = ouvrir(&url, "w1".into(), "chut".into());
    canal.attendre_identite().await.expect("enrôlement");
    assert!(canal.ordres().is_some());
    assert!(canal.ordres().is_none());
}

// ---------------------------------------------------------------------------
// Correction du 20 août 2026 — le refus d'une plateforme d'une AUTRE version.
// ---------------------------------------------------------------------------

/// 🔴 LA ROUGE DE BOUT EN BOUT DU DÉFAUT 2. La recette G1 a relevé, sur un
/// agent v1 face à une plateforme v2 : **0** ligne « la plateforme REFUSE la
/// version » et **10** reprises, jusqu'au palier de 30 s, sans terme. Ce test
/// joue la trame telle qu'elle arrive sur le fil — version de l'ÉMETTEUR, pas
/// la nôtre — et exige que la boucle RENONCE.
#[tokio::test]
async fn un_refus_de_version_emis_dans_une_autre_version_rend_l_attente_vaine() {
    let (url, _connexions) =
        faux_canal(vec![Scenario::RefuseBrut(r#"{"type":"refus","v":97,"motif":"version"}"#)]).await;
    let mut canal = ouvrir(&url, "vm-1".into(), "chut".into());

    let verdict = tokio::time::timeout(Duration::from_secs(3), canal.attendre_identite())
        .await
        .expect("la boucle doit RENONCER, pas boucler : c'est le défaut mesuré en recette G1");
    assert!(
        verdict.is_none(),
        "un refus de version émis dans une autre version doit rendre l'attente vaine"
    );
}

/// L'autre moitié, écrite en deux tests pour la raison déjà donnée plus haut
/// (`expect` interrompt au premier échec) : aucune seconde connexion.
#[tokio::test]
async fn un_refus_de_version_emis_dans_une_autre_version_n_ouvre_aucune_seconde_connexion() {
    let (url, mut connexions) =
        faux_canal(vec![Scenario::RefuseBrut(r#"{"type":"refus","v":97,"motif":"version"}"#)]).await;
    let _canal = ouvrir(&url, "vm-1".into(), "chut".into());

    connexions.recv().await.expect("premier enrôlement");
    tokio::time::sleep(Duration::from_millis(1_500)).await;
    assert!(
        connexions.try_recv().is_err(),
        "une seconde connexion a eu lieu : c'est la boucle sans terme de la recette G1"
    );
}

/// Un motif qu'AUCUNE version de ce dépôt ne connaît doit rester lisible, se
/// journaliser tel quel, et se réessayer — c'est la classe « le pair peut s'en
/// relever ». Sans cette tolérance, le remède ci-dessus ne tiendrait que
/// jusqu'au premier motif ajouté par une version future, et le mode de panne
/// reviendrait à l'identique.
#[tokio::test]
async fn un_refus_a_motif_inconnu_se_reessaie_au_lieu_de_devenir_illisible() {
    let (url, _connexions) = faux_canal(vec![
        Scenario::RefuseBrut(r#"{"type":"refus","v":98,"motif":"quota-depasse"}"#),
        Scenario::EnroleEtTient("APRES-MOTIF-INCONNU"),
    ])
    .await;
    let mut canal = ouvrir(&url, "vm-1".into(), "chut".into());

    let identite = tokio::time::timeout(Duration::from_secs(5), canal.attendre_identite())
        .await
        .expect("aucune reprise en 5 s après un refus à motif inconnu")
        .expect("la boucle a renoncé sur un motif qui n'est PAS `version`");
    assert_eq!(identite.prefixe, "APRES-MOTIF-INCONNU");
}
