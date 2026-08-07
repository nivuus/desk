//! La création d'une sortie virtuelle pour une session, et sa restitution au
//! pilote.
//!
//! Extrait de `boucle.rs` (tâche 1 du sous-bloc D10) pour rester sous le
//! plafond de 500 lignes du projet — **avant** l'addition qui l'aurait fait
//! franchir, et non après. C'est le seul geste qui a fonctionné en D9
//! (`capteur/serveur/instances.rs`) ; les deux fichiers traités après coup y
//! ont été compressés, geste que `CLAUDE.md` interdit, puis extraits quand
//! même.
//!
//! Aucune décision ici — la table décide, ce module agit —, exactement comme
//! le module parent.

use super::*;

/// Crée la sortie virtuelle d'une session, l'apparie à sa place DXGI, y pose
/// la fenêtre, et rend les effets à enchaîner.
///
/// Extrait de la boucle pour une raison de fond : **tout chemin d'échec sous
/// la création doit défaire la sortie**. Une sortie créée que la topologie
/// DXGI ne rend pas resterait sinon tenue jusqu'à l'arrêt du superviseur.
///
/// Chaque chemin d'échec appelle `table.enfant_mort`, qui sort l'entrée
/// d'`AttendLaSortie`. **Depuis la tâche 10, cet appel ne libère plus la
/// place dans la capacité** : l'entrée bascule en `Etat::SansSession` et le
/// contrôle périodique la relance, jusqu'à `RELANCES_MAX` fois (voir
/// `superviseur::table`). Conséquence à connaître : une fenêtre dont la
/// création de sortie échoue systématiquement fait donc envoyer jusqu'à
/// `RELANCES_MAX + 2` `VersLaShell::Refus` à la page-shell — soit **cinq**
/// avec `RELANCES_MAX = 3` : une par tentative avortée (l'originale plus les
/// trois relances, `relances` valant 0, 1, 2 puis 3), **plus** l'abandon final
/// que `relancer_les_orphelines` émet quand `relances >= RELANCES_MAX` — et
/// non plus un seul comme avant cette tâche.
pub(super) fn creer_sortie(
    pilote: &PiloteParIoctl,
    sorties: &mut Sorties<'_>,
    table: &mut Table,
    prises: &mut Vec<String>,
    envoyer: &impl Fn(&VersLaShell),
    demande: Demande,
) -> Vec<Effet> {
    let Demande { session, titre, largeur, hauteur } = demande;

    // Relevé AVANT création, et c'est la pièce maîtresse de l'appariement.
    //
    // `sortie_par_dimensions` ne filtre que sur « attachée, aux bonnes
    // dimensions, pas déjà prise » : rien n'y exclut les sorties PRÉEXISTANTES.
    // Or le viewport annoncé par le navigateur peut parfaitement égaler la
    // résolution d'un moniteur physique — c'est même le cas banal en plein
    // écran. Sans ce relevé, la fenêtre serait posée sur l'écran RÉEL de la VM
    // et la sortie virtuelle qu'on vient de créer deviendrait orpheline. On
    // n'apparie donc que parmi les sorties APPARUES, et le dépôt a déjà écrit
    // la doctrine : comparer des ensembles de NOMS, jamais des nombres.
    let avant = match relever_topologie("avant création de sortie") {
        Ok(avant) => noms_attaches(&avant),
        Err(erreur) => {
            tracing::error!(session = %session.0, %erreur, "topologie DXGI illisible avant création");
            envoyer(&VersLaShell::Refus {
                titre: titre.clone(),
                motif: "topologie d'affichage illisible".into(),
            });
            // Aucune sortie n'a été créée : rien à rendre au pilote. Mais
            // l'entrée doit sortir d'`AttendLaSortie` — `enfant_mort` la
            // bascule en `SansSession` (sa place reste comptée, voir la doc
            // de cette fonction) plutôt que de la retirer : le contrôle
            // périodique la relancera.
            return table.enfant_mort(&session);
        }
    };

    let id_pilote = match sorties.creer(largeur, hauteur, 60) {
        Ok(id) => id,
        Err(erreur) => {
            tracing::error!(session = %session.0, %erreur, "création de sortie refusée");
            envoyer(&VersLaShell::Refus { titre, motif: format!("{erreur}") });
            return table.enfant_mort(&session);
        }
    };

    // Attend le FAIT — qu'une sortie neuve apparaisse dans la topologie DXGI —
    // plutôt qu'un délai plat, tout en continuant de battre le chien de garde
    // du pilote (voir la doc d'`attendre_une_sortie_neuve`).
    let apparues = attendre_une_sortie_neuve(pilote, &avant, LIMITE_RATTACHEMENT);

    let Some(cible) = placement::sortie_par_dimensions(&apparues, largeur, hauteur, prises) else {
        // Journaliser les CANDIDATS, pas seulement la demande. L'appariement
        // par dimensions tolère `placement::TOLERANCE_PX` (quatre pixels, la
        // tolérance du replacement) et rien de plus : l'égalité stricte était
        // le choix initial, la recette D1 a montré qu'elle rendait l'ouverture
        // impossible sur une course de rattachement de quelques pixels
        // (1280×713 rendue 1280×720). Le facteur DPI de 1,5 que `CLAUDE.md`
        // documente sur une sortie virtuelle reste, lui, très loin de cette
        // tolérance, donc toujours refusé : si l'hôte applique une mise à
        // l'échelle, AUCUNE fenêtre ne s'ouvrira jamais, et un journal qui ne
        // redirait que la demande laisserait ce diagnostic entièrement à faire.
        tracing::error!(
            session = %session.0,
            demande = format!("{largeur}x{hauteur}"),
            apparues = ?apparues
                .iter()
                .map(|s| format!("{} {}x{}", s.nom_sortie, s.rect.width, s.rect.height))
                .collect::<Vec<_>>(),
            "sortie créée mais introuvable dans la topologie DXGI — elle est rendue au pilote"
        );
        rendre_sans_apparier(sorties, id_pilote);
        envoyer(&VersLaShell::Refus {
            titre,
            motif: "la sortie créée est introuvable dans la topologie d'affichage".into(),
        });
        return table.enfant_mort(&session);
    };

    let nom = cible.nom_sortie.clone();
    prises.push(nom.clone());
    // Les DEUX identifiants : celui du pilote pour la destruction, le nom
    // DXGI pour la capture. Aucune relation calculable entre eux.
    let suite = table.sortie_creee(&session, id_pilote, nom, (cible.rect.width, cible.rect.height));

    // Une table qui n'a rien à dire de cette sortie ne la retient nulle part :
    // `id_pilote` ne serait plus connu de personne (ni de la table, ni d'un
    // effet à venir), une place perdue sur dix, et le nom resterait bloqué
    // dans `prises` à jamais. Le cas n'est pas atteignable avec l'ordonnancement
    // actuel de la boucle — mais cet ordonnancement n'est déclaré porteur nulle
    // part, et il suffira qu'une étape s'insère un jour.
    if suite.is_empty() {
        tracing::error!(
            session = %session.0, id_pilote,
            "la table n'attendait plus cette sortie — elle est rendue au pilote"
        );
        rendre_sans_apparier(sorties, id_pilote);
        prises.retain(|p| *p != cible.nom_sortie);
        return Vec::new();
    }

    // Poser la fenêtre dessus avant que l'enfant ne capture. On lit la fenêtre
    // dans les effets que la table VIENT de rendre, et non dans la file
    // globale : celle-ci peut porter le `LancerEnfant` d'une autre session,
    // et on placerait alors la mauvaise fenêtre.
    if let Some(Effet::LancerEnfant { fenetre, .. }) = suite.first() {
        let hwnd = windows::Win32::Foundation::HWND(fenetre.0 as *mut core::ffi::c_void);
        if let Err(erreur) = placement::poser(hwnd, &cible.rect) {
            tracing::warn!(session = %session.0, %erreur, "placement de la fenêtre échoué");
        }
    }
    suite
}

/// Rend au pilote une sortie qui n'a jamais été appariée à une session.
///
/// Rien à retirer de `prises` : par construction, aucun de ces chemins n'y a
/// inscrit quoi que ce soit — ou l'appelant s'en charge.
fn rendre_sans_apparier(sorties: &mut Sorties<'_>, id_pilote: u32) {
    if let Err(erreur) = sorties.detruire(id_pilote) {
        tracing::error!(
            id_pilote, %erreur,
            "sortie orpheline NON rendue — la garde la retentera à l'arrêt"
        );
    }
}

/// Attend qu'une sortie neuve apparaisse dans la topologie, sans cesser de
/// battre le chien de garde.
///
/// Le battement n'est pas un détail : le pilote retire les sorties d'un client
/// qui cesse de pinguer, **y compris celles qu'on vient de créer**, et l'étape
/// de ping de la boucle est hors du parcours des effets.
///
/// **`enumerer_sorties_silencieux`, jamais `relever_topologie`, DANS LA
/// SCRUTATION.** À 10 Hz, `relever_topologie` journaliserait une ligne par
/// sortie DXGI existante à chaque tour — le dépôt a déjà payé deux fois pour
/// une trace émise à la cadence d'une boucle (chantier TURN, correctif I2 de
/// D1). Le relevé nommé et journalisé reste fait une fois avant l'appel
/// (`creer_sortie`), et — depuis la relecture de cette fonction — une fois de
/// plus SEULEMENT si l'attente expire, juste avant de rendre le vecteur vide.
///
/// **Ce relevé d'expiration n'est pas cosmétique.** Sans lui, un échec ne
/// laisse au journal que le relevé d'AVANT création (qui ne peut par
/// construction pas montrer la sortie neuve) et le journal des « candidats »
/// de l'appelant, qui ne liste que les sorties déjà filtrées `attachee_au_
/// bureau && nouvelles` — vide par construction si la sortie n'a jamais été
/// attachée. Deux pannes distinctes se confondaient alors sous un même
/// journal : « la sortie est apparue mais Windows n'y a jamais rien composé »
/// (le refus que la sonde multi-fenêtres nomme déjà) contre « elle n'est
/// jamais apparue du tout ». Le relevé complet et nommé — toutes les sorties,
/// attachées et non attachées — tranche entre les deux, et ne coûte rien en
/// régime normal : il ne s'exécute que sur le chemin d'échec.
fn attendre_une_sortie_neuve(
    pilote: &PiloteParIoctl,
    avant: &[String],
    limite: std::time::Duration,
) -> Vec<SortieDxgi> {
    let echeance = std::time::Instant::now() + limite;
    loop {
        if let Err(erreur) = pilote.pinguer() {
            tracing::warn!(%erreur, "ping du chien de garde pendant l'attente de rattachement");
        }
        let toutes = enumerer_sorties_silencieux().unwrap_or_default();
        let apparues: Vec<_> = toutes
            .iter()
            .filter(|s| s.attachee_au_bureau && !avant.contains(&s.nom_sortie))
            .cloned()
            .collect();
        if !apparues.is_empty() {
            return apparues;
        }
        if std::time::Instant::now() >= echeance {
            tracing::error!(
                limite_ms = limite.as_millis() as u64,
                "aucune sortie neuve n'est apparue dans la limite"
            );
            // Relevé complet, nommé, UNE fois — sur ce seul chemin d'échec.
            // C'est ici, et seulement ici, que ce diagnostic vaut : voir la
            // doc de la fonction.
            if let Err(erreur) = relever_topologie("attente de rattachement expirée") {
                tracing::error!(%erreur, "topologie DXGI illisible au moment de l'expiration");
            }
            return Vec::new();
        }
        std::thread::sleep(PAS_RATTACHEMENT);
    }
}

/// Rend une sortie au pilote et libère sa place DXGI.
pub(super) fn rendre_la_sortie(
    sorties: &mut Sorties<'_>,
    prises: &mut Vec<String>,
    sortie_pilote: u32,
    nom_sortie: String,
) {
    match sorties.detruire(sortie_pilote) {
        Ok(()) => {
            tracing::info!(sortie_pilote, "sortie virtuelle rendue au pilote");
            prises.retain(|p| *p != nom_sortie);
        }
        // La place DXGI reste RÉSERVÉE sur échec, et c'est le point de fond.
        //
        // Un refus de destruction signifie très probablement que la sortie
        // existe toujours — et qu'elle reste donc attachée au bureau. Libérer
        // sa place la rendrait à nouveau candidate : une fenêtre ultérieure de
        // mêmes dimensions pourrait s'y voir posée pendant que la table
        // retiendrait l'`id_pilote` de la sortie NEUVE, laquelle ne servirait
        // jamais et serait détruite à tort à la fermeture — l'ancienne restant
        // orpheline. Garder la place réservée coûte au pire une place DXGI
        // jusqu'à l'arrêt ; la libérer coûte une confusion d'identité.
        Err(erreur) => tracing::error!(
            sortie_pilote, %nom_sortie, %erreur,
            "sortie virtuelle NON rendue — la garde la retentera à l'arrêt, \
             et sa place DXGI reste réservée d'ici là"
        ),
    }
}
