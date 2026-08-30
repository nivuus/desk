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
use crate::geometry::Rect;

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

    // LEG 5 de D9. `borner_a_la_taille_max` attendait son appelant depuis que
    // le changement de mode de sortie a été retiré : c'est ici.
    //
    // ⚠️ Le viewport arrive en PIXELS PÉRIPHÉRIQUES depuis la tâche 5 de D9
    // (`client/src/main.ts`, `innerWidth × devicePixelRatio`) : un client à
    // `devicePixelRatio = 2` demande 2560×1440 là où il demandait 1280×720,
    // soit quatre fois les pixels à capturer et à encoder. Et le plafond de
    // 8 encodeurs concurrents n'a JAMAIS été mesuré au-delà de 720p — NVENC
    // borne en macroblocs par seconde, pas en nombre de sessions.
    //
    // `TAILLE_MAX_SORTIE` (1920×1080) n'est PAS calibrée : c'est un garde-fou
    // de prudence, et aucun jugement visuel ne l'a jugée.
    let (largeur, hauteur) =
        crate::windows_source_sortie::borner_a_la_taille_max((largeur, hauteur));

    // Relevé AVANT création. ⚠️ **Il a CESSÉ d'être la pièce maîtresse de
    // l'appariement au lot 32** — il en est désormais le REPLI, le chemin
    // principal étant de désigner notre sortie par le couple que le pilote
    // nous a rendu (voir `superviseur::designation`). Il n'est pas pour autant
    // devenu inutile, et ce qui suit dit pourquoi il reste calculé.
    //
    // `sortie_pour_viewport` ne filtre que sur « attachée, ASSEZ GRANDE, pas
    // déjà prise » : rien n'y exclut les sorties PRÉEXISTANTES. Or le viewport
    // annoncé par le navigateur peut parfaitement égaler, ou même être plus
    // petit que, la résolution d'un moniteur physique — c'est même le cas
    // banal en plein écran. Sans ce relevé, la fenêtre serait posée sur
    // l'écran RÉEL de la VM et la sortie virtuelle qu'on vient de créer
    // deviendrait orpheline. Quand la désignation ne rend rien, on n'apparie
    // donc que parmi les sorties APPARUES, et le dépôt a déjà écrit la
    // doctrine : comparer des ensembles de NOMS, jamais des nombres.
    //
    // 🔴 **CE RELEVÉ SEUL NE SUFFISAIT PAS, et le lot 30 l'a mesuré** : quand
    // la sortie neuve REMPLACE une cible forcée sur la même source, elle
    // hérite du nom d'avant, n'apparaît donc jamais, et cette différence
    // d'ensembles refusait une sortie parfaitement utilisable. Voir l'en-tête
    // de `superviseur::designation`.
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

    // Attend le FAIT — que NOTRE sortie soit là — plutôt qu'un délai plat, et
    // sans cesser de battre le chien de garde du pilote (voir la doc
    // d'`attendre_notre_sortie`).
    let (designee, candidates) =
        attendre_notre_sortie(pilote, id_pilote, &avant, LIMITE_RATTACHEMENT);

    let Some(cible) = placement::sortie_pour_viewport(&candidates, largeur, hauteur, prises)
    else {
        // Ce refus ne peut plus venir d'une sortie née TROP GRANDE — c'est le
        // leg 4 de D9, qui plafonnait le produit à trois fenêtres sur une VM
        // au registre pollué. Les énumérer toutes est le seul service que ce
        // commentaire rende à qui débogue cette `ERROR`, et **le champ
        // `designee` du journal ci-dessous dit laquelle des deux familles
        // s'applique** :
        //
        // `designee` NON VIDE — notre sortie a été nommée, et refusée quand
        // même :
        //   1. elle est plus PETITE que la demande, de plus de `TOLERANCE_PX` ;
        //   2. elle est DÉJÀ PRISE — `sortie_pour_viewport` filtre aussi sur
        //      `!deja_prises`, et `rendre_la_sortie` CRÉE délibérément ce cas :
        //      quand la destruction est refusée par le pilote, le nom reste
        //      réservé pour ne pas être réattribué.
        //
        // `designee` VIDE — la désignation n'a rien rendu (pilote sans
        // adaptateur connu, CCD muette ou en erreur, cible pas encore dans un
        // chemin actif, paire ambiguë) et le REPLI a couru :
        //   3. aucune sortie n'est apparue du tout ;
        //   4. celle qui est apparue est trop petite, ou déjà prise (1 et 2
        //      ci-dessus, mais sur une sortie qui n'est pas forcément la
        //      nôtre) ;
        //   5. 🔴 **notre sortie a REMPLACÉ une sortie préexistante**, donc
        //      elle n'est pas « apparue » — le défaut du lot 30, que la
        //      désignation ferme et que le repli, lui, ne peut pas voir.
        //
        // ❌ **Ce commentaire a dit « il ne reste que deux causes » pendant
        // toute une branche** (constat de la revue de la tâche 6 de D10,
        // différé puis repris à la revue finale), puis « TROIS » jusqu'au
        // lot 32 : il envoyait un débogueur cesser de chercher trop tôt, sur
        // une `ERROR` dont les causes manquantes étaient produites par le code
        // du même module. **Toute addition à ce chemin recompte cette liste.**
        tracing::error!(
            session = %session.0,
            demande = format!("{largeur}x{hauteur}"),
            // Vide quand la désignation n'a rien rendu : c'est ce qui départage
            // les deux familles de causes énumérées juste au-dessus, et sans ce
            // champ elles seraient indiscernables au journal.
            designee = designee.as_deref().unwrap_or(""),
            candidates = ?candidates
                .iter()
                .map(|s| format!("{} {}x{}", s.nom_sortie, s.rect.width, s.rect.height))
                .collect::<Vec<_>>(),
            "aucune sortie candidate ne peut servir ce viewport — elle est rendue au pilote"
        );
        rendre_sans_apparier(sorties, id_pilote);
        envoyer(&VersLaShell::Refus {
            titre,
            motif: "aucune sortie d'affichage ne peut servir cette fenêtre".into(),
        });
        return table.enfant_mort(&session);
    };

    // La sortie peut être bien plus grande que la fenêtre : c'est le cas
    // nominal sur une VM dont le registre a été pollué. La fenêtre est posée à
    // CETTE taille, à l'origine de la sortie, et la capture recadre le même
    // rectangle dans la duplication de CETTE sortie — jamais dans celle du
    // bureau, d'où l'absence du risque de fuite entre sessions que porte
    // `ModeCapture::FenetreRecadree` (voir l'en-tête de
    // `windows_source/sortie.rs`).
    let retenue = placement::taille_retenue((largeur, hauteur), (cible.rect.width, cible.rect.height));

    let nom = cible.nom_sortie.clone();
    prises.push(nom.clone());
    // Les DEUX identifiants : celui du pilote pour la destruction, le nom
    // DXGI pour la capture. Aucune relation calculable entre eux. La table
    // retient la taille RETENUE, pas celle de la sortie : c'est elle qui
    // voyage ensuite jusqu'au capteur (tâches 8 et 9), et que le contrôle
    // périodique de placement relit sans la recalculer.
    let suite = table.sortie_creee(&session, id_pilote, nom, retenue);

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
    //
    // À l'origine de la sortie, mais à la taille RETENUE — pas à `cible.rect`,
    // qui peut être bien plus grande (registre pollué, voir plus haut). Poser
    // à la taille de la sortie couvrirait plus que ce que la capture recadre.
    if let Some(Effet::LancerEnfant { fenetre, .. }) = suite.first() {
        let hwnd = windows::Win32::Foundation::HWND(fenetre.0 as *mut core::ffi::c_void);
        let rect =
            Rect { x: cible.rect.x, y: cible.rect.y, width: retenue.0, height: retenue.1 };
        if let Err(erreur) = placement::poser(hwnd, &rect) {
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

/// Attend que NOTRE sortie soit là, sans cesser de battre le chien de garde.
///
/// 🔴 **« NOTRE », et non « une sortie neuve » — c'est tout le lot 32.** La
/// fonction s'appelait `attendre_une_sortie_neuve`, et son prédicat
/// (`!avant.contains(…)`) était le défaut mesuré par le lot 30 : une sortie
/// qui REMPLACE une cible forcée hérite du nom d'avant et n'est donc jamais
/// « neuve ». Voir l'en-tête de `superviseur::designation`.
///
/// Rend le nom DÉSIGNÉ (vide si la désignation n'a rien rendu) et les
/// candidates. Le premier ne sert qu'au journal de l'appelant, où il départage
/// deux familles de causes qui seraient sinon indiscernables.
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
fn attendre_notre_sortie(
    pilote: &PiloteParIoctl,
    id_pilote: crate::moniteurs_virtuels::IdSortie,
    avant: &[String],
    limite: std::time::Duration,
) -> (Option<String>, Vec<SortieDxgi>) {
    // Relu UNE fois : le couple ne bouge pas pendant l'attente, et un
    // aller-retour sous le verrou du pilote n'a rien à faire dans une boucle
    // à 10 Hz. `None` pour un pilote qui ne connaît pas cet identifiant —
    // l'appelant retombe alors sur le repli, jamais sur une devinette.
    let adaptateur = pilote.adaptateur_de(id_pilote);
    // 🔴 LA REPRISE SE FAIT SUR LA MÊME SORTIE, JAMAIS SUR UNE NEUVE. Détruire
    // puis recréer changerait la topologie, donc redéclencherait la sonde
    // d'encodeur d'Apollo — la reprise nourrirait ce qu'elle attend — et
    // consommerait le vivier de dix (le bras rouge a relevé 9 sorties créées
    // pour 7 refus). Voir `superviseur::reprise`, qui porte la règle et ses
    // tests d'hôte.
    let mut tour: u32 = 1;
    let mut echeance = std::time::Instant::now() + limite;
    loop {
        if let Err(erreur) = pilote.pinguer() {
            tracing::warn!(%erreur, "ping du chien de garde pendant l'attente de rattachement");
        }
        let toutes = enumerer_sorties_silencieux().unwrap_or_default();

        // ① DÉSIGNER — par ce qu'on a DONNÉ au pilote, pas par ce qui a changé
        // autour. `chemins_actifs` est SILENCIEUSE, et il le faut : on est
        // dans une boucle à 10 Hz, et ce dépôt a payé deux fois une trace
        // émise à la cadence d'une boucle.
        let designee = adaptateur.filter(|_| designation::armee()).and_then(|adaptateur| {
            let chemins = config_affichage::chemins_actifs().ok()?;
            config_affichage::nom_gdi_de_la_cible(&chemins, adaptateur, id_pilote)
                .map(str::to_owned)
        });

        // ② Le REPLI vit dans `designation::candidates`, avec ses tests
        // d'hôte — la boucle ne fait que lui passer ce qu'elle a relevé. C'est
        // ce qui rend la règle éprouvable sans Windows : `creation_sortie` est
        // `#[cfg(windows)]` de bout en bout.
        let candidates = designation::candidates(&toutes, designee.as_deref(), avant);
        if !candidates.is_empty() {
            // 🔴 LA TRACE DE CHEMIN, ET ELLE N'EST PAS COSMÉTIQUE. Sans elle,
            // une fenêtre servie ne dit pas PAR QUEL CHEMIN elle l'a été, et
            // une verte obtenue par le repli — parce que Windows n'a pas
            // fabriqué de cible forcée ce jour-là — serait indiscernable
            // d'une verte obtenue par la désignation. Le `designee` du
            // journal ne paraissait que sur le REFUS, donc jamais quand tout
            // se passe bien : le succès était muet sur sa propre cause.
            //
            // Émise UNE FOIS par création (la boucle rend la main ici), et
            // non à la cadence de la scrutation.
            match designee.as_deref() {
                Some(nom) => tracing::info!(
                    id_pilote, ?adaptateur, nom_designe = nom,
                    "sortie DESIGNEE par son identifiant de cible (chemin ① — \
                     la correspondance CCD a rendu son nom GDI)"
                ),
                None => tracing::info!(
                    id_pilote,
                    "sortie retenue par DIFFERENCE D'ENSEMBLES (chemin ② de repli — \
                     la designation n'a rien rendu)"
                ),
            }
            return (designee, candidates);
        }
        if std::time::Instant::now() >= echeance {
            // Le tour est écoulé. La règle — bornée, testée sur l'hôte — dit
            // s'il en reste un.
            if let reprise::Suite::Reessayer { tour_suivant, apres } =
                reprise::apres_un_tour(tour, reprise::TOURS, reprise::REPIT)
            {
                // ⚠️ `warn!` et non `error!` : ce n'est pas encore un refus.
                // Un tour perdu et un abandon ne doivent pas se lire pareil.
                tracing::warn!(
                    id_pilote, tour, tours = reprise::TOURS,
                    limite_ms = limite.as_millis() as u64,
                    repit_ms = apres.as_millis() as u64,
                    "la sortie ne s'est pas attachée dans ce tour — on RÉESSAIE \
                     sur la MÊME sortie (l'attachement est intermittent, pas lent)"
                );
                std::thread::sleep(apres);
                tour = tour_suivant;
                echeance = std::time::Instant::now() + limite;
                continue;
            }
            tracing::error!(
                tours_epuises = tour,
                limite_ms = limite.as_millis() as u64,
                "aucune sortie neuve n'est apparue — TOUS LES TOURS DE REPRISE \
                 SONT ÉPUISÉS"
            );
            // Relevé complet, nommé, UNE fois — sur ce seul chemin d'échec.
            // C'est ici, et seulement ici, que ce diagnostic vaut : voir la
            // doc de la fonction.
            if let Err(erreur) = relever_topologie("attente de rattachement expirée") {
                tracing::error!(%erreur, "topologie DXGI illisible au moment de l'expiration");
            }
            // Le relevé complet ci-dessus ne dit pas POURQUOI la désignation
            // s'est tue. Cette ligne-là le dit, une fois, sur ce seul chemin
            // d'échec : sans elle, « CCD n'a jamais nommé notre cible » et
            // « CCD l'a nommée mais DXGI ne l'énumère pas » se confondraient.
            match adaptateur {
                None => tracing::error!(
                    id_pilote,
                    "le pilote ne connaît pas l'adaptateur de cette sortie — la désignation n'a pas pu être tentée, seul le repli a couru"
                ),
                Some(adaptateur) => tracing::error!(
                    id_pilote,
                    ?adaptateur,
                    "la cible n'a jamais été nommée par la configuration d'affichage dans la limite — voir moniteurs_virtuels::config_affichage"
                ),
            }
            return (None, Vec::new());
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
