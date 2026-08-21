//! Le renommage et la suppression : **quoi pousser, et DANS QUEL ORDRE par
//! rapport aux écritures dues**. **PUR** — aucun `cfg`, aucune E/S, aucune
//! horloge.
//!
//! # 🔴 LA RÈGLE DONT L'OUBLI PRODUIT UNE PERTE DE DONNÉES
//!
//! **Elle n'est écrite dans aucun document du dépôt avant celui-ci**, et elle
//! naît de l'interaction de deux sous-blocs dont chacun est correct seul.
//!
//! L'idiome d'enregistrement que la spec §3.5 donne pour justifier sa décision
//! D5 est : *écrire un fichier temporaire, renommer, supprimer l'ancien*.
//! LibreOffice, Word et la plupart des éditeurs l'emploient, et les trois
//! gestes arrivent **en rafale**, sur le même répertoire.
//!
//! Or F2 pousse les écritures **après coup**, à la fermeture du handle, dans
//! une fenêtre dont il déclare lui-même qu'elle n'est pas bornée en durée.
//! Donc, sans ce module :
//!
//! - **une écriture encore due sur `de` au moment où `Renommer` part arriverait
//!   APRÈS le renommage, sur un chemin qui n'existe plus** — et
//!   `getFileHandle(…, { create: true })` **RECRÉERAIT le fichier temporaire** :
//!   l'enregistrement serait perdu, et un fichier d'échange resterait sur le
//!   poste local ;
//! - **une écriture due sur un chemin qu'on vient de supprimer RECRÉERAIT** ce
//!   que l'utilisateur efface.
//!
//! **Chacun des deux sous-blocs est correct seul ; c'est leur interaction qui
//! détruit.** C'est la forme exacte des défauts que les revues transverses de
//! ce dépôt trouvent depuis D7, et la seule qu'une revue par tâche ne peut
//! structurellement pas voir.
//!
//! # ⚠️ CE MODULE NE DÉCIDE PAS DU `PRE_`, ET C'EST UNE DIVERGENCE DÉCLARÉE
//!
//! Le §3.2 du plan de F3 lui prescrit un `decider_prealable(etat, quoi)`.
//! **Il n'est pas écrit** : [`crate::pont::notifications::decider`] EST cette
//! fonction, elle existe depuis F1, elle est pure, elle est balayée sur les 32
//! bits du masque et sur quatre états, et F3 vient de lui ajouter les quatre
//! refus du renommage et de la suppression. En écrire une seconde ferait deux
//! vérités que rien ne confronte — le défaut de `TYPES_AGENT`
//! (`proto/ts/control.ts`), liste écrite à la main que rien ne compare à
//! l'union qu'elle reflète.

/// Ce qu'une mutation demande au poste local.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mutation {
    /// 🔴 **`de` EST LA SOURCE, `vers` LA DESTINATION.** S'y tromper de sens ne
    /// produirait aucune erreur : le renommage aurait lieu, à l'envers, et la
    /// destination écraserait la source. C'est le risque R-F3-1 du plan.
    Renommer { de: String, vers: String, repertoire: bool },
    Supprimer { chemin: String, repertoire: bool },
}

impl Mutation {
    /// Le chemin **SOURCE** — celui qui existe encore au moment où la mutation
    /// est décidée, et donc celui sur lequel des écritures peuvent être dues.
    pub fn source(&self) -> &str {
        match self {
            Mutation::Renommer { de, .. } => de,
            Mutation::Supprimer { chemin, .. } => chemin,
        }
    }

    /// La source est-elle un répertoire ?
    ///
    /// 🔴 **C'est ce qui décide si les écritures dues sur les ENFANTS
    /// comptent**, et c'est pour cela que `isdirectory` est transporté depuis
    /// le rappel plutôt que redécouvert par le navigateur : celui-ci le
    /// redemanderait au prix d'un aller-retour, et se tromperait sur une entrée
    /// que le renommage vient précisément de faire disparaître.
    pub fn repertoire(&self) -> bool {
        match self {
            Mutation::Renommer { repertoire, .. } | Mutation::Supprimer { repertoire, .. } => {
                *repertoire
            }
        }
    }
}

/// Ce qu'il faut faire **AVANT** de pousser une mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ordonnancement {
    /// Rien ne retient : pousser maintenant.
    Pousser,
    /// Ces chemins portent des écritures dues, et la mutation **N'EST PAS
    /// poussée** : il faut les vider d'abord, puis re-demander.
    ///
    /// ⚠️ **Le drainage n'est pas instantané, et son échec ne doit PAS être
    /// traité comme un succès.** Si les écritures ne partent pas, la mutation
    /// reste en attente, le fichier est NOMMÉ à l'utilisateur par le compteur
    /// de F2, et le journal porte sa raison. Pousser quand même perdrait
    /// l'enregistrement.
    AttendreEcrituresDues { chemins: Vec<String> },
    /// Ces écritures dues doivent être **RETIRÉES** du journal, **puis la
    /// mutation est poussée dans la foulée**.
    ///
    /// 🔴 **Le retrait vient AVANT la poussée, et l'ordre est le sens même** :
    /// pousser d'abord recréerait sur le poste local ce que l'utilisateur vient
    /// d'effacer, et la suppression qui suit ne rattraperait pas
    /// nécessairement — le navigateur peut la refuser (répertoire non vide,
    /// permission), et le fichier ressuscité resterait.
    AbandonnerEcrituresDues { chemins: Vec<String> },
}

/// Ce qu'il faut faire de `quoi`, sachant que `dues` sont les chemins dont des
/// octets attendent encore d'être poussés.
///
/// ⚠️ **`dues` porte les chemins de la file d'écriture — EN VOL COMPRIS.**
/// Ne considérer que l'attente laisserait passer le cas le plus courant : le
/// fichier temporaire dont la poussée vient de commencer, et que le renommage
/// suit de quelques millisecondes.
pub fn ordonnancer(dues: &[String], quoi: &Mutation) -> Ordonnancement {
    let concernes: Vec<String> = dues
        .iter()
        .filter(|due| concerne(due, quoi.source(), quoi.repertoire()))
        .cloned()
        .collect();
    if concernes.is_empty() {
        return Ordonnancement::Pousser;
    }
    match quoi {
        Mutation::Renommer { .. } => Ordonnancement::AttendreEcrituresDues { chemins: concernes },
        Mutation::Supprimer { .. } => {
            Ordonnancement::AbandonnerEcrituresDues { chemins: concernes }
        }
    }
}

/// Une écriture due sur `due` est-elle retenue par une mutation de `cible` ?
///
/// 🔴 **DEUX RÈGLES QUI SE CONTREDIRAIENT SI L'ON CONFONDAIT FICHIER ET
/// RÉPERTOIRE**, et c'est `repertoire` qui tranche :
///
/// - une écriture due sur un **AUTRE** chemin ne retarde rien — comparer par
///   préfixe seul ferait que toute écriture bloquerait tout renommage ;
/// - une écriture due sur un **ENFANT** d'un répertoire renommé retarde bien —
///   comparer par égalité seule laisserait le cas du répertoire passer à
///   travers, et l'enfant serait recréé sous l'ancien chemin.
///
/// ⚠️ **Le `/` du préfixe n'est pas décoratif** : sans lui, renommer `a`
/// retiendrait une écriture due sur `ab/x`, qui n'a rien à voir.
fn concerne(due: &str, cible: &str, repertoire: bool) -> bool {
    if due == cible {
        return true;
    }
    if !repertoire {
        return false;
    }
    // Le renommage de la RACINE (`cible` vide) n'existe pas : ProjFS ne livre
    // jamais de notification pour elle. Le cas est refusé plutôt que traité
    // comme « tout est enfant », qui retiendrait toute écriture pour toujours.
    if cible.is_empty() {
        return false;
    }
    due.starts_with(&format!("{cible}/"))
}

/// Les mutations en vol et en attente.
///
/// ⚠️ **Une seule en vol à la fois, GLOBALEMENT — c'est plus strict que ce que
/// le plan demande, et c'est délibéré.** Il écrit « une mutation par CHEMIN à
/// la fois » ; la file d'écriture de F2 sérialise, elle, globalement. Deux
/// disciplines différentes sur le même canal se relisent mal, et la plus
/// stricte rend l'autre vraie *a fortiori* : deux renommages du même chemin ne
/// peuvent pas se croiser si aucune paire ne le peut.
///
/// 🔴 **AUCUNE COALESCENCE, contrairement à [`crate::pont::ecriture::File`].**
/// Deux écritures du même fichier n'ont qu'un effet — écrire le dernier
/// contenu. Deux mutations, non : renommer `a`→`b` puis `b`→`c` sont deux
/// gestes dont **l'ordre est le sens**, et fusionner le second dans le premier
/// laisserait `b` sur le poste local.
#[derive(Debug, Default)]
pub struct FileMutations {
    en_vol: Option<Mutation>,
    attente: Vec<Mutation>,
}

impl FileMutations {
    pub fn nouvelle() -> Self {
        Self::default()
    }

    /// Inscrit une mutation, et rend **ce qu'il faut pousser maintenant**.
    ///
    /// `None` veut dire « rien à commencer » : une mutation est déjà en vol.
    pub fn signaler(&mut self, quoi: Mutation) -> Option<Mutation> {
        self.attente.push(quoi);
        self.demarrer()
    }

    /// La mutation en vol est finie — **quelle qu'en soit l'issue**.
    ///
    /// 🔴 **ÉCHEC COMPRIS.** Une mutation qui échoue libère le vol : sans quoi
    /// un seul refus bloquerait toutes les mutations suivantes. C'est ce que la
    /// file d'écriture de F2 fait déjà, pour la même raison.
    pub fn terminee(&mut self) -> Option<Mutation> {
        self.en_vol = None;
        self.demarrer()
    }

    /// Remet la mutation en vol **en TÊTE de l'attente**, sans la perdre.
    ///
    /// C'est ce qu'on fait d'une mutation que [`ordonnancer`] retient : les
    /// écritures dues partent, et la mutation repasse **avant** celles qui
    /// l'ont suivie — sans quoi l'ordre des gestes de l'utilisateur serait
    /// inversé.
    pub fn differer(&mut self) {
        if let Some(quoi) = self.en_vol.take() {
            self.attente.insert(0, quoi);
        }
    }

    pub fn en_vol(&self) -> Option<&Mutation> {
        self.en_vol.as_ref()
    }

    pub fn en_attente(&self) -> usize {
        self.attente.len()
    }

    fn demarrer(&mut self) -> Option<Mutation> {
        if self.en_vol.is_some() || self.attente.is_empty() {
            return None;
        }
        let suivante = self.attente.remove(0);
        self.en_vol = Some(suivante.clone());
        Some(suivante)
    }
}

#[cfg(test)]
mod tests;
