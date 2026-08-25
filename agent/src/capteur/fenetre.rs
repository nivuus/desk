//! Le fil d'une fenêtre : il tient un `WindowsSource` complet et le sert à
//! l'enfant par le tube.
//!
//! **Il ne réécrit AUCUN code de capture ni d'encodage.** `WindowsSource` est
//! déjà exactement le couple `DesktopCapture` + `H264Encoder` derrière le
//! trait `VideoSource` : ce module ne fait qu'appeler ce trait et transporter
//! ses résultats. C'est la simplification centrale du sous-bloc D4.
//!
//! **Depuis le sous-bloc D5, la source est OPTIONNELLE.** Le vivier
//! (`capteur/vivier.rs`, branché sur des canaux par `capteur/sommeil.rs`)
//! ordonne à ce fil de relâcher son encodeur et sa duplication, puis de les
//! reconstruire. Ces deux gestes se font ICI et nulle part ailleurs, pour la
//! même raison que la sortie de boucle : `Drop for H264Encoder` peut geler, et
//! sur ce fil-ci un gel ne coûterait que cette fenêtre.
//!
//! Cinq fichiers, parce que le sous-bloc D5 a porté celui-ci de 336 à plus de
//! 600 lignes : la boucle et le transport restent ici, l'ouverture d'une
//! fenêtre à son attache (D10, revue de la tâche 8), les transitions de
//! sommeil, le service des commandes et la trace des compteurs (D9, tâche 11)
//! vivent dans les modules enfants.

#![cfg(windows)]

// `transitions` porte `dormir` et `reveiller` — l'exécution, pour CETTE
// fenêtre, de ce que `crate::capteur::sommeil` décide pour toutes. Il ne
// s'appelle délibérément PAS `sommeil` : deux modules de ce nom dans le même
// sous-arbre se confondraient à la lecture, et l'import du registre entrerait
// en collision avec l'enfant.
mod commandes;
// `accent_fenetre` porte le tour d'accent de A1 — cinquième module enfant, sur
// le même patron que les quatre autres, et sa raison d'être est écrite dans son
// propre en-tête plutôt que recopiée ici.
//
// ⚠️ **Il porte un `#[path]` là où ses quatre frères n'en ont pas besoin, et pour
// la même raison que `transitions` ne s'appelle pas `sommeil`** : un `mod accent;`
// entrerait en collision, à la lecture comme au nommage, avec le
// `use crate::accent;` de ce fichier. Ce `#[path]`-là est HORS de la convention
// de `CLAUDE.md`, qui ne vise que les modules extraits d'un parent
// `#[cfg(windows)]` pour compiler sur l'hôte.
#[path = "fenetre/accent.rs"]
mod accent_fenetre;
// `ouverture` porte `Fenetre::ouvrir` — quatrième module enfant sur le même
// patron que les trois autres, extrait en revue de la tâche 8 du sous-bloc
// D10 : la tâche 8 et la tâche 9 avaient porté ce fichier à 505 lignes,
// au-dessus du plafond de 500.
mod ouverture;
// `trace` porte la trace périodique des compteurs de capture
// (`SOURCE_TRACE=1`) — troisième module enfant sur le même patron que les
// deux ci-dessus, extrait en revue de la tâche 11 (D9) pour la même raison de
// plafond de taille.
mod trace;
mod transitions;

use std::io::Write;
use std::sync::mpsc::{sync_channel, Receiver, Sender, SyncSender};

use crate::capteur::sommeil::file::ReceveurSession;
use std::time::{Duration, Instant};

use anyhow::Result;
use windows::Win32::Foundation::HWND;

use crate::accent;
use crate::capteur::plein_ecran;
use crate::capteur::protocole::{ecrire_image, ecrire_json, DepuisCapteur, VersCapteur};
use crate::h264::AccessUnit;
use crate::source::VideoSource;
use crate::windows_source::WindowsSource;

use self::commandes::{deposer, servir_les_commandes};
use self::trace::tracer_les_compteurs;

/// Pas de sommeil quand la source n'a rien rendu.
///
/// 10 ms, la valeur exacte de `FRAME_INTERVAL` côté transport : cette boucle
/// prend la place de l'interrogation que faisait l'enfant, et il n'y a aucune
/// raison de changer la cadence de sondage en même temps que le reste. Le
/// commentaire de `transport/piste_video.rs` explique pourquoi 10 ms et non
/// 16 : interroger plus souvent que la source ne produit lève une borne sans
/// rien coûter quand il n'y a rien à prendre.
const PAS_A_VIDE: Duration = Duration::from_millis(10);

/// Période des lignes de compteurs. **Jamais de trace par image** : le projet
/// a déjà perdu une session entière à une trace par paquet.
const PERIODE_COMPTEURS: Duration = Duration::from_secs(10);

/// Profondeur de la file entre le fil de fenêtre et le fil écrivain de la
/// connexion média.
///
/// **Bornée à dessein** : une file libre laisserait s'accumuler sans limite des
/// unités d'accès qu'un enfant qui ne lit plus ne prendra jamais. C'est le
/// pendant exact de `CAPACITE_FILE` côté enfant, et la contre-pression continue
/// donc de remonter jusqu'à la capture — mais elle remonte désormais dans
/// `deposer`, qui sert les commandes à chaque tour d'attente.
const CAPACITE_ECRITURES: usize = 8;

/// Ce que le fil de fenêtre confie au fil écrivain de la connexion média.
enum AEcrire {
    Image(AccessUnit),
    Etat(DepuisCapteur),
}

/// Faut-il continuer la boucle de fenêtre, ou la clore — et pourquoi.
enum Fin {
    Continuer,
    Terminer(&'static str),
}

/// Ce que le service des commandes doit connaître en plus de la source.
///
/// **Regroupé parce que ces quatre-là voyagent toujours ensemble** :
/// `servir_les_commandes` fait exécuter les commandes, et `deposer` l'appelle à
/// chaque tour de sa contre-pression. Les passer un à un allongeait les deux
/// signatures de quatre paramètres.
///
/// **N'emprunte rien de `Fenetre`** — `taille` est copiée — de sorte qu'un
/// contexte vivant n'empêche jamais un appel de méthode sur `&mut self`.
struct Contexte<'a> {
    /// La session, pour le registre de sommeil : c'est par elle que la
    /// visibilité reçue ici est arbitrée globalement.
    session: &'a str,
    /// Dimensions RETENUES de la fenêtre. Seule réponse possible à un
    /// redimensionnement reçu pendant un sommeil, où il n'y a plus de source à
    /// interroger.
    taille: (u32, u32),
    commandes: &'a Receiver<VersCapteur>,
    reponses: &'a Sender<DepuisCapteur>,
}

/// De quoi reconstruire la source à l'identique après un sommeil.
///
/// **`clock_origin` est retenue, jamais recalculée** : elle est l'origine des
/// horodatages de la piste vidéo, et la refaire au réveil décalerait le flux de
/// l'écart entre les deux origines — le même piège que l'attache résout par
/// `origine_qpc`.
struct Parametres {
    hwnd: HWND,
    sortie: String,
    fps: u32,
    debit: u32,
    clock_origin: Instant,
}

/// Une fenêtre servie par le capteur : sa source, et de quoi la nommer.
///
/// **Le `WindowsSource` ne quitte jamais le fil qui l'a construit.** Il porte
/// des objets COM et n'est pas `Sync` : `ouvrir` et `servir` sont appelées sur
/// le seul fil de fenêtre, et les commandes lui parviennent par `mpsc` depuis
/// le fil qui tient la connexion de commandes.
pub struct Fenetre {
    /// `None` quand la fenêtre dort : l'encodeur et la duplication DXGI sont
    /// alors relâchés, et c'est tout l'objet du sous-bloc D5. La sortie
    /// virtuelle, elle, n'est jamais touchée — c'est ce qui évite d'infliger un
    /// abandon de mutex aux fenêtres voisines à chaque endormissement.
    source: Option<WindowsSource>,
    parametres: Parametres,
    session: String,
    largeur: u32,
    hauteur: u32,
    /// PID du processus propriétaire de la fenêtre Windows, dérivé du `hwnd` à
    /// l'attache. C'est par lui que `capteur::audio::arbitrer` regroupe les
    /// fenêtres d'une même application.
    pid: u32,
}

impl Fenetre {
    pub fn dimensions(&self) -> (u32, u32) {
        (self.largeur, self.hauteur)
    }

    /// Sert la fenêtre jusqu'à la fin de sa vie.
    ///
    /// `ecrivain` est la connexion **média**, et elle est confiée à un fil
    /// ÉCRIVAIN dédié : ce fil-ci ne touche plus aucun objet fichier. Les
    /// réponses aux commandes partent par `reponses`, vers le fil qui tient la
    /// connexion de commandes et qui les écrit lui-même. Aucun objet fichier
    /// ne porte donc jamais une lecture et une écriture concurrentes.
    ///
    /// **Pourquoi un fil écrivain plutôt qu'une écriture directe.** Une
    /// écriture bloquante ici bloquait le fil de fenêtre *après* son sondage
    /// des commandes, donc sans en servir aucune — et l'enfant, qui attend sa
    /// réponse sans délai depuis la tâche 10, ne pouvait plus jamais reprendre
    /// sa lecture du média : interblocage à six maillons, relevé par la revue
    /// de la tâche 10. La seule attente que ce fil peut encore subir est celle
    /// de `deposer`, **qui sert les commandes à chaque tour**.
    pub fn servir<E: Write + Send + 'static>(
        mut self,
        commandes: Receiver<VersCapteur>,
        reponses: Sender<DepuisCapteur>,
        ecrivain: E,
    ) -> Result<()> {
        // Copiée une fois : les traces la citent à chaque tour, et la boucle
        // emprunte `self` en mutable pendant tout ce temps.
        let session = self.session.clone();

        // Consignation n°2 du sous-bloc D6, portée ici : tous les enfants et le
        // capteur écrivent dans le MÊME `agent.log` (stdout hérité depuis D4).
        // Une trace sans `session` y est un nombre dans un multiensemble
        // anonyme, et D6 a dû ajouter ce champ à deux traces EN PLEINE RECETTE.
        // Un span posé une fois sur le fil de fenêtre le donne à tout ce qui
        // s'émet en dessous, y compris aux `warn!` des modules appelés.
        let _span = tracing::info_span!("fenetre", session = %session).entered();

        // 🔴 **`pid` EST LA SEULE ATTRIBUTION session ↔ fenêtre WINDOWS DU
        // DÉPÔT, ET IL FAUT LE DIRE POUR QU'UN SUCCESSEUR NE LE RETIRE PAS
        // COMME DU BRUIT** (D-P3-7, sous-bloc P3). `enfant lancé`
        // (`superviseur/enfants.rs`) porte bien un `pid`, mais c'est celui du
        // processus ENFANT AGENT ; aucune autre trace n'associe une `session`
        // au `hwnd` ni au PID de l'APPLICATION Windows qu'elle diffuse.
        //
        // Sans lui, une recette qui écrit « le texte de B est arrivé dans LA
        // fenêtre de B » n'est pas ATTRIBUABLE — et un relevé non attribuable
        // n'est pas un verdict. La voie « coller un nonce et regarder quel
        // Bloc-notes a grandi » est CIRCULAIRE : elle établirait l'attribution
        // par le mécanisme même que la recette mesure, défaut que D8 a payé sur
        // `resoudreIdentite` et que son propre rapport qualifie de
        // « partiellement circulaire ».
        //
        // Le champ vit DÉJÀ sur `Fenetre` et est DÉJÀ passé à `inscrire` : rien
        // n'a eu à remonter. Le pilote le résout ensuite en
        // `MainWindowHandle` par `Get-Process -Id`, puis lit par `WM_GETTEXT`.
        tracing::info!(
            %session,
            pid = self.pid,
            sortie = %self.parametres.sortie,
            largeur = self.largeur,
            hauteur = self.hauteur,
            "fenêtre attachée au capteur"
        );

        let (ecritures, a_ecrire) = sync_channel::<AEcrire>(CAPACITE_ECRITURES);
        let session_ecrivain = session.clone();
        std::thread::spawn(move || ecrire_le_media(ecrivain, a_ecrire, &session_ecrivain));

        // Inscription au vivier. Une fenêtre naît ENDORMIE des deux côtés — au
        // vivier ET ici, `source` valant `None` depuis `ouvrir` : c'est ce qui
        // fait que le vivier voit la vérité dès la première seconde, et que
        // huit encodeurs au plus existent quel que soit le nombre de fenêtres
        // attachées. Le premier signal de visibilité du client la réveillera.
        //
        // ⚠️ **Corollaire : une fenêtre dont le client n'annonce JAMAIS sa
        // visibilité ne se réveille jamais, et sa page reste noire.** C'est le
        // comportement voulu — aucun encodeur ne doit être pris pour une
        // fenêtre que personne ne déclare regarder —, mais c'est la nouvelle
        // façon dont une session peut rester vide sans qu'aucune erreur ne soit
        // journalisée.
        //
        // `inscrire` frappe et rend la GÉNÉRATION de cette inscription (D9,
        // F5 de D7) : retenue en local — jamais sur `self`, elle n'a de sens
        // qu'entre cet appel et le `retirer` de fin de fonction, tous deux
        // sur ce même fil — et redonnée telle quelle à `retirer`, seul moyen
        // pour le registre de reconnaître un `retirer` déjà périmé par un
        // rattachement survenu entre-temps.
        let (ordres, generation) = crate::capteur::sommeil::inscrire(&session, self.pid);

        let resultat = self.boucler(&session, &ordres, &ecritures, &commandes, &reponses);

        // **Point de passage UNIQUE de toutes les sorties de la boucle**, y
        // compris ses sorties d'erreur : une session qui sortirait sans se
        // retirer garderait sa place au vivier pour toute la vie du processus.
        // Une panique sur ce fil court-circuiterait pourtant ces deux lignes —
        // le filet est alors la chute d'`ordres` pendant le déroulement de
        // pile, que le tour de roue du registre voit comme un canal rompu et
        // qu'il retire de lui-même. Ce chemin-ci est le déterministe.
        //
        // La source est relâchée AVANT le retrait, et explicitement plutôt que
        // par la chute de `self` en fin de fonction, pour que cet ordre ne
        // dépende pas de la position d'un `return` : `retirer` rend une place
        // que le vivier peut attribuer aussitôt à une endormie, laquelle
        // demanderait un encodeur de plus au matériel si le nôtre vivait
        // encore. Le relâchement reste sur ce fil-ci, comme partout ailleurs.
        drop(self.source.take());
        crate::capteur::sommeil::retirer(&session, generation);
        resultat
    }

    /// La boucle de service. **Extraite de `servir` pour que le relâchement de
    /// la source et le retrait du vivier n'aient qu'un seul point de passage**,
    /// quel que soit le chemin de sortie.
    ///
    /// ⚠️ **Aucun emprunt sur `self.source` ne survit à une instruction.**
    /// C'est la contrainte structurante de cette fonction depuis que la source
    /// est optionnelle : `appliquer_les_ordres` a besoin de `&mut self` entier
    /// (elle relâche et reconstruit la source), ce qui est incompatible avec le
    /// `let source = &mut self.source;` que cette boucle tenait autrefois d'un
    /// bout à l'autre. Chaque point d'usage reprend donc un emprunt neuf par
    /// `self.source.as_mut()`, dans une instruction qui se termine. **Ne pas
    /// réintroduire d'emprunt long** : le compilateur le refuserait, mais la
    /// tentation de contourner en déplaçant le sommeil hors de ce fil, elle,
    /// romprait l'invariant du relâchement sur ce fil-ci.
    fn boucler(
        &mut self,
        session: &str,
        ordres: &ReceveurSession,
        ecritures: &SyncSender<AEcrire>,
        commandes: &Receiver<VersCapteur>,
        reponses: &Sender<DepuisCapteur>,
    ) -> Result<()> {
        // Initialisé sur la taille RÉSOLUE par `ouvrir` — celle-là même qui est
        // partie dans `Attachee` —, jamais sur zéro ni sur une valeur devinée.
        // C'est ce qui fait de la comparaison du point 3 un filet réel : si la
        // texture rendue au premier réveil ne fait pas la taille annoncée (une
        // sortie mise à l'échelle DPI annonce moins qu'elle ne rend, voir
        // `capture::ouverture::taille_de_sortie`), l'écart devient un `Etat` que
        // l'enfant applique. Partir de zéro aurait produit un `Etat` inutile à
        // chaque session ; partir d'une devinette aurait masqué l'écart.
        let mut dernier_etat = (true, false, self.largeur, self.hauteur);
        let mut images = 0u64;
        let mut dernier_compte = Instant::now();
        // D8 : l'état de référence est celui lu à l'OUVERTURE de la fenêtre, pas
        // une valeur par défaut arbitraire — c'est la garde qui empêche une
        // application née sans bordure de faire entrer sa fenêtre navigateur en
        // plein écran sans raison (voir `plein_ecran::SuiviBordure`).
        let mut suivi_bordure = plein_ecran::SuiviBordure::nouveau(
            plein_ecran::lire_style(self.parametres.hwnd).unwrap_or(0),
        );
        let mut dernier_style = Instant::now();
        // A1 : `SuiviAccent` part de `None` et ANNONCE SA PREMIÈRE LECTURE —
        // c'est l'INVERSE de `SuiviBordure` juste au-dessus, et le pourquoi vit
        // dans la doc d'`accent::SuiviAccent`.
        let mut suivi_accent = accent::SuiviAccent::neuf();
        // `Instant::now()` et non « il y a longtemps » : la première lecture
        // attend `PERIODE_ACCENT`, ce qui laisse la session s'établir. ⚠️ Si la
        // recette la trouve trop tardive, c'est `PERIODE_ACCENT` qu'il faut
        // régler, pas cette ligne.
        let mut dernier_accent = Instant::now();

        let motif = loop {
            // Refait à chaque tour : la taille retenue peut changer au réveil.
            // Ne contient que des copies et des emprunts extérieurs à `self`,
            // donc n'entrave aucun `&mut self`.
            let ctx =
                Contexte { session, taille: (self.largeur, self.hauteur), commandes, reponses };

            // 0. Les ordres du vivier. Avant tout le reste : dormir libère des
            //    ressources, et il n'y a aucune raison d'encoder une image de
            //    plus quand l'ordre est déjà là.
            if let Fin::Terminer(motif) = self.appliquer_les_ordres(ordres, ecritures, &ctx) {
                break motif;
            }

            // 1. Les commandes en attente, s'il y en a. Elles sont rares, et
            //    elles sont servies MÊME ENDORMIE : refuser tout pendant le
            //    sommeil ferait échouer l'adaptation réseau de l'enfant et
            //    clore la session par un chemin étranger au sommeil.
            if let Fin::Terminer(motif) = servir_les_commandes(self.source.as_mut(), &ctx) {
                break motif;
            }

            // 2. Une image, s'il y en a une — et il n'y en a jamais quand la
            //    fenêtre dort. Extraite par une instruction qui se termine,
            //    pour que l'emprunt meure avec elle : `deposer` en reprend un
            //    neuf juste après.
            let unite = match self.source.as_mut() {
                Some(source) => source.next_frame(),
                None => None,
            };
            match unite {
                Some(unite) => {
                    images += 1;
                    // La file bornée EST la contre-pression : si l'enfant ne
                    // lit plus, ce fil finit par attendre — et il n'attend que
                    // pour SA fenêtre, sans jamais cesser de servir les
                    // commandes. Une unité d'accès ne peut pas être jetée sans
                    // corrompre le flux (les images P référencent les
                    // précédentes), d'où l'attente plutôt que l'abandon.
                    if let Fin::Terminer(motif) =
                        deposer(AEcrire::Image(unite), ecritures, self.source.as_mut(), &ctx)
                    {
                        break motif;
                    }
                }
                // Rien à envoyer : soit le bureau n'a pas changé, soit la
                // fenêtre dort. Dans les deux cas, souffler.
                None => std::thread::sleep(PAS_A_VIDE),
            }

            // 3. L'état, au CHANGEMENT seulement. Une fenêtre endormie n'en a
            //    aucun à relever : son dernier `Etat` reste vrai — la sortie et
            //    la géométrie ne bougent pas pendant le sommeil — et c'est
            //    `Sommeil` qui dit au client ce qui lui arrive.
            let etat = self.source.as_ref().map(|source| {
                let (largeur, hauteur) = source.dimensions();
                (source.is_alive(), source.is_exhausted(), largeur, hauteur)
            });
            if let Some(etat) = etat {
                if etat != dernier_etat {
                    dernier_etat = etat;
                    let message = DepuisCapteur::Etat {
                        vivante: etat.0,
                        epuisee: etat.1,
                        largeur: etat.2,
                        hauteur: etat.3,
                    };
                    if let Fin::Terminer(motif) =
                        deposer(AEcrire::Etat(message), ecritures, self.source.as_mut(), &ctx)
                    {
                        break motif;
                    }
                    if !etat.0 || etat.1 {
                        tracing::info!(%session, vivante = etat.0, epuisee = etat.1, "source close");
                        break "la source est morte ou épuisée";
                    }
                }
            }

            // 4. D8 : le style de la fenêtre dit si l'application est passée en
            //    plein écran. Bridé par son propre minuteur — voir
            //    `plein_ecran::PERIODE_STYLE`.
            //
            //    `plein_ecran::actif()` D'ABORD : `PLEIN_ECRAN=0` désarme la
            //    détection — la relecture du style et l'annonce `PleinEcran`
            //    qui en découle. C'est tout ce que ce mécanisme fait
            //    désormais : le sous-bloc D9 a retiré l'autre moitié, le
            //    changement de mode de la sortie virtuelle (voir
            //    `plein_ecran::actif` pour le constat de mesure).
            if plein_ecran::actif() && dernier_style.elapsed() >= plein_ecran::PERIODE_STYLE {
                dernier_style = Instant::now();
                if let Some(style) = plein_ecran::lire_style(self.parametres.hwnd) {
                    if let Some(actif) = suivi_bordure.observer(style) {
                        tracing::info!(%session, actif, "plein ecran de la fenetre Windows");
                        let message = DepuisCapteur::PleinEcran { actif };
                        if let Fin::Terminer(motif) =
                            deposer(AEcrire::Etat(message), ecritures, self.source.as_mut(), &ctx)
                        {
                            break motif;
                        }
                    }
                }
            }

            // 5. A1 : la teinte dominante de l'icône de la fenêtre.
            //    **Le corps vit dans `fenetre/accent.rs`** : l'addition aurait
            //    porté ce fichier à 500 lignes EXACTEMENT, donc à marge nulle,
            //    et la règle du dépôt est « extraction, jamais compression ».
            //    Il a déjà franchi 500 deux fois (508 en D9, 505 en D10).
            #[cfg(windows)]
            if let Some(Fin::Terminer(motif)) = accent_fenetre::tour(
                &mut suivi_accent,
                &mut dernier_accent,
                self.parametres.hwnd,
                ecritures,
                self.source.as_mut(),
                &ctx,
            ) {
                break motif;
            }

            if dernier_compte.elapsed() >= PERIODE_COMPTEURS {
                let ecoule = dernier_compte.elapsed().as_secs_f64();
                tracing::info!(
                    %session,
                    images,
                    endormie = self.source.is_none(),
                    cadence = format!("{:.1}", images as f64 / ecoule),
                    "cadence du capteur"
                );
                tracer_les_compteurs(self.source.as_ref());
                images = 0;
                dernier_compte = Instant::now();
            }
        };

        tracing::info!(%session, images, motif, "fin de la fenêtre côté capteur");
        Ok(())
    }
}

/// Le fil écrivain de la connexion média : il ne fait qu'écrire, et il est le
/// seul à toucher cet objet fichier. Personne ne le lit.
fn ecrire_le_media<E: Write>(mut ecrivain: E, charges: Receiver<AEcrire>, session: &str) {
    for charge in charges {
        let ecrit = match charge {
            AEcrire::Image(unite) => ecrire_image(&mut ecrivain, &unite),
            AEcrire::Etat(message) => ecrire_json(&mut ecrivain, &message),
        };
        // `flush` à chaque charge : devant une fenêtre immobile, la charge
        // suivante peut ne jamais venir, et l'enfant attendrait celle-ci dans
        // un tampon. Même leçon que la réponse d'attache de la tâche 9.
        if let Err(erreur) = ecrit.and_then(|()| ecrivain.flush()) {
            tracing::warn!(%session, %erreur, "écriture de la connexion média interrompue");
            return;
        }
    }
}
