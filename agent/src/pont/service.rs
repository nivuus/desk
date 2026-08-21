//! **Le fil du pont** : il possède la table, complète les commandes ProjFS, et
//! balaie les expirations. C'est la catégorie 3 de la discipline de fil décrite
//! en tête de [`crate::pont::projfs`].
//!
//! Il ne possède **ni** le `Rtc` **ni** le socket — c'est
//! [`crate::pont::transport::tourner`] qui les tient, sur son propre fil, et
//! les deux se parlent par les deux `mpsc` que le plan définit. La raison de ce
//! découpage est écrite en tête de [`crate::pont::projfs`], sous la divergence
//! qu'elle constitue.
//!
//! # Ce que ce fil ne fait JAMAIS
//!
//! - **Il ne parcourt pas la racine.** Un `read_dir` sur la racine traverserait
//!   ProjFS, donc déclencherait nos propres rappels d'énumération, qui
//!   inscrivent une commande que **ce fil-ci** doit compléter : il
//!   s'attendrait lui-même. C'est pourquoi le relevé d'hydratation compte ce
//!   que le pont écrit au lieu de mesurer le disque.
//!
//!   ⚠️ **ET C'EST EXACTEMENT POURQUOI LE FIL D'ÉCRITURE DE F2 LUI EST
//!   DISTINCT** : celui-là, lui, LIT des fichiers de la racine
//!   (`pont::ecriture::fil`). Le loger ici rejouerait la phrase ci-dessus.
//! - **Il ne rejoue jamais une commande expirée.** Une requête rejouée
//!   produirait une seconde réponse sans destinataire (spec §5.3).

mod reponses;
mod verbes;

use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use windows::core::HRESULT;

use proto::fichiers::entetes;
use crate::pont::ecriture::fil::Ordre;
use crate::pont::enumeration::Session;
use crate::pont::erreurs::Erreur;
use crate::pont::projfs::{ContexteProjFs, Etat, PERIODE_HYDRATATION};
use crate::pont::transport::DuNavigateur;

/// Période du balayage des expirations.
///
/// ⚠️ **NON CALIBRÉE.** Posée, pas mesurée — c'est F4 qui donnera de quoi la
/// juger. Elle borne le retard avec lequel une commande échue est complétée :
/// une application attend donc au pire son budget plus cette période.
pub const PERIODE_BALAYAGE: Duration = Duration::from_millis(250);

/// Période du **recensement** : une ligne `info!` qui nomme les douze causes et
/// leurs comptes.
///
/// ⚠️ **NON CALIBRÉE.** Posée, pas mesurée, comme les cinq autres constantes de
/// temps de ce pont.
///
/// 🔴 **UNE LIGNE PAR PÉRIODE, JAMAIS UNE PAR ÉCHEC.** Le chantier TURN a payé
/// 18 619 lignes en quelques secondes pour une trace par paquet, écrites sur un
/// partage CIFS depuis la boucle : **la mesure détruisait ce qu'elle
/// mesurait**. Et l'alternative naïve — monter le pont en `RUST_LOG=debug` —
/// produirait une ligne par rappel, c'est-à-dire le même défaut par une autre
/// porte.
///
/// ⚠️ **Plus courte que [`PERIODE_HYDRATATION`] (60 s), et à dessein** : le
/// relevé d'hydratation dit une grandeur qui croît lentement, le recensement
/// sert à décider si une recette a exercé ce qu'elle croit avoir exercé.
pub const PERIODE_RECENSEMENT: Duration = Duration::from_secs(10);

/// La boucle du fil du pont. Rend quand le canal se ferme ou que le transport
/// s'arrête.
pub fn tourner(etat: Arc<Etat>, entrant: Receiver<DuNavigateur>) {
    let mut dernier_releve = Instant::now();
    let mut dernier_recensement = Instant::now();
    loop {
        match entrant.recv_timeout(PERIODE_BALAYAGE) {
            Ok(DuNavigateur::CanalOuvert) => {
                // 🔴 **C'est ce drapeau qui arme le refus d'écriture par ÉTAT.**
                // Tant qu'il est faux, `PRE_CONVERT_TO_FULL` rend
                // `ERROR_IO_DEVICE` — le seul instant où une application peut
                // encore apprendre que le navigateur n'est pas là.
                etat.canal_ouvert.store(true, Ordering::Relaxed);
                tracing::info!("canal du pont ouvert : le navigateur peut servir les requêtes");
            }
            Ok(DuNavigateur::CanalFerme) => {
                etat.canal_ouvert.store(false, Ordering::Relaxed);
                tracing::warn!("canal du pont fermé : les commandes en vol sont abandonnées");
                tout_completer(&etat, Erreur::CanalFerme);
                // ⚠️ **UNE DERNIÈRE LIGNE À L'ARRÊT, sur les DEUX sorties.**
                // Sans elle, une session plus courte que `PERIODE_RECENSEMENT`
                // ne rendrait AUCUN recensement — et un critère (4) lu sur un
                // journal vide serait indiscernable d'un critère non tenu.
                // C'est le piège du `grep` de D8 : un contrôle qui rend zéro
                // pour deux raisons opposées.
                recenser(&etat);
                return;
            }
            Ok(DuNavigateur::Reponse { correlation, trame }) => {
                traiter(&etat, correlation, &trame);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                tracing::info!("transport du pont arrêté : le fil du pont s'arrête");
                tout_completer(&etat, Erreur::CanalFerme);
                recenser(&etat);
                return;
            }
        }
        balayer(&etat);
        if dernier_releve.elapsed() >= PERIODE_HYDRATATION {
            etat.tracer_hydratation();
            dernier_releve = Instant::now();
        }
        if dernier_recensement.elapsed() >= PERIODE_RECENSEMENT {
            recenser(&etat);
            dernier_recensement = Instant::now();
        }
    }
}

/// La ligne de recensement — **l'instrument du critère (4) de F3**.
///
/// ```text
/// codes rendus total=17 introuvable=3 chemin-introuvable=1 acces-refuse=0 …
/// ```
///
/// 🔴 **UN CODE JAMAIS PRODUIT AFFICHE `0`, ET C'EST TOUT L'INTÉRÊT.** Le
/// critère (4) — « chacun des douze est observé au moins une fois » — devient
/// alors un `grep` sur UNE ligne, et il **ne peut pas être satisfait par
/// accident** : une exécution qui n'exerce rien rend douze zéros.
///
/// ⚠️ **`info!` et non `debug!`** : `scripts/run-agent.sh` pose `RUST_LOG=info`
/// par défaut, et la doctrine de ce dépôt est que l'exploitation y tourne. Une
/// mitigation muette n'en est pas une — c'est la raison écrite pour les deux
/// traces de `encode/arret.rs`, appliquée ici.
fn recenser(etat: &Etat) {
    // ── LE RELEVÉ DE LA TABLE — l'instrument du legs n°4 de F1 ────────────
    //
    // 🔴 **C'est ce qui départage les quatre hypothèses**, et aucune n'était
    // départageable jusqu'ici. F1 a mesuré des lectures qui CALENT sans jamais
    // expirer — `commande expirée` reste à 0 pendant 540 s — et déclare qu'on
    // ne sait pas OÙ le blocage se produit. Voir `pont::table::plus_ancienne`,
    // qui porte le tableau de lecture.
    //
    // ⚠️ **Une ligne toutes les 10 s, jamais une par rappel.** Le chantier TURN
    // a payé 18 619 lignes en quelques secondes pour une trace par paquet,
    // écrites sur un partage CIFS depuis la boucle : la mesure détruisait ce
    // qu'elle mesurait.
    let maintenant = Instant::now();
    let (en_vol, sans_commande, plus_ancienne_ms) = match etat.table.lock() {
        Ok(table) => (
            table.en_vol(),
            table.sans_commande(),
            table.plus_ancienne(maintenant).map(|d| d.as_millis()).unwrap_or(0),
        ),
        // ⚠️ **Un verrou empoisonné est DIT, pas tu.** Rendre des zéros ferait
        // lire « rien en vol » là où la table est inaccessible — c'est-à-dire
        // la PREMIÈRE ligne du tableau de lecture, qui accuserait le rappel.
        Err(_) => {
            tracing::warn!("recensement impossible : le verrou de la table est empoisonne");
            return;
        }
    };
    let sessions = etat.sessions.lock().map(|s| s.len()).unwrap_or(0);
    tracing::info!(
        "pont en vol={} sans_commande={} plus_ancienne_ms={} sessions={} octets_hydrates={} \
         entrees_hydratees={}",
        en_vol,
        sans_commande,
        plus_ancienne_ms,
        sessions,
        etat.octets_hydrates.load(Ordering::Relaxed),
        etat.entrees_hydratees.load(Ordering::Relaxed),
    );

    let manquants: Vec<&str> = etat
        .compteurs
        .manquants()
        .into_iter()
        .map(crate::pont::compteurs::nom)
        .collect();
    tracing::info!(
        // ⚠️ **Un champ `tracing` porterait des séquences ANSI entre son nom et
        // sa valeur sur un journal BRUT** — c'est le piège que la recette
        // d'entrée de D8 a payé, et que le `grep` de F1 a rejoué trois fois.
        // Le recensement est donc **une chaîne unique**, `nom=valeur` séparés
        // par des espaces, et il se lit tel quel sans `sed`.
        "codes rendus {} | jamais rendus : {}",
        etat.compteurs.recensement(),
        if manquants.is_empty() { "aucun".to_string() } else { manquants.join(",") }
    );
}

/// Complète en erreur tout ce qui reste en vol. Appelée quand plus aucune
/// réponse ne peut arriver.
fn tout_completer(etat: &Etat, cause: Erreur) {
    let restantes = match etat.table.lock() {
        Ok(mut table) => table.vider(),
        Err(empoisonne) => empoisonne.into_inner().vider(),
    };
    for (commande, correlation) in restantes {
        oublier_contexte(etat, correlation);
        // ⚠️ **Une écriture abandonnée doit être DITE au fil d'écriture**, sans
        // quoi sa poussée resterait « en vol » à jamais et la file n'avancerait
        // plus. L'entrée, elle, RESTE au journal — c'est le fil qui décide, et
        // c'est ce qui la rend récupérable.
        prevenir_l_ecriture(etat, commande, correlation, cause);
        verbes::completer(etat, commande, HRESULT(etat.compteurs.rendre(cause)));
    }
}

/// Dit au fil d'écriture qu'une de ses corrélations est morte.
///
/// **Rien n'est fait pour une commande ProjFS** : le fil d'écriture ne connaît
/// que les siennes, et lui en signaler une autre lui ferait clore une poussée
/// qui n'est pas la sienne.
fn prevenir_l_ecriture(etat: &Etat, commande: Option<i32>, correlation: u32, cause: Erreur) {
    if commande.is_some() {
        return;
    }
    let code = match cause {
        Erreur::DelaiDepasse => proto::fichiers::CodeEchec::Interne,
        Erreur::CanalFerme => proto::fichiers::CodeEchec::AccesRefuse,
        _ => proto::fichiers::CodeEchec::Interne,
    };
    let _ = etat.vers_ecriture.send(Ordre::Echec { correlation, code });
}

/// Retire les commandes échues et les complète en délai dépassé.
///
/// **Rien n'est rejoué**, jamais : une commande expirée dont on rejouerait la
/// requête produirait une seconde réponse sans destinataire, et le navigateur
/// répondrait à une corrélation que la table ne connaît plus.
fn balayer(etat: &Etat) {
    let echues = match etat.table.lock() {
        Ok(mut table) => table.expirees(Instant::now()),
        Err(_) => return,
    };
    for (commande, correlation) in echues {
        tracing::warn!(?commande, correlation, "commande expirée : le navigateur n'a pas répondu");
        oublier_contexte(etat, correlation);
        prevenir_l_ecriture(etat, commande, correlation, Erreur::DelaiDepasse);
        verbes::completer(etat, commande, HRESULT(etat.compteurs.rendre(Erreur::DelaiDepasse)));
    }
}

/// Retire le contexte ProjFS d'une corrélation, s'il en reste un.
fn oublier_contexte(etat: &Etat, correlation: u32) -> Option<ContexteProjFs> {
    etat.en_attente.lock().ok()?.remove(&correlation)
}

/// Traite une réponse du navigateur.
fn traiter(etat: &Etat, correlation: u32, octets: &[u8]) {
    let trame = match proto::fichiers::decoder(octets) {
        Ok(trame) => trame,
        Err(erreur) => {
            tracing::warn!(correlation, %erreur, "réponse du navigateur illisible, jetée");
            return;
        }
    };
    // ⚠️ **`resoudre` rend `None` pour une corrélation annulée, expirée ou
    // inconnue, et la réponse est alors JETÉE.** Appliquer une réponse dont la
    // commande ProjFS a déjà été complétée écrirait dans un tampon que le
    // système a repris.
    let Some((commande, attendue)) = etat.table.lock().ok().and_then(|mut t| t.resoudre(correlation))
    else {
        tracing::debug!(correlation, "réponse tardive ou inconnue : jetée");
        return;
    };
    let contexte = oublier_contexte(etat, correlation);

    if trame.type_message == proto::fichiers::TYPE_ECHEC {
        let cause = match serde_json::from_slice::<entetes::Echec>(trame.entete) {
            Ok(echec) => cause_de(echec.code),
            Err(erreur) => {
                tracing::warn!(correlation, %erreur, "échec au code illisible");
                Erreur::Inattendue
            }
        };
        tracing::debug!(?commande, correlation, ?cause, "le navigateur refuse");
        // Une écriture refusée : le code du protocole voyage TEL QUEL vers le
        // fil, qui le nomme au journal. Le traduire en `Erreur` d'abord
        // perdrait la distinction entre « disque plein » et « casse ambiguë »,
        // que `pont::erreurs` ne porte pas — et c'est le journal, pas le
        // `HRESULT`, qui est le seul destinataire (voir `pont::notifications`).
        if commande.is_none() {
            let _ = etat.vers_ecriture.send(Ordre::Echec {
                correlation,
                code: match serde_json::from_slice::<entetes::Echec>(trame.entete) {
                    Ok(echec) => echec.code,
                    Err(_) => proto::fichiers::CodeEchec::Interne,
                },
            });
        }
        return reponses::terminer(etat, commande, contexte, HRESULT(etat.compteurs.rendre(cause)));
    }

    let issue = reponses::appliquer(etat, correlation, commande, attendue, &trame, contexte.as_ref());
    match issue {
        reponses::Suite::Termine(resultat) => reponses::terminer(etat, commande, contexte, resultat),
        // La lecture continue : la commande est déjà réinscrite, et son
        // contexte est resté en place — surtout ne pas la compléter.
        reponses::Suite::Poursuit => {}
    }
}

/// Le code d'échec du protocole → la cause locale.
///
/// `match` **exhaustif** : un code neuf du protocole ne peut pas tomber dans un
/// bras fourre-tout et hériter en silence de la cause d'un autre — c'est le
/// défaut que `pont::erreurs` existe pour ne pas rejouer.
fn cause_de(code: proto::fichiers::CodeEchec) -> Erreur {
    use proto::fichiers::CodeEchec;
    match code {
        CodeEchec::Introuvable => Erreur::Introuvable,
        CodeEchec::CheminIntrouvable => Erreur::CheminIntrouvable,
        CodeEchec::AccesRefuse => Erreur::AccesRefuse,
        CodeEchec::ProtegeEnEcriture => Erreur::ProtegeEnEcriture,
        CodeEchec::NonSupporte => Erreur::NonSupporte,
        // Une plage plus grande que ce que le navigateur peut rendre. Le pont
        // découpe déjà en `TAILLE_TRAME_MAX` ; recevoir ce code signale un
        // désaccord de constante entre les deux bouts, pas une condition
        // d'exécution.
        CodeEchec::TropGrand => Erreur::Inattendue,
        CodeEchec::Interne => Erreur::Inattendue,
        // Les TROIS codes de F2. Ils naissent d'une poussée d'ÉCRITURE, donc
        // d'une commande qui ne complète aucun rappel ProjFS.
        //
        // 🔴 **AUCUN D'EUX N'ATTEINT UNE APPLICATION WINDOWS, et la traduction
        // ci-dessous ne sert QU'AU JOURNAL.** À l'instant où ils arrivent,
        // l'application a refermé son handle depuis longtemps et cru avoir
        // enregistré : il n'y a plus rien à compléter. Sans cette phrase, un
        // successeur lirait `ERROR_DISK_FULL` comme un code rendu à quelqu'un.
        CodeEchec::DisquePlein => Erreur::DisquePlein,
        CodeEchec::DejaPresent => Erreur::DejaPresent,
        // ⚠️ `CasseAmbigue` PARTAGE `Inattendue` avec `TropGrand`, et c'est
        // délibéré : `pont::erreurs` n'a pas de variante pour ce refus, en
        // créer une appartiendrait à la table complète des douze `HRESULT` de
        // **F3**, et la spec §5.1 interdit qu'un même code serve deux causes
        // distinctes — la contrainte porte sur le CODE, pas sur le fourre-tout,
        // dont c'est précisément le rôle d'être nommé comme tel. Ce qui porte
        // la cause est le JOURNAL et la page-shell, qui nomment le fichier.
        CodeEchec::CasseAmbigue => Erreur::Inattendue,
        // 🔵 **LA SEULE DE F3, ET LA SEULE QUI SOIT DIAGNOSTIQUE.** Elle n'est
        // pas un fourre-tout : le navigateur refuse de supprimer un répertoire
        // NON VIDE parce que F3 appelle `removeEntry(nom)` **sans
        // `recursive`** — un geste dans la VM ne doit pas déclencher une
        // destruction récursive du poste local sur la foi d'un miroir qu'aucune
        // preuve ne dit à jour. La recevoir signifie donc que **le miroir a
        // dérivé**, et `ERROR_DIR_NOT_EMPTY` est exactement ce qu'un
        // successeur cherchera au journal.
        CodeEchec::RepertoireNonVide => Erreur::RepertoireNonVide,
    }
}

/// Remplit un tampon d'entrées depuis une session déjà chargée.
///
/// Exposée parce que le rappel `GetDirectoryEnumeration` l'emprunte
/// **directement** quand la session est déjà chargée : c'est le cas nominal
/// après le premier tour, il ne consulte pas le navigateur, et repasser par le
/// fil du pont ferait un aller-retour de fil pour une liste déjà en mémoire.
///
/// ⚠️ **C'est le seul travail qu'un rappel fait lui-même**, et il est borné :
/// une copie de noms dans un tampon que ProjFS a fourni, sous le verrou des
/// sessions, **sans aucune E/S**. La discipline de fil interdit l'attente, pas
/// le calcul.
pub fn remplir_session(
    etat: &Etat,
    session: &mut Session,
    tampon: windows::Win32::Storage::ProjectedFileSystem::PRJ_DIR_ENTRY_BUFFER_HANDLE,
) -> HRESULT {
    verbes::remplir(etat, session, tampon)
}
