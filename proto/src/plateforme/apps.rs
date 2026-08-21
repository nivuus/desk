//! Les types de la GESTION D'APPLICATIONS du canal plateforme <-> agent :
//! l'application elle-même, la provenance de son icône, et l'issue d'un ordre
//! de lancement.
//!
//! 🔴 EXTRAIT DE `proto/src/plateforme.rs` VERBATIM (sous-bloc G2), PARCE QUE
//! LE PLAFOND DE 500 LIGNES A ÉTÉ FRANCHI — 588 — ET QUE LA DOCTRINE DU DÉPÔT
//! EST DE RATTRAPER PAR UNE EXTRACTION, JAMAIS PAR UNE COMPRESSION.
//!
//! ⚠️ **L'EXTRACTION AURAIT DÛ PRÉCÉDER L'ADDITION, ET ELLE NE L'A PAS FAIT.**
//! Le plan de G2 avait nommé trois extractions à jouer d'avance — les deux
//! fichiers de tests de `proto/` et `routes-applications.test.ts` — et les
//! trois ont bien été jouées AVANT leur addition. Celle-ci n'était pas prévue :
//! le fichier était annoncé à 433 lignes pour « +1 enum, +1 variante, +2
//! champs », et la documentation de ces additions l'a porté à 588. **Le
//! franchissement est DÉCLARÉ plutôt que dissimulé**, comme ce dépôt l'exige de
//! ses trois franchissements de D10 et de ses deux de D9.
//!
//! La frontière est la MÊME que celle des deux fichiers de tests, et ce n'est
//! pas un hasard : le protocole porte déjà cette coupure — le cycle de vie
//! d'un côté, la gestion d'apps de l'autre.

use serde::{Deserialize, Serialize};

/// D'où vient l'image : la plus grande entrée réellement PRÉSENTE dans le
/// répertoire d'icônes de la source (`GRPICONDIR` d'un module PE, `ICONDIR`
/// d'un `.ico`).
///
/// 🔴 CE N'EST PAS LA TAILLE RENDUE, ET LES DEUX NE DOIVENT JAMAIS ÊTRE
/// CONFONDUES. Mesuré le 20 août 2026 sur deux témoins fabriqués — versés
/// depuis, dans `agent/testdata/g2-temoin-{48,256}.ico` : un `.ico` ne
/// contenant QU'UNE entrée 48×48, interrogé à 256, rend **256×256 32bpp** —
/// par `IShellItemImageFactory::GetImage` comme par `PrivateExtractIconsW`,
/// sans `SIIGBF_SCALEUP` et **MÊME avec `SIIGBF_BIGGERSIZEOK`**, c'est-à-dire
/// en disant explicitement au Shell qu'une taille plus grande conviendrait.
/// **Les quatre lignes de rendu des deux témoins sont identiques ; seule la
/// ligne `ICONDIR` diffère.** Un critère qui comparerait la taille rendue à
/// 256 NE PEUT DONC PAS ÉCHOUER.
///
/// ✅ **ÉPROUVÉ SUR LE PRODUIT le 21 août 2026** : les deux témoins entrés au
/// catalogue réel rendent tous deux un PNG de **256×256**, et `source_max` vaut
/// `{"pixels":48}` pour l'un et `{"pixels":256}` pour l'autre.
///
/// 🔵 **`NonMesuree` S'ÉCRIT EN DEUX MOTS, ET CE N'EST PAS UN HASARD.** Le
/// commentaire d'[`IssueLancement`] inscrit une lacune de couverture : ses
/// quatre variantes étant d'un seul mot, `rename_all` y est INOBSERVABLE et
/// aucun test ne peut rougir si la convention change. `non-mesuree` contre
/// `non_mesuree` la rend observable **pour cet enum-ci** ; ⚠️ elle reste
/// OUVERTE pour `IssueLancement`, qu'aucune tâche de G2 ne touche.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceMax {
    /// La plus grande entrée du répertoire d'icônes, en pixels.
    ///
    /// ⚠️ `bWidth == 0` VAUT 256 dans le format : le champ fait un octet, et
    /// 256 n'y tient pas. La conversion se fait dans le module PUR
    /// `agent::apps::icone::ressource`, jamais ici.
    Pixels(u16),
    /// 🔴 UNE VALEUR DISTINCTE DE 256, ET IL EST INTERDIT DE LES CONFONDRE.
    /// La provenance n'est ni un module PE ni un `.ico` lisible : association
    /// de type, espace de noms Shell, ou ressource illisible. **Mesuré sur le
    /// produit le 21 août 2026 : 71 des 154 applications de cette VM.**
    ///
    /// Sur le fil, serde en fait la chaîne `"non-mesuree"` : elle ne peut
    /// structurellement pas être un nombre, et c'est ce que le critère ④ de
    /// recette demande.
    NonMesuree,
}

/// Une application telle que l'agent la découvre sur le disque de la VM.
///
/// ⚠️ `arguments` est BRUT et SENSIBLE À LA CASSE, contrairement à `cible` et
/// `repertoire` qui sont normalisés (casse repliée). C'est la spec D4 : deux
/// raccourcis qui ne diffèrent que par la casse d'un chemin Windows désignent
/// le même fichier, alors que deux lignes de commande qui ne diffèrent que par
/// la casse d'un argument sont deux invocations distinctes — les replier
/// fusionnerait `-Mode admin` et `-mode Admin`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Application {
    /// Empreinte du triplet `(cible, arguments, repertoire)` — l'identité.
    pub cle: String,
    /// Le nom du `.lnk`, sans son extension.
    pub nom: String,
    /// Le chemin du `.lnk` LUI-MÊME, et c'est lui qu'on lance (spec D6).
    pub chemin: String,
    /// Le chemin de la cible, normalisé.
    pub cible: String,
    /// Les arguments, BRUTS (voir ci-dessus). Vide = `""`, jamais absent.
    pub arguments: String,
    /// Le répertoire de travail, normalisé.
    pub repertoire: String,
    /// L'empreinte SHA-256 du PNG de l'icône, en hexadécimal minuscule — ou
    /// `None` quand l'extraction a échoué.
    ///
    /// ⚠️ UNE APPLICATION SANS ICÔNE VAUT MIEUX QU'UNE APPLICATION ABSENTE
    /// (spec §7). `None` n'est pas une erreur, et s'émet en `"icone":null`.
    ///
    /// ⚠️ **Aucun `#[serde(default)]`, aucun `skip_serializing_if`** : c'est
    /// la règle que ce module s'impose déjà pour le champ `v` — un champ
    /// ABSENT doit être rejeté, pas silencieusement complété. Un `default`
    /// ferait accepter un catalogue d'agent v2 sans que rien ne le dise.
    ///
    /// 🔴 ET C'EST POURQUOI CE CHAMP PORTE UN `deserialize_with` QUI NE FAIT
    /// RIEN D'AUTRE QUE DÉLÉGUER : sans lui, serde rendrait le champ
    /// facultatif TOUT SEUL, parce qu'il est de type `Option`. Voir
    /// [`super::icone_obligatoire`] et la mesure qui y est transcrite.
    #[serde(deserialize_with = "super::icone_obligatoire")]
    pub icone: Option<String>,
    /// Toujours présent. Vaut [`SourceMax::NonMesuree`] quand `icone` est
    /// `None`, et peut aussi le valoir quand `icone` existe — une icône dont
    /// la provenance n'est pas lisible.
    ///
    /// ⚠️ La combinaison inverse — `icone` nul et une taille mesurée — est
    /// INTERDITE, et aucun chemin ne l'écrit.
    pub source_max: SourceMax,
    /// La couleur DOMINANTE de l'icône, en `#rrggbb`, ou `None`.
    ///
    /// 🔴 C'EST LA « COULEUR D'ACCENT » QUE LA CONCEPTION DE ④ DEMANDE AU §G5,
    /// et elle est **PAR APPLICATION** — à ne pas confondre avec celle du
    /// sous-projet ①, qui est **par FENÊTRE**, arrive en cours de session sur
    /// le canal de contrôle WebRTC, et ne décrit pas la même chose. Les deux
    /// se calculent par la même règle pure (`agent::accent::dominante`) ;
    /// c'est leur SUJET qui diffère.
    ///
    /// ⚠️ `None` N'EST PAS UNE ERREUR : une icône trop pâle, trop sombre ou
    /// trop transparente n'a pas de dominante, et `dominante` rend `None` par
    /// construction (sa clause 5). Le manifeste OMET alors `theme_color`
    /// plutôt que d'en inventer un.
    ///
    /// 🔴 MÊME `deserialize_with` QUE `icone`, ET POUR LA MÊME RAISON : sans
    /// lui, serde rendrait le champ facultatif TOUT SEUL parce qu'il est de
    /// type `Option`, et un catalogue d'agent d'une autre version passerait
    /// sans que rien ne le dise.
    #[serde(deserialize_with = "super::icone_obligatoire")]
    pub accent: Option<String>,
    /// Les extensions que cette application ouvre — minuscules, **avec** le
    /// point, triées et dédupliquées.
    ///
    /// 🔴 DES EXTENSIONS, JAMAIS DES TYPES MIME (décision D13 du plan de G5).
    /// Faire voyager le MIME doublerait la table — Rust **et** TypeScript —
    /// pour une donnée qui n'est **pas une propriété de la VM** : c'est une
    /// convention du Web. La carte extension → MIME vit **une seule fois**,
    /// côté plateforme, à l'endroit qui écrit le manifeste.
    ///
    /// ⚠️ VIDE EST UN ÉTAT NORMAL, PAS UNE PANNE : la plupart des applications
    /// n'ouvrent aucun type de fichier. Le champ reste PRÉSENT sur le fil.
    ///
    /// ⚠️ L'ORDRE EST IMPOSÉ PAR L'AGENT (`apps::associations::ranger`) et il
    /// n'est pas décoratif : la plateforme compare le catalogue reçu à celui
    /// qu'elle connaît, et deux listes IDENTIQUES dans un ordre différent la
    /// feraient écrire à chaque tour.
    pub associations: Vec<String>,
}

/// Ce qu'un ordre de lancement a réellement fait.
///
/// 🔴 `Raccourci` CONTRE `Cible` EST CE QUI REND LE CRITÈRE DE RECETTE
/// DÉCIDABLE : lancer par la cible reconstruite au lieu du `.lnk` passerait un
/// critère qui ne dirait que « quelque chose s'est lancé ». Nommer le chemin
/// emprunté distingue les deux sans avoir à ruser.
///
/// ⚠️ CE N'EST PAS UN [`super::MotifCanal`], et le réemployer serait un défaut :
/// deux valeurs de `MotifCanal` FERMENT le socket, et un lancement raté ne
/// doit fermer aucun canal.
///
/// ⚠️ **LACUNE DE COUVERTURE, INSCRITE PLUTÔT QUE SUBIE (recette G1, 20 août
/// 2026).** Le `rename_all` ci-dessous est INOBSERVABLE sur cet enum : ses
/// quatre variantes sont d'UN SEUL MOT, donc `kebab-case`, `snake_case`,
/// `lowercase` et `camelCase` produisent tous les quatre mêmes chaînes.
/// **Aucun test ne peut donc rougir si la convention de nommage change ici**
/// — vérifié par mutation : remplacer `kebab-case` par `snake_case` sur cet
/// enum laisse `cargo test -p proto` à **75 passed, 0 failed**.
///
/// **Le contraste est mesuré sur le même module** : la même mutation appliquée
/// à l'enum qui porte `BattementRecu` — deux mots, donc `battement-recu` contre
/// `battement_recu` — fait ÉCHOUER `conformite_aux_vecteurs_partages`. La
/// protection existe donc bel et bien pour les variantes composées, et pas pour
/// celles-ci.
///
/// ✅ **ET LE SOUS-BLOC G2 EN AJOUTE UN SECOND TÉMOIN** : [`SourceMax`] porte
/// `NonMesuree`, deux mots, dont la casse est éprouvée. **La lacune de CET
/// enum-ci reste entière** — la première variante d'`IssueLancement` écrite en
/// deux mots la refermera d'elle-même, et jusque-là toute modification de cette
/// ligne doit être relue à la main.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IssueLancement {
    /// Le `.lnk` lui-même a été exécuté. C'est le chemin nominal.
    Raccourci,
    /// Le `.lnk` a échoué (disparu, illisible) et la cible enregistrée a pris
    /// le relais.
    Cible,
    /// La clé n'est dans aucun catalogue de l'agent. Rendue par l'appelant,
    /// qui seul connaît le catalogue courant.
    Inconnue,
    /// Le raccourci ET la cible ont échoué. Les deux tentatives sont
    /// journalisées : une issue typée, jamais un silence.
    Echec,
}
