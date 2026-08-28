//! Les deux types d'OBSERVATION que le superviseur passe à
//! [`super::EtatRelance`] : l'issue d'un processus mort, et ce qu'un tour de
//! boucle voit du processus.
//!
//! **Extrait de `relance_pont.rs` le 25 août 2026, DANS UN COMMIT DÉDIÉ ET
//! AVANT l'addition qui l'a rendu nécessaire** — la règle des 500 lignes de
//! `CLAUDE.md`, et sa forme forte : « la forme forte est l'extraction jouée
//! dans une tâche DÉDIÉE, AVANT celle qui ajoute ». Le parent était à 399 et
//! l'addition du round 5 l'aurait porté à 499, soit une marge de UNE ligne,
//! que ce dépôt a vue se reperdre six fois.
//!
//! ⚠️ **La « convention de module enfant » de `CLAUDE.md` NE S'APPLIQUE PAS
//! ICI, et c'est vérifié plutôt que supposé** : elle ne vise que les modules
//! qu'on extrait d'un parent `#[cfg(windows)]` pour les faire compiler sur
//! l'hôte. `relance_pont` est portable de bout en bout, ce fichier aussi ;
//! c'est un enfant ordinaire, déclaré par un `mod issue;` à l'intérieur de
//! son parent, sans `#[path]` et sans préfixe de nom.

/// Ce qu'un processus supervisé laisse derrière lui en mourant — **le
/// discriminant du réarmement du repli depuis le round de correction 4**, à
/// la place d'une durée de vie qui ne distinguait pas une session saine d'un
/// refus endormi (voir la doc de tête de [`super`], module `relance_pont`).
///
/// Le type est PUR : il ne connaît ni `std::process::ExitStatus`, ni Windows,
/// ni `tracing`. La conversion depuis le code de sortie est
/// [`IssueDeSortie::depuis_le_code`], et c'est le seul point de contact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueDeSortie {
    /// Code de sortie **0** : le processus a fini son travail et s'est
    /// arrêté normalement. Pour le pont, c'est le `Ok(())` de
    /// `pont::executer` — la page-shell a fermé sa session.
    Propre,
    /// Code de sortie **non nul** : `bail!`, panique, `exit(n)`. Pour le
    /// pont, c'est le refus du relais, honoré puis propagé.
    Erreur,
    /// **Aucun code n'est disponible** : le processus a été tué par un signal
    /// (POSIX), ou l'état n'a pas pu être lu.
    Inconnue,
}

impl IssueDeSortie {
    /// Depuis le code de sortie, tel que `std::process::ExitStatus::code()`
    /// le rend — `None` quand il n'y en a pas.
    ///
    /// 🔴 **`None` DEVIENT `Inconnue`, ET `Inconnue` NE RÉARME PAS.** C'est
    /// le sens SÛR, et voici pourquoi : le coût des deux erreurs n'est pas
    /// symétrique. Réarmer à tort rouvre le défaut que ce round ferme — le
    /// martèlement à 100 connexions/minute contre un budget partagé de 120,
    /// c'est-à-dire le **verrouillage de la VM entière**, aucune fenêtre
    /// neuve ne pouvant plus s'attacher. Ne PAS réarmer à tort coûte, au
    /// pire, une reconnexion saine retardée de `REPLI_MAX_MS` (30 s) une
    /// fois — un inconfort borné, sur un service que le cadrage §4 déclare
    /// FACULTATIF et dont une panne ne touche jamais le flux vidéo.
    ///
    /// ⚠️ **Sur la cible réelle, ce cas ne court pas** : Windows rend
    /// toujours un code de sortie, `ExitStatus::code()` y étant `Some(_)`
    /// même pour un `TerminateProcess`. `Inconnue` couvre l'hôte POSIX (où
    /// `relance_pont` compile et se teste) et l'avenir — il est livré, éprouvé,
    /// et **jamais exercé en production** : c'est dit plutôt que supposé.
    pub fn depuis_le_code(code: Option<i32>) -> Self {
        match code {
            Some(0) => Self::Propre,
            Some(_) => Self::Erreur,
            None => Self::Inconnue,
        }
    }

    /// Cette issue prouve-t-elle qu'une panne passée est RÉSOLUE, donc que le
    /// repli exponentiel peut repartir de son plancher ?
    pub fn prouve_une_panne_resolue(self) -> bool {
        matches!(self, Self::Propre)
    }
}

/// Ce que le superviseur OBSERVE d'un processus à un tour de boucle.
///
/// 🔴 **`Mort` N'EST RENDU QU'UNE FOIS PAR MORT**, et tout `relance_pont` en
/// dépend — voir la doc de tête de [`super`] : la propriété vit dans le câblage
/// `#[cfg(windows)]` (`etat_du_pont` pose `*pont = None` en constatant la
/// mort), donc hors de portée de `cargo test --workspace`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtatObserve {
    /// Le processus tourne — ou son état est illisible et **tenu pour
    /// vivant**, ce qui est la décision de `etat_du_pont` (deux ponts se
    /// disputant la même racine ProjFS coûtent plus cher qu'un tour perdu).
    Vivant,
    /// Le processus vient d'être vu mort, avec cette issue.
    Mort(IssueDeSortie),
    /// Aucun processus : jamais lancé (un `spawn` en échec), ou mort déjà
    /// constatée à un tour précédent. **Ne réarme rien** — un `spawn` qui
    /// échoue en boucle doit voir son repli croître, c'est le cas même que
    /// le critique ③ du round 1 a mesuré.
    Absent,
}
