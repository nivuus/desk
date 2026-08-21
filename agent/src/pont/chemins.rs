//! Normalisation d'un chemin livré par ProjFS en chemin logique pour la File
//! System Access API. **PUR** : aucun `cfg`, aucune dépendance à `windows`,
//! entièrement testé sur l'hôte.
//!
//! ProjFS livre `PRJ_CALLBACK_DATA.FilePathName` : un chemin **relatif à la
//! racine de virtualisation**, en contre-obliques, sans lettre de lecteur. Ce
//! module le transforme en chemin logique — composants séparés par `/` — ou le
//! **refuse**. Il ne fait jamais confiance à ce qu'il reçoit : la racine de
//! virtualisation est traversée par n'importe quelle application de la session
//! Windows, y compris hostile.
//!
//! ⚠️ **La CASSE est le piège structurel de ce module, et F1 ne le résout
//! pas.** Windows est insensible à la casse ; la File System Access API ne
//! l'est **pas** : `getFileHandle("Rapport.txt")` échoue là où NTFS aurait
//! ouvert `rapport.txt`. Ce module **conserve la casse** telle que ProjFS l'a
//! livrée — la replier serait pire, puisque la FSA ne retrouverait plus rien
//! du tout — et le défaut est **documenté, pas masqué**.
//!
//! ✅ **LE REMÈDE EST ARRIVÉ EN F3, ET CE N'EST PAS UNE TABLE DE
//! CORRESPONDANCE.** *(Ces lignes annonçaient « une table de correspondance
//! alimentée par l'énumération, qui seule connaît la casse réelle du disque »,
//! et la donnaient comme appartenant « à F3 ou plus tard ».)* F3 livre
//! `client/src/fichiers/noms.ts`, qui **énumère le parent à CHAQUE
//! résolution, SANS AUCUN CACHE** — un cache que rien n'invalide est le défaut
//! de l'ancien pont (`src/file.js`, cache SANS TTL). ✅ **`Rafraichir` EST
//! LIVRÉ DEPUIS F5** (21 août 2026) : un bouton de la page-shell vide le cache
//! d'énumération du pont **et** le cache négatif de ProjFS.
//! ⚠️ **`noms.ts` N'EN PROFITE PAS, et c'est à dire** : il n'a toujours aucun
//! cache, donc rien à vider. Le `Rafraichir` de F5 vide le cache
//! d'ÉNUMÉRATION, qui est un autre objet.
//!
//! ⚠️ **CE MODULE-CI N'A PAS CHANGÉ POUR AUTANT, et c'est délibéré** : il
//! conserve toujours la casse telle que ProjFS l'a livrée. C'est le NAVIGATEUR
//! qui replie, parce que **lui seul voit le poste local**. Ce que F3 ajoute
//! ici est [`avec_dernier_composant`], qui fait redescendre le nom canonique
//! jusqu'à `PrjWritePlaceholderInfo`.
//!
//! ⚠️ **ET LA MOITIÉ VM DU DÉFAUT N'EST PAS RÉPARABLE**, ni ici ni ailleurs :
//! quand NTFS résout la casse sur un fichier DÉJÀ hydraté, nous ne sommes pas
//! consultés. *Le déclarer résolu sans l'avoir mesuré serait exactement le
//! geste que ce dépôt reproche à ses constantes non calibrées.*
//!
//! ❌ **CE MODULE ANNONÇAIT « obtiendra donc `Introuvable` », ET LA RECETTE DE
//! F1 L'A RÉFUTÉ : le défaut réel est PIRE, parce qu'il est SILENCIEUX.**
//! Mesuré sur la VM, **trois exécutions sur trois** (`mesure-exec{1,2,5}.txt`,
//! sous `docs/superpowers/plans/journaux-pont-fichiers/`) : avec `Casse.txt`
//! sur le poste local, `casse.txt` **et** `CASSE.TXT` rendent tous deux le
//! CONTENU de `Casse.txt`, sans erreur — l'application reçoit le mauvais
//! fichier et ne peut pas le savoir. **Et le comportement n'est pas cohérent
//! avec lui-même** : dans la même exécution, `GROS.BIN` rend bien
//! « introuvable ».
//!
//! ⚠️ **Le mécanisme est une HYPOTHÈSE cohérente avec les pièces, pas une
//! mesure.** L'écart suit exactement l'HYDRATATION : la trace
//! `racine hydratee … octets=42 entrees=1` dit qu'une seule entrée de 42
//! octets — `Casse.txt`, lu par la sonde juste avant — vivait en local, et
//! NTFS, insensible à la casse, la retrouve alors **sans jamais atteindre ce
//! module** ; `gros.bin`, jamais hydraté, retombe sur le rappel, qui demande
//! au navigateur une casse qu'il ne connaît pas. **Rien ne l'établit** : il
//! faudrait une exécution où l'ordre d'hydratation est renversé.
//!
//! La comparaison, elle, **replie bien la casse** là où Windows le fait :
//! `CON.txt`, `con.txt` et `Con.TXT` désignent tous le même périphérique
//! réservé, et les trois sont refusés.

/// Pourquoi un chemin est refusé. Une cause par variante : deux causes qui
/// partageraient une variante rendraient le journal inutilisable le jour où
/// l'une des deux se produirait en production.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheminRefuse {
    /// Un composant `..` — la traversée que la spec §4.4 nomme.
    Remontee,
    /// Un flux de données alternatif NTFS (`fichier.txt:Zone.Identifier`).
    FluxAlternatif,
    /// Un nom de périphérique réservé (`CON`, `NUL`, `COM1`…).
    NomReserve,
    /// Une lettre de lecteur, une racine, ou un chemin UNC.
    Absolu,
    /// Un composant vide, produit par un séparateur doublé.
    Vide,
    /// Les unités UTF-16 livrées par ProjFS ne forment pas du texte valide.
    NonUtf16Valide,
}

/// Les noms de périphérique réservés de Win32.
///
/// ⚠️ La liste est celle des **périphériques DOS**, pas une liste de sécurité
/// arbitraire : Windows les résout AVANT de regarder le système de fichiers, à
/// n'importe quelle profondeur, et avec n'importe quelle extension.
const NOMS_RESERVES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Décode les unités UTF-16 d'un `PCWSTR` ProjFS, puis normalise.
///
/// Ce n'est pas une commodité : c'est le seul producteur de
/// [`CheminRefuse::NonUtf16Valide`]. Il vit ici, pur et testé sur l'hôte,
/// plutôt que dans `pont/projfs.rs` où rien ne pourrait l'éprouver.
pub fn normaliser_utf16(unites: &[u16]) -> Result<String, CheminRefuse> {
    let texte: Result<String, _> = char::decode_utf16(unites.iter().copied()).collect();
    match texte {
        Ok(t) => normaliser(&t),
        Err(_) => Err(CheminRefuse::NonUtf16Valide),
    }
}

/// Normalise un chemin ProjFS en chemin logique, ou dit pourquoi il est refusé.
///
/// La chaîne vide est **licite** : c'est le chemin de la racine elle-même,
/// celui que ProjFS livre pour l'énumération du répertoire racine.
pub fn normaliser(brut: &str) -> Result<String, CheminRefuse> {
    if brut.is_empty() {
        return Ok(String::new());
    }
    // Un chemin absolu ou UNC n'est jamais relatif à la racine : ProjFS n'en
    // livre pas, donc en recevoir un signale qu'on n'est pas sur le chemin
    // qu'on croit — refus, pas rattrapage.
    if brut.starts_with('\\') || brut.starts_with('/') || brut.chars().nth(1) == Some(':') {
        return Err(CheminRefuse::Absolu);
    }

    let mut composants = Vec::new();
    for composant in brut.split(['\\', '/']) {
        if composant.is_empty() {
            return Err(CheminRefuse::Vide);
        }
        // `.` est inoffensif et se laisse tomber ; `..` ne se laisse JAMAIS
        // résoudre. Résoudre `a\..\b` en `b` serait déjà une erreur : le
        // chemin franchirait alors la racine sur un `a` inexistant, et
        // surtout `a\..\..\x` deviendrait indistinguable de `x` après une
        // seule passe de résolution. On refuse la remontée, on ne la calcule
        // pas — c'est le seul traitement qui n'ait pas de cas limite.
        if composant == "." {
            continue;
        }
        if composant == ".." {
            return Err(CheminRefuse::Remontee);
        }
        if composant.contains(':') {
            return Err(CheminRefuse::FluxAlternatif);
        }
        if est_reserve(composant) {
            return Err(CheminRefuse::NomReserve);
        }
        composants.push(composant);
    }
    Ok(composants.join("/"))
}

/// Un composant désigne-t-il un périphérique réservé ?
///
/// La comparaison replie la casse et ignore l'extension **et** les points et
/// espaces de fin, exactement comme Win32 : `con`, `CON.txt`, `Con. ` désignent
/// tous le périphérique console.
fn est_reserve(composant: &str) -> bool {
    let base = composant.split('.').next().unwrap_or(composant);
    let base = base.trim_end_matches([' ', '.']);
    NOMS_RESERVES.iter().any(|r| r.eq_ignore_ascii_case(base))
}

#[cfg(test)]
mod tests;

/// Remplace le DERNIER composant d'un chemin ProjFS par `nom`, en gardant les
/// séparateurs et la casse de tout ce qui précède.
///
/// 🔴 **C'est la conséquence ① du canonicaliseur de casse de F3**, et elle est
/// PURE pour être éprouvée sur l'hôte : `PrjWritePlaceholderInfo` doit recevoir
/// le nom **STOCKÉ sur le poste local**, jamais celui que l'application a tapé.
///
/// Sans elle, un `GROS.BIN` demandé sur un `gros.bin` local ferait créer un
/// substitut nommé `GROS.BIN` dans la racine. La racine étant NTFS — donc
/// insensible à la casse —, l'ouverture réussirait ; mais **une énumération du
/// parent rendrait `gros.bin`** : deux noms pour un fichier, dont un qui
/// n'existe nulle part.
///
/// ⚠️ **Le séparateur de ProjFS est `\`**, jamais `/` : ce n'est pas le chemin
/// logique normalisé, c'est celui que le système a livré et qu'il reprendra tel
/// quel.
///
/// Rend `None` quand il n'y a rien à changer — chemin vide, ou dernier
/// composant déjà égal à `nom`. **L'appelant garde alors les octets d'origine**,
/// ce qui préserve la propriété que F1 s'était donnée : ne pas reconvertir un
/// chemin qu'on n'a aucune raison de toucher.
pub fn avec_dernier_composant(chemin_projfs: &str, nom: &str) -> Option<String> {
    if chemin_projfs.is_empty() || nom.is_empty() {
        return None;
    }
    match chemin_projfs.rfind('\\') {
        Some(i) => {
            if &chemin_projfs[i + 1..] == nom {
                return None;
            }
            Some(format!("{}{}", &chemin_projfs[..=i], nom))
        }
        None => {
            if chemin_projfs == nom {
                return None;
            }
            Some(nom.to_string())
        }
    }
}
