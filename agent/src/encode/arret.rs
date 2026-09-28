//! Mise au repos des MFT d'un encodeur avant leur relâchement.
//!
//! Séparé d'`encode.rs` (déjà en dette de taille, voir `CLAUDE.md`) plutôt
//! qu'ajouté dedans.

use std::time::{Duration, Instant};

use windows::core::Interface;
use windows::Win32::Media::MediaFoundation::{
    IMFShutdown, IMFTransform, MFSHUTDOWN_COMPLETED, MFT_MESSAGE_NOTIFY_END_OF_STREAM,
    MFT_MESSAGE_NOTIFY_END_STREAMING, MFT_MESSAGE_TYPE,
};

// La file de travail sérialisée imposée à la MFT, et sa barrière.
mod file;
pub(super) use file::FileMft;

/// Met au repos les deux MFT d'un encodeur, dans l'ordre, avant que leurs
/// références COM ne soient relâchées.
///
/// **Pourquoi cette séquence existe** — relevé de la tâche 2bis (deux
/// plantages, piles identiques, symbolisées) : la faute est levée par
/// `RtlEnterCriticalSection` dans du code de `nvEncMFTH264x.dll` exécuté sous
/// `CSerialWorkQueue::QueueItem::ExecuteWorkItem`, c'est-à-dire **sur un fil de
/// la file de travail Media Foundation**, sur une section critique dont le
/// `DebugInfo` vaut `NULL`. La MFT a donc encore un élément de travail en vol
/// quand nous détruisons l'encodeur.
///
/// Dans les deux vidages de 2bis, le fil principal était simultanément dans
/// `MFShutdown` → `RtwqShutdown` → `CPlatform::FinalShutdown`. Retirer
/// `MFShutdown` du chemin n'a PAS empêché la faute : cet appel n'est donc pas
/// **nécessaire** à la faute (voir `super::demarrer_media_foundation` pour la
/// portée exacte de ce relevé).
///
/// Ce qui la traite est le COUPLE arrêt + barrière, et il a fallu retirer
/// chacun des deux séparément pour l'établir : l'arrêt seul laisse la faute
/// revenir (1 sur 10), la barrière seule aussi (2 sur 5). Ni l'un ni l'autre
/// n'est redondant — voir `arreter` et `FileMft`.
///
/// L'amont (le convertisseur) est mis au repos avant l'aval (l'encodeur) : il
/// ne doit plus rien produire pendant qu'on arrête celui qui consomme.
///
/// Chaque étape se journalise : ces appels peuvent bloquer sans rendre la
/// main, et la dernière ligne écrite est alors le seul moyen de savoir lequel.
/// Elles ne courent qu'à la destruction d'un encodeur, jamais par trame. Le
/// détail est en `debug`, mais les deux traces qui encadrent
/// `IMFShutdown::Shutdown` — seul appel dont un gel a été OBSERVÉ — sont en
/// `info`, donc lisibles sous le `RUST_LOG=info` de `scripts/run-agent.sh` :
/// une mitigation muette en exploitation n'en est pas une.
///
/// # Quand cette séquence court, et ce qu'elle coûte au pire
///
/// **Ce n'est pas un chemin réservé au multi-fenêtres à venir : il court
/// AUJOURD'HUI, en production mono-fenêtre.** `Drop for H264Encoder` s'exécute
/// à chaque remplacement de l'encodeur de la session — `set_encode_size`,
/// appelé par `transport::adaptation` à **chaque changement de barreau** du
/// réseau, et `resize` / la reconstruction de chaîne au redimensionnement (les
/// deux dans `windows_source.rs`). Tous deux tournent sur le fil unique de
/// `Session::run` (`spawn_blocking`) : ce qui bloque ici fige la session
/// **entière** — capture, encodage, RTP, ICE — sans reprise.
///
/// **Borne du pire cas, par destruction d'encodeur** : la partie bornée vaut au
/// plus `2 × DELAI_BARRIERE + 2 × DELAI_ARRET_MFT` = **8 s** (deux barrières,
/// plus une confirmation d'arrêt par MFT), ramenés à **6 s** là où le
/// convertisseur n'expose pas `IMFShutdown`. **Le total n'est borné par rien
/// pour autant** : ni les quatre `ProcessMessage`, ni les deux
/// `IMFShutdown::Shutdown` (voir `arreter`). Nominal relevé, sans commune
/// mesure : 0,5 ms par encodeur, 4,0 ms pour huit d'affilée, `attente_ms=0`
/// partout (`paralleles-n8.log`) — **mais un nominal n'est pas une borne**.
pub(super) fn mettre_au_repos(
    convertisseur: &IMFTransform,
    encodeur: &IMFTransform,
    file_encodeur: &FileMft,
) {
    // Fin de flux : inchangé, c'est ce que faisait déjà `Drop`.
    message(
        convertisseur,
        "convertisseur",
        "END_OF_STREAM",
        MFT_MESSAGE_NOTIFY_END_OF_STREAM,
    );
    message(
        convertisseur,
        "convertisseur",
        "END_STREAMING",
        MFT_MESSAGE_NOTIFY_END_STREAMING,
    );
    message(
        encodeur,
        "encodeur",
        "END_OF_STREAM",
        MFT_MESSAGE_NOTIFY_END_OF_STREAM,
    );
    message(
        encodeur,
        "encodeur",
        "END_STREAMING",
        MFT_MESSAGE_NOTIFY_END_STREAMING,
    );

    // DEUX MESSAGES DÉLIBÉRÉMENT ABSENTS, et ce n'est pas un oubli.
    //
    // `MFT_MESSAGE_COMMAND_FLUSH` et `MFT_MESSAGE_SET_D3D_MANAGER` à zéro
    // (deux pistes du brief 2ter) ont été posés ici, puis retirés sur relevé :
    // avec eux, à N = 4 encodeurs, **une exécution sur quatre s'est bloquée
    // sans retour** dans ce bloc, juste après la mise au repos de l'encodeur
    // n°0 et pendant celle du n°1 (journal arrêté sur « libération d'un
    // encodeur : avant id=1 », processus encore vivant treize minutes plus
    // tard, `Responding: True`). Le blocage est **borné à ce bloc** : la trace
    // suivante n'a jamais été écrite. Les deux fins de flux ci-dessus, elles,
    // précèdent ce chantier et n'ont jamais bloqué. Preuve versée :
    // `docs/superpowers/plans/journaux-duplications-paralleles/2ter-blocage-n4-flush-setd3dmanager.log`.
    // NON établi : lequel des deux bloquait, ni pourquoi.

    // LE CONVERTISSEUR N'A PAS DE FILE IMPOSÉE, et c'est un arbitrage mesuré,
    // pas un oubli. La revue a raison sur le principe : `create_color_converter`
    // tente d'abord `find_hardware_video_processor()`, et sur un hôte où un
    // Video Processor MATÉRIEL est enregistré, le convertisseur serait une MFT
    // matérielle avec son propre travail asynchrone, que rien ici ne couvre.
    // Sur cette VM c'est toujours le repli logiciel qui sort, donc une MFT
    // synchrone.
    //
    // Lui imposer une file et une barrière a été fait, puis retiré : dans cette
    // forme (8 files sérialisées à N = 4 au lieu de 4), une exécution sur six à
    // N = 4 s'est **figée dans `IMFShutdown::Shutdown` de l'encodeur**, trace
    // « Shutdown : avant » écrite, « après » jamais
    // (`2ter-gel-n4-shutdown.log`). La forme sans file au convertisseur avait,
    // elle, passé 16 exécutions à N = 4 sans gel. Couvrir un cas qui n'existe
    // sur aucune machine éprouvée, au prix d'un gel observé sur celle qu'on
    // éprouve, est un mauvais échange.
    //
    // NON établi : que la file du convertisseur soit la CAUSE de ce gel. C'est
    // la seule différence structurelle entre les deux formes, et le gel n'est
    // apparu qu'avec elle — sur six exécutions. `Shutdown()` peut aussi bien
    // porter ce risque en propre (voir `arreter`).

    // Barrière : plus rien de ce qui était déjà en file ne court encore.
    file_encodeur.barriere("encodeur", "après END_STREAMING");

    // Arrêt explicite des MFT, puis SECONDE barrière : l'arrêt lui-même dépose
    // du travail sur la file, et c'est précisément ce travail-là qu'il faut
    // attendre. Voir `arreter` pour ce qui rend ces deux appels nécessaires.
    arreter(convertisseur, "convertisseur");
    arreter(encodeur, "encodeur");

    file_encodeur.barriere("encodeur", "après IMFShutdown");
}

/// Garde-fou de l'attente de confirmation d'arrêt d'une MFT.
const DELAI_ARRET_MFT: Duration = Duration::from_secs(2);

/// Demande à une MFT d'arrêter ses files de travail, et attend qu'elle le
/// confirme.
///
/// `IMFShutdown::Shutdown` est le mécanisme documenté par lequel un client de
/// MFT obtient cet arrêt — c'est ce que fait le pipeline Media Foundation
/// lui-même, via `MFShutdownObject`, quand il démonte un nœud de topologie. On
/// l'appelle directement plutôt que par `MFShutdownObject` pour savoir, et
/// pouvoir journaliser, si la MFT expose seulement cette interface
/// (`MFShutdownObject` rend `S_OK` sans rien dire quand elle l'ignore), et pour
/// pouvoir attendre la confirmation par `GetShutdownStatus`.
///
/// # Deux relevés qui se contredisent en apparence, et ce qu'ils disent
///
/// 1. **Seul, cet appel ne suffit pas.** Il rend `MFSHUTDOWN_COMPLETED` en 0 ms
///    et la faute survient quand même : 1 récidive sur 10 exécutions, la
///    dernière ligne du journal avant la mort étant justement la confirmation
///    d'arrêt (`2ter-recidive-apres-imfshutdown-pile.log`).
///    **`MFSHUTDOWN_COMPLETED` d'une MFT ne prouve donc pas l'absence
///    d'élément de travail en vol la concernant.**
/// 2. **Mais il est nécessaire.** Retiré du chemin en laissant la barrière
///    seule, la faute est revenue **2 fois sur 5 exécutions**, pile et décalage
///    identiques, alors même que la barrière avait été franchie
///    (`2ter-recidive-barriere-seule-agent.log` et `…-pile.log`). Barrière et
///    arrêt ne sont pas
///    redondants : l'arrêt fait cesser la MFT, la barrière attend ce qu'il
///    laisse derrière lui. C'est pourquoi la seconde barrière suit cet appel.
///
/// # Ce que cet appel coûte comme risque, et pourquoi il reste
///
/// `Shutdown()` n'est borné par RIEN — le garde-fou ci-dessous ne borne que la
/// boucle de confirmation qui suit. Un appel non borné dans un `Drop` gèle la
/// session entière, ce qui serait pire que le plantage qu'on corrige. **Et ce
/// n'est pas un risque différé au multi-fenêtres** : ce `Drop` court déjà en
/// production mono-fenêtre — voir `mettre_au_repos`, qui nomme les deux chemins
/// et écrit la borne du pire cas.
///
/// **Et ce gel a été OBSERVÉ, dans cet appel précis.** À N = 4, une exécution
/// s'est arrêtée sur `IMFShutdown::Shutdown : avant mft="encodeur"` (id=2,
/// 18:00:27,347197) sans jamais écrire son `après`, processus encore vivant
/// treize minutes plus tard :
/// `docs/superpowers/plans/journaux-duplications-paralleles/2ter-gel-n4-shutdown.log`.
/// Ce n'est donc pas un risque théorique.
///
/// **La cause n'est PAS attribuée.** Cette exécution portait aussi une file
/// imposée au convertisseur, retirée depuis (voir `mettre_au_repos`) ; le
/// départage entre les deux n'a pas été fait, et six exécutions ne l'auraient
/// pas permis. Que le gel ait disparu avec cette file ne prouve pas qu'il
/// venait d'elle.
///
/// L'appel reste malgré tout, faute d'alternative sûre : le déporter sur un
/// autre fil exigerait de faire traverser une interface COM à une frontière
/// d'appartement (le fil principal est dans un STA — cadres
/// `ClassicSTAThreadWaitForHandles` du vidage 2bis), ce qui échangerait un
/// risque contre un défaut certain. Ce qui reste acquis, et rien de plus :
/// l'appel est encadré de deux traces **`info`** — et non `debug`, sans quoi la
/// mitigation serait muette sous le `RUST_LOG=info` de l'exploitation —, de
/// sorte qu'un gel se lit au lieu de rester muet ; c'est ainsi que celui-ci a
/// été vu.
fn arreter(mft: &IMFTransform, quoi: &'static str) {
    let arret: IMFShutdown = match mft.cast() {
        Ok(arret) => arret,
        Err(err) => {
            // Relevé, pas supposé : si l'interface manque, le journal le dit,
            // et l'on sait que ce chemin n'a rien arrêté du tout. C'est le cas
            // du convertisseur logiciel sur cette VM (`0x80004002`).
            tracing::debug!(mft = quoi, erreur = %err, "MFT sans IMFShutdown : pas d'arrêt explicite");
            return;
        }
    };

    // `info` et non `debug` : seule mitigation du seul appel non borné, et
    // l'exploitation tourne en `RUST_LOG=info`. Deux lignes par destruction
    // d'encodeur, jamais par trame. NE PAS REDESCENDRE.
    tracing::info!(mft = quoi, "IMFShutdown::Shutdown : avant");
    if let Err(err) = unsafe { arret.Shutdown() } {
        tracing::warn!(mft = quoi, erreur = %err, "IMFShutdown::Shutdown refusé");
        return;
    }
    tracing::info!(mft = quoi, "IMFShutdown::Shutdown : après");

    let debut = Instant::now();
    loop {
        match unsafe { arret.GetShutdownStatus() } {
            Ok(statut) if statut == MFSHUTDOWN_COMPLETED => {
                tracing::debug!(
                    mft = quoi,
                    attente_ms = debut.elapsed().as_millis() as u64,
                    "arrêt de la MFT confirmé"
                );
                return;
            }
            // `MFSHUTDOWN_INITIATED` : l'arrêt court encore, on repasse.
            Ok(_) => {}
            Err(err) => {
                tracing::debug!(
                    mft = quoi,
                    erreur = %err,
                    "GetShutdownStatus indisponible : arrêt demandé mais non confirmable"
                );
                return;
            }
        }
        if debut.elapsed() >= DELAI_ARRET_MFT {
            tracing::warn!(
                mft = quoi,
                delai_ms = DELAI_ARRET_MFT.as_millis() as u64,
                "arrêt de la MFT non confirmé dans le délai : on relâche quand même"
            );
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// Envoie un message à une MFT en encadrant l'appel de deux traces : un appel
/// qui ne rend pas la main se lit alors dans le journal.
fn message(mft: &IMFTransform, quoi: &'static str, nom: &'static str, message: MFT_MESSAGE_TYPE) {
    tracing::debug!(mft = quoi, message = nom, "mise au repos : avant");
    let issue = unsafe { mft.ProcessMessage(message, 0) };
    tracing::debug!(
        mft = quoi,
        message = nom,
        refuse = issue.is_err(),
        "mise au repos : après"
    );
}
