//! L'issue d'une installation : ce qui s'est réellement passé, et rien de plus.
//!
//! 🔴 **LE CODE DE SORTIE EST RAPPORTÉ, JAMAIS INTERPRÉTÉ, et c'est la
//! décision qui gouverne tout ce module.** `msiexec` rend **3010**
//! (`ERROR_SUCCESS_REBOOT_REQUIRED`) pour un succès qui demande un
//! redémarrage, et beaucoup d'installeurs graphiques rendent **0** après une
//! annulation par l'utilisateur. Un verdict tiré du code se tromperait donc
//! **dans les deux sens** : il appellerait échec une installation réussie, et
//! succès une installation que personne n'a faite. C'est la doctrine que le
//! sous-bloc D8 a payée sur un autre terrain — *juger sur la RELECTURE, jamais
//! sur le code de retour* —, et la relecture est ici la fenêtre de comptage de
//! [`super::fenetre`], qui dit ce que le disque a réellement gagné.
//!
//! 🔴 **LE SEUL RÔLE DE L'`Option` EST DE DISTINGUER « CODE RECUEILLI » DE
//! « CODE PERDU ».** Sa VALEUR n'entre dans aucune branche, et un test le
//! garde en posant `Some(3010)` sur une installation qui a bel et bien posé
//! ses raccourcis. Le code voyage jusqu'au hub, qui l'affiche à l'exploitant ;
//! il ne décide de rien ici.
//!
//! **Pur, aucun `cfg`, aucune horloge, aucune entrée-sortie** — comme
//! `capteur::plein_ecran` et `apps::reconciliation` : ses épreuves courent sur
//! l'hôte Linux, là où le reste de l'installation ne peut pas être éprouvé.

/// Pourquoi une installation n'a **jamais démarré**.
///
/// ⚠️ **`Refusee` EST UNE ADDITION À LA SPÉCIFICATION, QUI N'EN COMPTE QUE
/// TROIS, et elle est justifiée** : ces cinq cas ne sont ni un succès, ni un
/// « sans effet », ni une ignorance — **ce sont des refus, et ils savent
/// pourquoi**. Les fondre dans [`Issue::IssueInconnue`] ferait lire « on ne
/// sait pas » là où l'on sait très bien, et priverait le hub du seul message
/// qu'il puisse afficher utilement à l'utilisateur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motif {
    /// Le SHA-256 relu après écriture n'est pas celui qui était annoncé.
    Empreinte,
    /// `CreateProcessW` a rendu `ERROR_ELEVATION_REQUIRED` (740). C'est le
    /// remède qui rend une élévation LISIBLE au lieu d'une attente que
    /// personne ne comprend : la boîte de consentement s'ouvre sur le bureau
    /// sécurisé, hors de portée de toute capture.
    ElevationRequise,
    /// L'extension n'est ni `.exe` ni `.msi` — `.bat` en particulier, dont
    /// l'interprète, le répertoire de travail et la politique d'exécution
    /// appelleraient leurs propres décisions.
    Extension,
    /// 🔴 Le processus qui installe est assigné à un **job object**, donc
    /// l'installeur y mourrait avec l'agent — au milieu d'une écriture de
    /// registre, et la machine resterait à moitié installée. Un refus bruyant
    /// vaut mieux qu'une installation qu'un redéploiement tuera. C'est un
    /// GARDE, pas le remède : le remède est le leg n°1 de G1, qui n'appartient
    /// pas à ce sous-bloc.
    JobObject,
    /// Il n'y a pas la place d'écrire l'installeur.
    DisquePlein,
}

impl Motif {
    /// Les cinq variantes, dans l'ordre où le test les parcourt.
    ///
    /// 🔴 ANTI-OUBLI : une variante ajoutée sans sa ligne ici serait absente
    /// du test de correspondance, qui compare cette liste à une table écrite à
    /// la main dont le compte est en dur — le compilateur exige la branche de
    /// [`Motif::mot`], et le test exige l'entrée.
    pub const TOUS: [Self; 5] = [
        Self::Empreinte,
        Self::ElevationRequise,
        Self::Extension,
        Self::JobObject,
        Self::DisquePlein,
    ];

    /// Le mot exact qui voyage sur le fil.
    ///
    /// 🔴 ÉCRIT À LA MAIN, JAMAIS PAR UN `rename_all` : le sous-bloc G1 a
    /// MESURÉ qu'une convention de sérialisation est **inobservable** sur un
    /// enum dont toutes les variantes tiennent en un mot — passer `kebab-case`
    /// à `snake_case` sur `IssueLancement` laissait `cargo test -p proto` à
    /// 75 passed. `elevation-requise` referme la lacune de lui-même, et la
    /// table ci-dessous la garde même si une variante d'un seul mot venait à
    /// rester seule.
    pub fn mot(self) -> &'static str {
        match self {
            Self::Empreinte => "empreinte",
            Self::ElevationRequise => "elevation-requise",
            Self::Extension => "extension",
            Self::JobObject => "job-object",
            Self::DisquePlein => "disque-plein",
        }
    }

    /// Le motif que ce mot désigne, ou `None` si nous ne le connaissons pas.
    ///
    /// 🔴 `None` N'EST PAS UNE ERREUR : c'est un motif d'une version qui nous
    /// dépasse, et l'appelant doit le journaliser **verbatim** plutôt que de
    /// le perdre ou de le remplacer par un défaut.
    pub fn depuis_mot(mot: &str) -> Option<Self> {
        Self::TOUS.into_iter().find(|candidat| candidat.mot() == mot)
    }
}

/// Ce que l'agent déclare à la plateforme quand une installation s'achève.
///
/// ⚠️ **TYPE LOCAL, EN ATTENTE DE SON JUMEAU DE PROTOCOLE.** Le vocabulaire du
/// fil vit dans `proto::plateforme` ; tant qu'il n'y est pas, ce module ne
/// l'importe pas et l'appelant fera la conversion. Les noms sont les mêmes des
/// deux côtés, à dessein.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Issue {
    /// La fenêtre de comptage a vu **au moins une** application apparaître.
    Reussie,
    /// La fenêtre s'est fermée à **zéro** : l'installeur a tourné, le
    /// catalogue n'a pas bougé. C'est le cas d'une annulation.
    SansEffet,
    /// Le code de sortie n'a pas pu être recueilli, ou l'installation a
    /// expiré. **Nous ne savons pas**, et le dire est le seul énoncé honnête.
    IssueInconnue,
    /// L'installation n'a **jamais démarré**, et l'on sait pourquoi.
    Refusee,
}

impl Issue {
    /// L'issue, dans l'ordre de priorité que la spécification fixe.
    ///
    /// **L'ordre n'est pas indifférent, et chaque marche a sa raison :**
    ///
    /// 1. **`Refusee` l'emporte sur tout.** Rien n'a été lancé, donc `apparues`
    ///    ne compte que ce que d'AUTRES réconciliations ont trouvé — une
    ///    installation concurrente, ou une application posée à la main pendant
    ///    la fenêtre. En tirer un succès serait **inventer** un effet à un
    ///    processus qui n'a pas existé.
    /// 2. **`IssueInconnue` l'emporte ensuite**, sur l'absence de code comme
    ///    sur l'expiration. Un installeur expiré peut être **encore en train de
    ///    travailler** : ses apparitions sont alors partielles, et annoncer
    ///    `Reussie` reviendrait à déclarer achevé ce que personne n'a vu
    ///    s'achever. ⚠️ Cette marche coûte donc quelques faux `IssueInconnue`
    ///    sur des installations qui ont réellement abouti après l'expiration —
    ///    **c'est assumé** : « je ne sais pas » se corrige par une seconde
    ///    lecture du catalogue, « c'est réussi » ne se corrige pas.
    /// 3. **`Reussie` si la fenêtre a compté quelque chose**, `SansEffet`
    ///    sinon.
    ///
    /// ⚠️ `code_sortie` n'est LU que par `is_none()`. Si un jour une branche
    /// se met à comparer sa valeur, c'est que la décision de tête de module a
    /// été perdue.
    pub fn depuis(
        code_sortie: Option<i32>,
        apparues: usize,
        expire: bool,
        refus: Option<Motif>,
    ) -> Self {
        if refus.is_some() {
            return Self::Refusee;
        }
        if code_sortie.is_none() || expire {
            return Self::IssueInconnue;
        }
        if apparues > 0 {
            Self::Reussie
        } else {
            Self::SansEffet
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 🔴 LE TÉMOIN DE LA DÉCISION DE TÊTE DE MODULE. `msiexec` rend 3010 pour
    /// un succès qui demande un redémarrage : une interprétation « code non
    /// nul ⇒ échec » rendrait ici autre chose que `Reussie`, sur une
    /// installation qui a posé deux raccourcis sous nos yeux.
    #[test]
    fn un_code_de_sortie_non_nul_ne_defait_pas_une_fenetre_qui_a_compte() {
        assert_eq!(Issue::depuis(Some(3010), 2, false, None), Issue::Reussie);
        // Les mêmes, pour que la première ne passe pas par hasard : aucun de
        // ces codes ne doit peser, quel que soit son signe ou sa magnitude.
        for code in [1_i32, 1603, 259, -1, i32::MIN, i32::MAX] {
            assert_eq!(
                Issue::depuis(Some(code), 1, false, None),
                Issue::Reussie,
                "le code {code} a été interprété"
            );
        }
    }

    /// 🔴 L'AUTRE SENS DE LA MÊME ERREUR, et c'est le témoin de la spec ⑥ :
    /// un installeur graphique annulé sort en **0** sans rien avoir posé.
    #[test]
    fn un_installeur_annule_sort_en_zero_et_reste_sans_effet() {
        assert_eq!(Issue::depuis(Some(0), 0, false, None), Issue::SansEffet);
    }

    #[test]
    fn un_refus_l_emporte_sur_tout_le_reste() {
        // Y compris sur une fenêtre pleine et un code de succès : ce qui est
        // apparu pendant la fenêtre vient d'ailleurs, puisque rien n'a démarré.
        assert_eq!(
            Issue::depuis(Some(0), 7, false, Some(Motif::ElevationRequise)),
            Issue::Refusee
        );
        assert_eq!(
            Issue::depuis(None, 0, true, Some(Motif::JobObject)),
            Issue::Refusee
        );
    }

    #[test]
    fn l_ignorance_l_emporte_sur_une_fenetre_pleine() {
        // Code perdu : l'agent est mort avant de le recueillir.
        assert_eq!(Issue::depuis(None, 3, false, None), Issue::IssueInconnue);
        // Expiré : l'installeur peut encore travailler, ses apparitions sont
        // peut-être partielles.
        assert_eq!(Issue::depuis(Some(0), 3, true, None), Issue::IssueInconnue);
        assert_eq!(Issue::depuis(None, 0, true, None), Issue::IssueInconnue);
    }

    #[test]
    fn le_cas_nominal_ne_tient_qu_a_la_fenetre() {
        assert_eq!(Issue::depuis(Some(0), 1, false, None), Issue::Reussie);
        assert_eq!(Issue::depuis(Some(0), 0, false, None), Issue::SansEffet);
    }

    /// La table des motifs, parcourue dans les DEUX sens — un `rename_all`
    /// n'aurait pas permis de la faire rougir.
    #[test]
    fn la_table_des_motifs_fait_l_aller_retour_sur_les_cinq() {
        let attendus = [
            (Motif::Empreinte, "empreinte"),
            (Motif::ElevationRequise, "elevation-requise"),
            (Motif::Extension, "extension"),
            (Motif::JobObject, "job-object"),
            (Motif::DisquePlein, "disque-plein"),
        ];
        // 🔴 ANTI-OUBLI : `TOUS` doit couvrir exactement l'énumération
        // ci-dessus. Une variante ajoutée sans sa ligne ici fausse ce compte.
        assert_eq!(Motif::TOUS.len(), attendus.len());
        for (motif, mot) in attendus {
            assert!(Motif::TOUS.contains(&motif), "{mot} absent de TOUS");
            assert_eq!(motif.mot(), mot);
            assert_eq!(Motif::depuis_mot(mot), Some(motif));
        }
        // Un mot inconnu ne devient JAMAIS un motif par défaut.
        assert_eq!(Motif::depuis_mot("quota-depasse"), None);
        assert_eq!(Motif::depuis_mot(""), None);
        assert_eq!(Motif::depuis_mot("Empreinte"), None);
        assert_eq!(Motif::depuis_mot("elevation_requise"), None);
    }
}
