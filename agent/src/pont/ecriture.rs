//! La file des écritures dues : **quoi** pousser, **dans quel ordre**, et ce
//! qu'on fait d'une notification qui arrive pendant une poussée. **PUR** —
//! aucun `cfg`, aucune E/S, aucune horloge.
//!
//! # Les cinq règles, et ce que chacune empêche
//!
//! 1. **UNE SEULE POUSSÉE EN VOL À LA FOIS.** Deux flux `createWritable()`
//!    concurrents sur le même fichier s'écraseraient l'un l'autre ; deux flux
//!    sur des fichiers distincts satureraient la file SCTP, ce que F1 a déjà
//!    décidé d'éviter (« un morceau en vol à la fois »).
//! 2. **UNE NOTIFICATION QUI ARRIVE PENDANT UNE POUSSÉE EST REJOUÉE APRÈS**,
//!    et le fichier est alors **relu depuis le début**. La jeter perdrait les
//!    derniers octets écrits par l'utilisateur, **silencieusement** ; pousser
//!    la suite sans relire mêlerait des morceaux de deux époques du même
//!    fichier, ce qu'aucun condensat ne rattraperait.
//! 3. **ORDRE FIFO D'INSCRIPTION entre chemins distincts.** Un `HashSet` en
//!    rendrait un différent à chaque exécution, et la reprise d'un lot
//!    deviendrait irreproductible.
//! 4. **UNE CRÉATION DE RÉPERTOIRE NE PORTE AUCUN CONTENU.** Lui faire lire un
//!    fichier rendrait `IsADirectory` sur le chemin le plus banal qui soit.
//! 5. **UNE CRÉATION DE FICHIER EST SUIVIE, EN GÉNÉRAL, D'UNE POUSSÉE DE
//!    CONTENU** — à la fermeture du handle. Un fichier créé et jamais écrit
//!    reste vide des deux côtés, ce qui est juste.
//!
//! # ⚠️ Ce que F3 attend de ce module, et qu'il ne faut pas lui retirer
//!
//! Le plan de F3 (« le renommage sans renommage ») pose deux règles dont
//! l'oubli produit une perte de données, et **les deux supposent une file
//! indexée par CHEMIN** :
//!
//! - `Renommer { de, vers }` doit **pousser d'abord** les écritures dues sur
//!   `de` — sans quoi une poussée en retard arriverait **après** le renommage,
//!   sur un chemin qui n'existe plus, et le navigateur **recréerait le fichier
//!   temporaire** : l'enregistrement serait perdu ;
//! - `Supprimer { chemin }` doit **retirer** les écritures dues sur `chemin` —
//!   les pousser **recréerait ce que l'utilisateur efface**.
//!
//! **Chacun des deux sous-blocs est correct seul ; c'est leur interaction qui
//! détruit.** F2 ne les implémente pas — il n'a ni renommage ni suppression —,
//! mais il expose ce qu'elles exigent : [`File::en_vol`], [`File::attend`] et
//! [`File::oublier`].

pub mod fil;

/// Ce qui déclenche une poussée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evenement {
    /// Un fichier a été refermé après modification, ou tronqué à l'ouverture.
    Modifie { chemin: String },
    /// Une entrée vient d'apparaître dans la racine.
    Cree { chemin: String, repertoire: bool },
}

impl Evenement {
    pub fn chemin(&self) -> &str {
        match self {
            Evenement::Modifie { chemin } | Evenement::Cree { chemin, .. } => chemin,
        }
    }

    /// Un répertoire n'a **aucun octet** à lire.
    pub fn est_repertoire(&self) -> bool {
        matches!(self, Evenement::Cree { repertoire: true, .. })
    }
}

/// La file des écritures dues.
///
/// ⚠️ **Un `Vec` et non un `HashMap`, pour la raison de la règle 3** : l'ordre
/// est la seule chose qui rende la reprise déterministe.
#[derive(Debug, Default)]
pub struct File {
    /// La poussée en cours, s'il y en a une.
    en_vol: Option<Evenement>,
    /// ⚠️ **Ce que la poussée en cours devra REJOUER à sa fin.** Il ne suffit
    /// pas d'un booléen : l'événement rejoué peut être d'une autre nature que
    /// celui en vol (une création suivie d'une modification).
    a_rejouer: Option<Evenement>,
    /// Les chemins en attente, dans leur ordre d'inscription.
    attente: Vec<Evenement>,
}

impl File {
    pub fn nouvelle() -> Self {
        Self::default()
    }

    /// Signale un événement, et rend **ce qu'il faut pousser MAINTENANT**.
    ///
    /// `None` veut dire « rien à commencer » : ou bien une poussée est déjà en
    /// vol, ou bien le chemin attendait déjà son tour.
    ///
    /// ⚠️ **DIVERGENCE DÉCLARÉE AVEC LE PLAN DE F2.** Sa signature annonce
    /// « `None` si déjà en vol → marque `a_rejouer` », et donne à
    /// [`File::terminee`] le seul rôle de « rendre l'événement à rejouer ».
    /// Pris à la lettre, **un chemin distinct mis en attente pendant une
    /// poussée ne serait jamais démarré** : rien ne le sortirait de la file. Les
    /// deux méthodes rendent donc *l'événement à pousser maintenant*, ce qui
    /// couvre le rejeu ET la file d'attente.
    pub fn signaler(&mut self, evenement: Evenement) -> Option<Evenement> {
        if self.en_vol.as_ref().map(Evenement::chemin) == Some(evenement.chemin()) {
            // Règle 2 : jamais deux poussées du même chemin, jamais une
            // notification perdue.
            //
            // 🔴 **LA BASE DE LA FUSION EST L'ÉVÉNEMENT EN VOL quand aucun
            // rejeu n'est encore posé, et un test l'a attrapé ROUGE.** Prendre
            // `a_rejouer` seul — qui vaut `None` la première fois — perdait le
            // fait qu'un RÉPERTOIRE était en vol : une modification arrivée
            // pendant sa création l'aurait remplacé, et le fil aurait tenté de
            // LIRE un répertoire.
            let base = self.a_rejouer.take().or_else(|| self.en_vol.clone());
            self.a_rejouer = Some(fusionner(base, evenement));
            return None;
        }
        match self.attente.iter().position(|e| e.chemin() == evenement.chemin()) {
            // Coalescence **en place** : le chemin garde son rang. Le faire
            // remonter en queue ferait passer devant lui des entrées plus
            // jeunes, alors qu'il attend depuis plus longtemps.
            Some(i) => {
                let ancien = self.attente.remove(i);
                self.attente.insert(i, fusionner(Some(ancien), evenement));
                None
            }
            None => {
                self.attente.push(evenement);
                self.demarrer()
            }
        }
    }

    /// La poussée du chemin est finie — **quelle qu'en soit l'issue**.
    ///
    /// 🔴 **ÉCHEC COMPRIS, et c'est délibéré.** Une poussée qui échoue libère
    /// le vol : l'entrée reste due AU JOURNAL, mais la file doit pouvoir
    /// avancer, sans quoi un seul échec bloquerait toutes les écritures
    /// suivantes. C'est le journal qui n'oublie pas, pas cette file.
    pub fn terminee(&mut self, chemin: &str) -> Option<Evenement> {
        if self.en_vol.as_ref().map(Evenement::chemin) != Some(chemin) {
            return None;
        }
        self.en_vol = None;
        if let Some(rejeu) = self.a_rejouer.take() {
            // Le rejeu passe DEVANT la file : le fichier vient d'être réécrit,
            // et ses octets sont les plus récents que quiconque attende.
            self.attente.insert(0, rejeu);
        }
        self.demarrer()
    }

    /// Le chemin de la poussée en cours.
    pub fn en_vol(&self) -> Option<&str> {
        self.en_vol.as_ref().map(Evenement::chemin)
    }

    /// Ce chemin est-il en vol ou en attente ? **Ce que F3 lira** avant de
    /// pousser un renommage.
    pub fn attend(&self, chemin: &str) -> bool {
        self.en_vol() == Some(chemin) || self.attente.iter().any(|e| e.chemin() == chemin)
    }

    /// Retire un chemin de l'ATTENTE. **Ce que F3 appellera** sur une
    /// suppression : pousser une écriture due sur un chemin supprimé
    /// recréerait ce que l'utilisateur efface.
    ///
    /// ⚠️ **Ne touche pas à la poussée EN VOL** : ses trames sont déjà
    /// parties, et le fil les termine. Le supprimer d'ici ferait que son `Fait`
    /// n'aurait plus de destinataire.
    pub fn oublier(&mut self, chemin: &str) {
        self.attente.retain(|e| e.chemin() != chemin);
        if self.a_rejouer.as_ref().map(Evenement::chemin) == Some(chemin) {
            self.a_rejouer = None;
        }
    }

    pub fn en_attente(&self) -> usize {
        self.attente.len()
    }

    fn demarrer(&mut self) -> Option<Evenement> {
        if self.en_vol.is_some() || self.attente.is_empty() {
            return None;
        }
        let suivant = self.attente.remove(0);
        self.en_vol = Some(suivant.clone());
        Some(suivant)
    }
}

/// Fusionne deux événements du **même** chemin.
///
/// 🔴 **UNE CRÉATION DE RÉPERTOIRE N'EST JAMAIS REMPLACÉE.** Un répertoire ne
/// se « modifie » pas : le laisser devenir un `Modifie` ferait lire un
/// répertoire comme un fichier, et le poste local recevrait `IsADirectory` sur
/// le chemin le plus banal qui soit.
///
/// Partout ailleurs, **le plus récent gagne** : une création puis une
/// modification du même fichier n'ont qu'un seul effet, écrire le contenu — et
/// l'écrivain du navigateur crée le fichier au passage.
fn fusionner(ancien: Option<Evenement>, neuf: Evenement) -> Evenement {
    match ancien {
        Some(a) if a.est_repertoire() => a,
        _ => neuf,
    }
}

#[cfg(test)]
mod tests;
