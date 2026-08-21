//! Où l'installeur atterrit, comment son nom est jugé, et ce dont le disque se
//! souvient quand l'agent, lui, ne se souvient de rien.
//!
//! 🔴 CE MODULE EST PUR : aucun `#[cfg]`, aucun accès au système de fichiers.
//! Il **compose et valide** des chemins, il n'en crée aucun ; il **juge** deux
//! booléens de présence, il ne lit aucun répertoire. La racine
//! (`%ProgramData%`) est un **paramètre**, sans quoi rien ne serait jugeable
//! sur l'hôte Linux.
//!
//! ⚠️ POURQUOI `%ProgramData%` (spec D7) : **pas `%TEMP%`**, que Windows purge
//! y compris **pendant** une installation — une archive auto-extractible qui
//! relit son propre fichier échouerait au milieu, sur un code de sortie
//! opaque ; **pas le profil utilisateur**, qu'un installeur élevé, sous un
//! autre jeton, peut ne pas voir. `%ProgramData%` est lisible par tous les
//! comptes et survit aux redémarrages — ce dont `Etat` a besoin.
//!
//! 🔴 REJETER, JAMAIS ASSAINIR EN SILENCE. Le `nom` vient du navigateur ; un
//! assainissement muet transformerait `..\..\evil.exe` en un nom acceptable
//! **et écrirait quand même un fichier**, sous un nom que personne n'a
//! demandé — le produit ferait quelque chose de raisonnable au lieu de dire
//! non. Chaque refus porte donc **son** motif, et celui d'un nom dangereux
//! n'est pas celui d'une extension refusée.

/// Le suffixe de la racine, sous `%ProgramData%` ; puis les deux marqueurs,
/// écrits l'un **avant** `CreateProcessW`, l'autre **après** la sortie.
pub const RACINE_RELATIVE: &str = r"Guacamole\installeurs";
pub const MARQUEUR_COMMENCE: &str = ".commence";
pub const MARQUEUR_TERMINE: &str = ".termine";

/// Au-delà, un répertoire d'installeur oublié est à supprimer.
///
/// ⚠️ **NON CALIBRÉE** — 24 h est la valeur que la spec propose, et elle
/// rejoint la liste que ce dépôt tient depuis `BPP_MIN`.
///
/// 🔴 CONSÉQUENCE À NE PAS PERDRE : **les marqueurs partent avec le
/// répertoire**, donc la mémoire d'`Etat` est **bornée à 24 h** — passé quoi un
/// ordre réémis pour une installation déjà jouée serait rejoué. La seconde
/// ceinture est côté plateforme, qui cesse de réémettre dès que la ligne n'est
/// plus `en_attente`.
pub const EXPIRATION_INSTALLEUR_MS: u64 = 24 * 60 * 60 * 1000;

/// Les deux bornes de longueur, en octets.
///
/// ⚠️ C'EST LEUR SOMME QUI COMPTE : `C:\ProgramData\Guacamole\installeurs\`
/// fait 36 caractères, et `36 + 64 + 1 + 128 = 229`, sous les 260 de
/// `MAX_PATH`. Les relever sans refaire cette addition produirait un chemin que
/// `CreateProcessW` refuse, très loin d'ici.
pub const IDENTIFIANT_MAX_OCTETS: usize = 64;
pub const NOM_MAX_OCTETS: usize = 128;

/// Les extensions retenues, et ce qu'on en fait.
const EXTENSIONS_RETENUES: &[(&str, Extension)] =
    &[("exe", Extension::Exe), ("msi", Extension::Msi)];

/// Reconnus comme scripts, pour être refusés **à part** — voir `Refus::Script`.
const SCRIPTS: &[&str] = &["bat", "cmd", "com", "ps1", "vbs", "js", "wsf"];

/// Ce que Windows interdit dans un nom, hors les séparateurs, qui ont leur
/// propre motif.
const CARACTERES_INTERDITS: &[char] = &['<', '>', ':', '"', '|', '?', '*'];

/// Les périphériques hérités de MS-DOS. Windows les réserve **quelle que soit
/// l'extension** : `CON.exe` désigne toujours la console.
const NOMS_RESERVES: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Pourquoi un composant de chemin est refusé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefusNom {
    Vide,
    TropLong(usize),
    /// Une barre oblique : le nom prétend désigner un chemin, quand il ne doit
    /// désigner qu'un fichier.
    Separateur(char),
    /// Un `..`, où qu'il soit. ⚠️ **`..` EST SIGNIFICATIF SOUS WINDOWS** — le
    /// sous-bloc D3 l'avait introduit dans un composant de chemin puis rattrapé
    /// à la ronde suivante.
    Remontee,
    CaractereInterdit(char),
    /// Un périphérique réservé, replié en minuscules.
    Reserve(String),
    /// Un point ou une espace final : Windows les rogne en silence, donc le
    /// fichier ouvert n'est pas celui qu'on a nommé.
    FinInterdite(char),
}

/// Ce que l'agent fera du fichier une fois posé : `.exe` est exécuté tel quel,
/// `.msi` passe par `msiexec /i` — aucun mode silencieux n'est imposé (spec D8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extension {
    Exe,
    Msi,
}

/// Pourquoi un dépôt est refusé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refus {
    /// L'identifiant d'installation, qui est lui aussi un composant de chemin.
    Identifiant(RefusNom),
    Nom(RefusNom),
    /// 🔴 UN SCRIPT A SON PROPRE MOTIF : un `.bat` n'a pas d'interprète
    /// implicite comme un `.exe`, et le lancer exigerait de choisir un shell,
    /// un répertoire de travail et une politique d'exécution — trois décisions
    /// que personne n'a prises. Le confondre avec « extension inconnue » ferait
    /// chercher un fichier exotique là où il y a une décision à prendre.
    Script(String),
    /// Toute autre extension, repliée en minuscules — vide si le nom n'en
    /// porte aucune.
    Extension(String),
}

/// L'identifiant d'installation, jugé sur une **liste d'autorisation**.
///
/// ⚠️ L'ASYMÉTRIE AVEC `valider_nom` EST DÉLIBÉRÉE. Un identifiant est fabriqué
/// par la plateforme : on peut lui imposer un alphabet, et une liste
/// d'autorisation est la seule qui dise quelque chose des caractères qu'on n'a
/// pas vus. Un nom de fichier est écrit par un humain — accents, espaces et
/// parenthèses y sont légitimes —, et une liste d'autorisation refuserait
/// `Firefox Setup 130.0.exe`.
pub fn valider_identifiant(id: &str) -> Result<(), RefusNom> {
    if id.is_empty() {
        return Err(RefusNom::Vide);
    }
    if id.len() > IDENTIFIANT_MAX_OCTETS {
        return Err(RefusNom::TropLong(id.len()));
    }
    match id.chars().find(|c| !c.is_ascii_alphanumeric() && *c != '-' && *c != '_') {
        Some(c @ ('\\' | '/')) => Err(RefusNom::Separateur(c)),
        Some(c) => Err(RefusNom::CaractereInterdit(c)),
        None => Ok(()),
    }
}

/// Le nom du fichier, jugé sur une liste de refus.
///
/// La remontée est cherchée **avant** le séparateur : c'est le fait le plus
/// dangereux des deux, donc celui qu'un journal doit nommer.
pub fn valider_nom(nom: &str) -> Result<(), RefusNom> {
    if nom.is_empty() {
        return Err(RefusNom::Vide);
    }
    if nom.len() > NOM_MAX_OCTETS {
        return Err(RefusNom::TropLong(nom.len()));
    }
    if nom.contains("..") {
        return Err(RefusNom::Remontee);
    }
    if let Some(c) = nom.chars().find(|c| *c == '\\' || *c == '/') {
        return Err(RefusNom::Separateur(c));
    }
    if let Some(c) = nom.chars().find(|c| CARACTERES_INTERDITS.contains(c) || c.is_control()) {
        return Err(RefusNom::CaractereInterdit(c));
    }
    let tronc = nom.split('.').next().unwrap_or_default().to_lowercase();
    if NOMS_RESERVES.contains(&tronc.as_str()) {
        return Err(RefusNom::Reserve(tronc));
    }
    match nom.chars().next_back() {
        Some(c @ ('.' | ' ')) => Err(RefusNom::FinInterdite(c)),
        _ => Ok(()),
    }
}

/// La règle d'extension, une fois le nom jugé sûr.
pub fn extension_de(nom: &str) -> Result<Extension, Refus> {
    let ext = match nom.rsplit_once('.') {
        Some((_, ext)) => ext.to_lowercase(),
        None => String::new(),
    };
    if let Some((_, retenue)) = EXTENSIONS_RETENUES.iter().find(|(n, _)| *n == ext) {
        Ok(*retenue)
    } else if SCRIPTS.contains(&ext.as_str()) {
        Err(Refus::Script(ext))
    } else {
        Err(Refus::Extension(ext))
    }
}

/// Le répertoire d'une installation, sous la racine donnée.
pub fn repertoire(racine: &str, id: &str) -> Result<String, Refus> {
    valider_identifiant(id).map_err(Refus::Identifiant)?;
    Ok(format!("{}\\{RACINE_RELATIVE}\\{id}", racine.trim_end_matches('\\')))
}

/// Le chemin d'atterrissage, et ce qu'on fera du fichier.
///
/// 🔴 LES DEUX COMPOSANTS SONT JUGÉS, PAS SEULEMENT LE NOM : l'identifiant est
/// lui aussi interpolé dans un chemin, et *ne jamais interpoler une valeur
/// brute dans un composant de chemin* est une règle que ce dépôt a déjà payée.
pub fn chemin(racine: &str, id: &str, nom: &str) -> Result<(String, Extension), Refus> {
    let dossier = repertoire(racine, id)?;
    valider_nom(nom).map_err(Refus::Nom)?;
    Ok((format!("{dossier}\\{nom}"), extension_de(nom)?))
}

/// Ce que les deux marqueurs disent d'une installation déjà vue. `Neuf` : on
/// télécharge et on exécute. `Termine` : on ne fait rien, et l'on réémet le
/// verdict conservé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Etat {
    Neuf,
    /// 🔴 `.commence` SEUL : ON N'EXÉCUTE PAS, et l'on rapporte une issue
    /// inconnue. L'installeur a tourné, son code de sortie est perdu ; rejouer
    /// serait rejouer un installeur sur une machine à l'état **inconnu**,
    /// c'est-à-dire la seule chose dont on ne sache pas sortir.
    Commence,
    Termine,
}

/// La règle, depuis la seule présence des deux marqueurs.
///
/// `.termine` l'emporte : écrit **après** `.commence`, les deux coexistent au
/// repos, et lire `Commence` sur une installation finie la rapporterait
/// inconnue alors que son verdict est là.
pub fn etat(commence: bool, termine: bool) -> Etat {
    match (commence, termine) {
        (_, true) => Etat::Termine,
        (true, false) => Etat::Commence,
        (false, false) => Etat::Neuf,
    }
}

/// Les répertoires à supprimer, depuis leur âge.
///
/// ⚠️ LE BALAYAGE EST OPPORTUNISTE, JAMAIS UN MINUTEUR : il court à chaque
/// réconciliation. Un minuteur d'entretien est une décision d'exploitation —
/// qui l'observe, que fait-il si le disque est plein — hors de ce sous-bloc.
pub fn a_purger(entrees: &[(String, u64)]) -> Vec<&str> {
    entrees
        .iter()
        .filter(|(_, age_ms)| *age_ms > EXPIRATION_INSTALLEUR_MS)
        .map(|(id, _)| id.as_str())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const RACINE: &str = r"C:\ProgramData";

    /// 🔴 LA ROUGE, PREMIÈRE MOITIÉ : le nom est **refusé**, pas assaini en
    /// silence, et l'on compare le refus **et son motif**.
    #[test]
    fn une_remontee_de_chemin_est_refusee_et_le_motif_la_nomme() {
        let mechant = r"..\..\Windows\System32\evil.exe";
        assert_eq!(chemin(RACINE, "i-1", mechant), Err(Refus::Nom(RefusNom::Remontee)));
        // Un séparateur sans remontée a son propre motif : le nom prétend
        // désigner un chemin, ce qui n'est pas la même faute.
        assert_eq!(valider_nom(r"sous\setup.exe"), Err(RefusNom::Separateur('\\')));
        assert_eq!(valider_nom("sous/setup.exe"), Err(RefusNom::Separateur('/')));
    }

    /// 🔴 LA ROUGE, SECONDE MOITIÉ : deux motifs distincts, deux assertions —
    /// un test qui ne regarderait que « refusé » laisserait passer leur
    /// confusion.
    #[test]
    fn un_script_est_refuse_par_son_extension_et_non_par_son_nom() {
        assert_eq!(chemin(RACINE, "i", "setup.bat"), Err(Refus::Script("bat".into())));
        assert_eq!(valider_nom("setup.bat"), Ok(()));
        assert_eq!(chemin(RACINE, "i", "s.zip"), Err(Refus::Extension("zip".into())));
        assert_eq!(chemin(RACINE, "i", "s"), Err(Refus::Extension(String::new())));
    }

    #[test]
    fn le_chemin_nominal_se_compose_et_dit_ce_qu_on_fera_du_fichier() {
        let attendu = r"C:\ProgramData\Guacamole\installeurs\a1-b2_c3\VB_Setup.exe";
        assert_eq!(chemin(RACINE, "a1-b2_c3", "VB_Setup.exe"), Ok((attendu.into(), Extension::Exe)));
        assert_eq!(chemin(RACINE, "i", "Truc.MSI").map(|(_, e)| e), Ok(Extension::Msi));
        // Un nom d'humain passe : accents, espaces, points internes.
        assert!(chemin(RACINE, "i", "Éditeur Pro 3.1 (x64).exe").is_ok());
        // Le chemin le plus long que les deux bornes autorisent tient sous
        // MAX_PATH : c'est ce que leur somme achète.
        let nom = format!("{}.exe", "b".repeat(NOM_MAX_OCTETS - 4));
        let long = chemin(RACINE, &"a".repeat(IDENTIFIANT_MAX_OCTETS), &nom).unwrap();
        assert!(long.0.len() < 260, "{} caractères", long.0.len());
    }

    #[test]
    fn l_identifiant_est_juge_lui_aussi_sur_une_liste_d_autorisation() {
        // Un accent est légitime dans un NOM et refusé dans un IDENTIFIANT :
        // c'est l'asymétrie des deux listes, et elle est voulue.
        for (id, motif) in [
            ("..", RefusNom::CaractereInterdit('.')),
            (r"a\b", RefusNom::Separateur('\\')),
            ("", RefusNom::Vide),
            ("é", RefusNom::CaractereInterdit('é')),
        ] {
            assert_eq!(chemin(RACINE, id, "s.exe"), Err(Refus::Identifiant(motif)), "sur {id:?}");
        }
        let trop = "a".repeat(IDENTIFIANT_MAX_OCTETS + 1);
        assert_eq!(valider_identifiant(&trop), Err(RefusNom::TropLong(trop.len())));
    }

    #[test]
    fn les_autres_refus_de_nom_portent_chacun_le_leur() {
        let trop = "a".repeat(NOM_MAX_OCTETS + 1);
        for (nom, motif) in [
            ("CON.exe", RefusNom::Reserve("con".into())),
            ("lpt9.exe", RefusNom::Reserve("lpt9".into())),
            ("s<1>.exe", RefusNom::CaractereInterdit('<')),
            ("s\u{7}.exe", RefusNom::CaractereInterdit('\u{7}')),
            ("setup.exe ", RefusNom::FinInterdite(' ')),
            ("setup.exe.", RefusNom::FinInterdite('.')),
            ("", RefusNom::Vide),
            (trop.as_str(), RefusNom::TropLong(trop.len())),
        ] {
            assert_eq!(valider_nom(nom), Err(motif), "sur {nom:?}");
        }
    }

    #[test]
    fn les_marqueurs_disent_les_trois_etats_et_termine_l_emporte() {
        assert_eq!(etat(false, false), Etat::Neuf);
        assert_eq!(etat(true, false), Etat::Commence);
        // `.termine` est écrit APRÈS `.commence` : les deux coexistent au repos.
        assert_eq!(etat(true, true), Etat::Termine);
        assert_eq!(etat(false, true), Etat::Termine);
    }

    /// La borne est assiégée des deux côtés : un âge égal à l'expiration est
    /// conservé, une milliseconde de plus part.
    #[test]
    fn la_purge_prend_ce_qui_a_depasse_l_expiration_et_rien_d_autre() {
        let entrees = vec![
            ("neuf".to_string(), 0),
            ("pile".to_string(), EXPIRATION_INSTALLEUR_MS),
            ("juste-apres".to_string(), EXPIRATION_INSTALLEUR_MS + 1),
            ("vieux".to_string(), EXPIRATION_INSTALLEUR_MS * 3),
        ];
        assert_eq!(a_purger(&entrees), vec!["juste-apres", "vieux"]);
        assert!(a_purger(&[]).is_empty());
    }
}
