# Chantier C, volet 1 — Adaptation réseau : plan d'implémentation

> **Pour les agents :** SOUS-COMPÉTENCE REQUISE : utiliser
> superpowers:subagent-driven-development (recommandé) ou
> superpowers:executing-plans pour dérouler ce plan tâche par tâche. Les étapes
> emploient des cases à cocher (`- [ ]`).

**Objectif :** asservir le débit et la résolution d'encodage à ce que le lien
porte réellement, et le dire à l'utilisateur quand il ne porte plus assez.

**Architecture :** un contrôleur pur (`agent/src/congestion.rs`) reçoit les
observations que str0m produit déjà (estimation BWE, RTT et perte des Receiver
Reports) et rend des décisions ; `transport.rs` les récolte et les applique via
la source vidéo et l'encodeur Opus ; un nouveau message de contrôle porte l'état
du lien jusqu'au navigateur.

**Pile technique :** Rust (str0m 0.21, Media Foundation via windows-rs, libopus),
TypeScript côté client (Vite + vitest), `tc`/`netem` pour le banc d'essai.

**Spec :** `docs/superpowers/specs/2026-07-29-reseau-adaptatif-design.md`

**Volet 2 (traversée NAT / TURN) :** plan séparé,
`docs/superpowers/plans/2026-07-29-traversee-nat.md`. Les deux volets sont
indépendants ; celui-ci n'en dépend pas.

## Contraintes globales

- **Branche** : `chantier-reseau-adaptatif`, créée depuis `main`. Un commit par
  étape « Commit » du plan.
- **La non-régression LAN prime sur tout le reste** : à la fin du plan, la
  recette sur profil `lan` doit encore relever **≥ 55 i/s** et une **médiane de
  latence < 50 ms** (référence mesurée : 62,6 i/s et 47,9 ms,
  `plans/2026-07-28-debit-latence.md`). Un gain sur lien dégradé payé par une
  perte sur LAN est un échec.
- **Toute panne dégrade en le disant, aucune ne tue la session.** Règle déjà
  suivie par `transport.rs` (`échec d'envoi UDP, ignoré`).
- **Journaliser une seule fois** les conditions permanentes (adaptation
  indisponible, refus de l'encodeur), jamais à chaque tour de boucle.
- **Les modules purs ne prennent aucune dépendance à `windows`, à `str0m` ni au
  socket.** C'est ce qui les rend testables sur Linux. `congestion.rs` manipule
  des `u32` en bits par seconde ; la conversion depuis `str0m::bwe::Bitrate` a
  lieu dans `transport.rs`.
- **La VM Windows n'est pas démarrée automatiquement.** Avant toute tâche qui la
  touche : `virsh list --all`, puis `virsh start Windows` et attendre WinRM (voir
  CLAUDE.md, « Cycle de vie de la VM Windows »).
- **Ne pas réemployer `cursor::Hysteresis`** : elle compte des observations
  booléennes consécutives, là où le contrôleur a besoin d'un filtre temporel
  asymétrique sur une échelle à quatre barreaux. Ce sont deux mécanismes
  différents qui portent le même nom de famille.
- **Français** dans les commentaires, la documentation et les messages de
  journal, comme tout le dépôt.

---

## Structure des fichiers

| Fichier | Responsabilité | Tâches |
| --- | --- | --- |
| `scripts/netem.sh` | **Créer.** Poser et retirer un profil de dégradation réseau sur l'hôte, dans les deux sens | 1 |
| `agent/src/congestion.rs` | **Créer.** Contrôleur pur : observations → décisions. Aucune dépendance Windows/str0m/socket | 3, 4, 5 |
| `agent/src/main.rs` | **Modifier.** Déclarer `mod congestion` ; passer le plafond de débit au transport | 3, 9 |
| `agent/src/encode.rs` | **Modifier.** Taille d'encodage distincte de la taille de capture ; débit modifiable à chaud | 6 |
| `agent/src/source.rs` | **Modifier.** Deux méthodes par défaut sur `VideoSource` : `set_bitrate`, `set_encode_size` | 7 |
| `agent/src/windows_source.rs` | **Modifier.** Implémenter ces deux méthodes ; reconstruire l'encodeur seul, pas la capture | 7 |
| `agent/src/opus.rs` | **Modifier.** Exposer `set_packet_loss_perc` | 8 |
| `agent/src/transport.rs` | **Modifier.** Activer le BWE ; récolter `EgressBitrateEstimate` et `MediaEgressStats` ; appliquer les décisions | 9 |
| `proto/src/control.rs` | **Modifier.** `AgentControl::Link`, `CONTROL_VERSION` = 3 | 10 |
| `proto/ts/control.ts` | **Modifier.** Miroir TypeScript | 10 |
| `proto/vectors.json` | **Modifier.** Vecteurs de test partagés à la version 3 | 10 |
| `client/src/webrtc.ts` | **Modifier.** `playoutDelayHint = 0` sur le receiver vidéo | 11 |
| `client/src/lien.ts` | **Créer.** Traduction de `AgentControl::Link` en texte d'indicateur, testable sans DOM | 11 |
| `client/src/main.ts` | **Modifier.** Câbler l'indicateur | 11 |
| `docs/superpowers/plans/2026-07-29-reseau-adaptatif-resultats.md` | **Créer.** Constats de la tâche 2 et recette de la tâche 12 | 2, 12 |

---

## Tâche 1 : Banc netem

Rien de ce que ce plan construit n'est observable sur gigabit. Le banc vient
donc en premier, et il sert aussi à la mesure de la tâche 2.

**Fichiers :**
- Créer : `scripts/netem.sh`

**Interfaces :**
- Produit : `scripts/netem.sh <profil>` où `<profil>` ∈ `lan | adsl | 4g |
  congestionné | effondrement | off`. Consommé par les tâches 2 et 12.

- [ ] **Étape 1 : Relever l'interface réellement traversée**

La VM est sur le pont libvirt dont l'hôte porte `192.168.3.1`. Relever son nom :

```bash
ip -o addr show | grep 192.168.3.1
```

Noter le nom d'interface rendu (typiquement `virbr…`). Il est passé au script
par la variable `IFACE`, avec ce nom comme valeur par défaut.

- [ ] **Étape 2 : Écrire le script**

Le sens agent→navigateur, celui qui porte la vidéo, arrive **en entrée** sur
cette interface : `tc` ne sachant façonner qu'en sortie, il faut le rediriger
vers une interface `ifb`. Sans cette moitié, seul le sens navigateur→agent
serait dégradé et le banc ne mesurerait rien d'utile.

```bash
#!/usr/bin/env bash
# Banc de dégradation réseau pour la recette du chantier C.
#
# Pose un profil sur l'interface du pont de la VM, DANS LES DEUX SENS :
#   - sortant (hôte → VM)      : qdisc netem directement sur $IFACE
#   - entrant (VM → hôte)      : redirigé vers $IFB, car tc ne façonne qu'en
#                                sortie. C'est ce sens qui porte la vidéo.
#
# Usage : scripts/netem.sh <lan|adsl|4g|congestionné|effondrement|off>
set -euo pipefail

IFACE="${IFACE:-virbr1}"
IFB="${IFB:-ifb0}"

profil="${1:-}"
case "$profil" in
    lan)            debit=""        latence=""      gigue=""     perte="" ;;
    adsl)           debit="8mbit"   latence="30ms"  gigue="5ms"  perte="0%" ;;
    4g)             debit="10mbit"  latence="60ms"  gigue="20ms" perte="1%" ;;
    congestionné)   debit="3mbit"   latence="100ms" gigue="20ms" perte="3%" ;;
    effondrement)   debit="500kbit" latence="150ms" gigue="30ms" perte="5%" ;;
    off)            debit=""        latence=""      gigue=""     perte="" ;;
    *)
        echo "profil inconnu : '${profil}'" >&2
        echo "attendus : lan adsl 4g congestionné effondrement off" >&2
        exit 2
        ;;
esac

nettoyer() {
    tc qdisc del dev "$IFACE" root        2>/dev/null || true
    tc qdisc del dev "$IFACE" ingress     2>/dev/null || true
    tc qdisc del dev "$IFB"   root        2>/dev/null || true
}

nettoyer

# `lan` et `off` sont le même état du réseau — aucune qdisc — mais deux
# intentions différentes : `lan` est le profil TÉMOIN de la recette, `off`
# est le retrait du banc. Les distinguer évite d'écrire « recette passée
# sous lan » alors qu'on avait simplement tout retiré.
if [ "$profil" = "lan" ] || [ "$profil" = "off" ]; then
    echo "profil ${profil} : aucune dégradation posée sur ${IFACE}"
    exit 0
fi

modprobe ifb numifbs=1
ip link set dev "$IFB" up

# Sens sortant (hôte → VM).
tc qdisc add dev "$IFACE" root netem \
    delay "$latence" "$gigue" distribution normal \
    loss "$perte" \
    rate "$debit"

# Sens entrant (VM → hôte) : c'est celui qui porte la vidéo.
tc qdisc add dev "$IFACE" handle ffff: ingress
tc filter add dev "$IFACE" parent ffff: protocol all u32 match u32 0 0 \
    action mirred egress redirect dev "$IFB"
tc qdisc add dev "$IFB" root netem \
    delay "$latence" "$gigue" distribution normal \
    loss "$perte" \
    rate "$debit"

echo "profil ${profil} posé sur ${IFACE} et ${IFB} : ${debit}, ${latence} ±${gigue}, perte ${perte}"
```

- [ ] **Étape 3 : Vérifier que le script pose et retire réellement**

```bash
chmod +x scripts/netem.sh
sudo scripts/netem.sh congestionné
tc qdisc show dev virbr1
tc qdisc show dev ifb0
```

Attendu : une qdisc `netem` sur chacune des deux interfaces, avec `rate 3Mbit`.

```bash
sudo scripts/netem.sh off
tc qdisc show dev virbr1
```

Attendu : plus aucune qdisc `netem` — seulement la qdisc par défaut.

- [ ] **Étape 4 : Vérifier que la dégradation est réellement subie**

Avec la VM démarrée (`virsh start Windows`), profil `congestionné` posé :

```bash
sudo scripts/netem.sh congestionné
ping -c 5 192.168.3.2
```

Attendu : un aller-retour d'environ 200 ms (100 ms posés dans chaque sens), à
comparer au sub-milliseconde du profil `lan`. Si le RTT reste sous 10 ms, la
redirection `ifb` n'a pas pris : vérifier le nom d'interface de l'étape 1.

- [ ] **Étape 5 : Commit**

```bash
git add scripts/netem.sh
git commit -m "test(reseau): banc netem à profils nommés, dégradation dans les deux sens"
```

---

## Tâche 2 : Constater ce que NACK et RTX font déjà

La spec (§3.1, §10) l'exige avant toute construction : str0m annonce
`a=rtcp-fb nack` et `a=rtpmap … rtx/90000` dans sa réponse, et Chromium les
offre. Si la résilience est déjà là, il n'y a rien à construire — et il faut
l'écrire plutôt que d'ajouter du code qui ne servirait à rien.

**Fichiers :**
- Créer : `docs/superpowers/plans/2026-07-29-reseau-adaptatif-resultats.md`

**Interfaces :**
- Consomme : `scripts/netem.sh` (tâche 1).
- Produit : un constat écrit. Aucune interface de code.

- [ ] **Étape 1 : Démarrer la VM et l'agent**

```bash
virsh list --all
virsh start Windows
until timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null; do sleep 5; done
set -a && source .env && set +a
scripts/build-agent.sh
scripts/run-agent.sh
```

- [ ] **Étape 2 : Relever la réponse SDP réellement produite**

Sous profil `lan`, connecter le client et capturer le SDP de réponse. Le plus
simple est de l'extraire du journal de l'agent, `accept_offer` produisant la
chaîne rendue :

```bash
grep -i -E "rtcp-fb|rtx|transport-cc" /chemin/vers/agent.log
```

Consigner **littéralement** les lignes trouvées. Ce qu'on cherche à savoir :
`a=rtcp-fb:<pt> nack` est-il présent, `a=rtpmap:<pt> rtx/90000` est-il présent,
et `a=extmap:… transport-wide-cc` est-il négocié (ce dernier conditionne le BWE
de la tâche 9).

- [ ] **Étape 3 : Mesurer sous perte réelle**

```bash
sudo scripts/netem.sh 4g
```

Côté navigateur, relever dans `getStats()` : `nackCount` et
`retransmittedPacketsSent` de l'entrée `outbound-rtp` distante, ou à défaut
`nackCount` de l'entrée `inbound-rtp` locale (le navigateur compte les NACK
qu'il émet). Côté agent, `MediaEgressStats.nacks` n'est pas encore journalisé —
l'ajouter temporairement suffit :

```rust
Event::MediaEgressStats(stats) => {
    tracing::info!(nacks = stats.nacks, plis = stats.plis, rtt = ?stats.rtt, "stats sortantes");
}
```

- [ ] **Étape 4 : Écrire le constat**

Créer `docs/superpowers/plans/2026-07-29-reseau-adaptatif-resultats.md` avec
une section « §1 NACK et RTX : ce qui existait déjà », qui répond par oui ou non,
chiffres à l'appui :

- les NACK sont-ils émis par le navigateur sous perte ?
- l'agent retransmet-il (compteur `nacks` non nul et paquets RTX vus) ?
- `transport-wide-cc` est-il négocié ?

Si la réponse à la troisième question est **non**, la tâche 9 devra le faire
négocier — c'est un prérequis du BWE, et le découvrir ici coûte une mesure
plutôt qu'une tâche entière de débogage.

- [ ] **Étape 5 : Retirer l'instrumentation temporaire et committer le constat**

```bash
sudo scripts/netem.sh off
git checkout agent/src/transport.rs
git add docs/superpowers/plans/2026-07-29-reseau-adaptatif-resultats.md
git commit -m "mesure(reseau): constater ce que NACK, RTX et TWCC font déjà"
```

---

## Tâche 3 : Contrôleur — échelle de résolutions et sélection du barreau

**Fichiers :**
- Créer : `agent/src/congestion.rs`
- Modifier : `agent/src/main.rs` (déclaration du module)

**Interfaces :**
- Produit : `congestion::{Config, Echelle, Barreau}`, `Echelle::depuis(source,
  fps)`, `Echelle::barreaux()`, `Echelle::barreau_finance(bps) -> usize`.
  Consommé par les tâches 4, 5 et 9.

- [ ] **Étape 1 : Écrire le test qui échoue**

Créer `agent/src/congestion.rs` avec, pour tout contenu, ce module de tests :

```rust
//! Contrôleur de congestion : décide du débit et de la résolution d'encodage
//! à partir de ce que le pair rapporte.
//!
//! Aucune dépendance à Windows, à str0m ni au socket — c'est ce qui rend
//! toute la politique testable sur Linux, sans VM et sans réseau. Même
//! raison d'être que `geometry.rs`, `rebuild.rs` et `clock.rs`.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_echelle_a_quatre_barreaux_decroissants_et_pairs() {
        let echelle = Echelle::depuis((1920, 1080), 60);
        let tailles: Vec<(u32, u32)> = echelle.barreaux().iter().map(|b| b.taille).collect();

        assert_eq!(tailles.len(), 4, "quatre barreaux attendus");
        assert_eq!(tailles[0], (1920, 1080), "le premier barreau est la taille source");
        for (i, (w, h)) in tailles.iter().enumerate() {
            assert_eq!(w % 2, 0, "barreau {i} : largeur impaire, refusée par H.264");
            assert_eq!(h % 2, 0, "barreau {i} : hauteur impaire, refusée par H.264");
        }
        for i in 1..tailles.len() {
            assert!(
                tailles[i].0 < tailles[i - 1].0,
                "barreau {i} pas plus petit que le précédent : {:?}",
                tailles
            );
        }
    }

    #[test]
    fn le_barreau_finance_est_le_plus_haut_que_le_debit_paie() {
        let echelle = Echelle::depuis((1920, 1080), 60);
        let barreaux = echelle.barreaux();

        // Très large : le barreau 0.
        assert_eq!(echelle.barreau_finance(50_000_000), 0);

        // Juste au minimum du barreau 0 : encore le barreau 0.
        assert_eq!(echelle.barreau_finance(barreaux[0].min_bps), 0);

        // Un bit sous le minimum du barreau 0 : on descend d'un cran.
        assert_eq!(echelle.barreau_finance(barreaux[0].min_bps - 1), 1);

        // Sous le minimum du dernier barreau : on reste au dernier, c'est le
        // plancher. Déclarer l'insuffisance est le rôle du contrôleur
        // (tâche 5), pas celui de l'échelle.
        assert_eq!(echelle.barreau_finance(0), barreaux.len() - 1);
    }
}
```

- [ ] **Étape 2 : Lancer le test pour vérifier qu'il échoue**

Déclarer d'abord le module. Dans `agent/src/main.rs`, ajouter la ligne dans le
bloc non conditionné (celui qui commence par `mod audio;`), en ordre
alphabétique — donc entre `mod clock;` et `mod cursor;` :

```rust
mod congestion;
```

Puis :

```bash
cargo test -p agent congestion
```

Attendu : ÉCHEC de compilation — `cannot find type 'Echelle' in this scope`.

- [ ] **Étape 3 : Écrire l'implémentation minimale**

Au-dessus du module de tests, dans `agent/src/congestion.rs` :

```rust
/// Diviseurs successifs appliqués à la taille source pour former l'échelle.
///
/// Quatre barreaux, choisis pour que chaque descente soit visible sans être
/// brutale : de 1080p on passe à 864p, puis 720p, puis 540p.
const DIVISEURS: [f32; 4] = [1.0, 1.25, 1.5, 2.0];

/// Débit minimal, en bits par pixel et par image, en dessous duquel un barreau
/// devient laid.
///
/// **C'est LE réglage du contrôleur.** La valeur de départ est choisie pour
/// donner une échelle cohérente sous le plafond de 12 Mb/s en 1080p60 (6,2 →
/// 4,0 → 2,8 → 1,6 Mb/s), pas mesurée. La tâche 12 la confirme ou la corrige
/// sur le banc netem, et consigne l'ajustement.
const BPP_MIN: f32 = 0.05;

/// Un barreau de l'échelle : une taille d'encodage et le débit en dessous
/// duquel elle cesse d'être regardable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Barreau {
    pub taille: (u32, u32),
    pub min_bps: u32,
}

/// Échelle de résolutions dérivée d'une taille source et d'une cadence.
#[derive(Debug, Clone)]
pub struct Echelle {
    barreaux: Vec<Barreau>,
}

impl Echelle {
    pub fn depuis(source: (u32, u32), fps: u32) -> Self {
        let (sw, sh) = source;
        let barreaux = DIVISEURS
            .iter()
            .map(|d| {
                // `& !1` : H.264 exige des dimensions paires. La même
                // contrainte est déjà appliquée par `WindowsSource::resize`.
                // `.max(2)` empêche une source minuscule de produire une
                // dimension nulle, que Media Foundation refuserait.
                let w = (((sw as f32) / d) as u32 & !1).max(2);
                let h = (((sh as f32) / d) as u32 & !1).max(2);
                let pixels = w as u64 * h as u64;
                let min_bps = (pixels * fps as u64) as f32 * BPP_MIN;
                Barreau { taille: (w, h), min_bps: min_bps as u32 }
            })
            .collect();
        Self { barreaux }
    }

    pub fn barreaux(&self) -> &[Barreau] {
        &self.barreaux
    }

    /// Indice du barreau le plus haut que `disponible_bps` finance.
    ///
    /// Rend le dernier barreau quand rien ne le finance : l'échelle n'a pas
    /// de barreau en dessous. Constater l'insuffisance est le rôle du
    /// contrôleur, qui seul sait qu'on est au plancher.
    pub fn barreau_finance(&self, disponible_bps: u32) -> usize {
        self.barreaux
            .iter()
            .position(|b| disponible_bps >= b.min_bps)
            .unwrap_or(self.barreaux.len() - 1)
    }
}
```

- [ ] **Étape 4 : Lancer le test pour vérifier qu'il passe**

```bash
cargo test -p agent congestion
```

Attendu : SUCCÈS, 2 tests.

- [ ] **Étape 5 : Commit**

```bash
git add agent/src/congestion.rs agent/src/main.rs
git commit -m "feat(reseau): échelle de résolutions et sélection du barreau financé"
```

---

## Tâche 4 : Contrôleur — hystérésis asymétrique et temps de séjour

**Fichiers :**
- Modifier : `agent/src/congestion.rs`

**Interfaces :**
- Consomme : `Echelle`, `Barreau` (tâche 3).
- Produit : `congestion::Hysteresis` avec
  `Hysteresis::new(barreau_initial: usize, now: Instant)` et
  `observer(&mut self, vise: usize, now: Instant) -> Option<usize>`. Consommé
  par la tâche 5.

- [ ] **Étape 1 : Écrire les tests qui échouent**

Ajouter dans le module `tests` de `agent/src/congestion.rs` :

```rust
    use std::time::{Duration, Instant};

    /// Instant de référence des tests. Placé loin dans le passé pour que
    /// toute soustraction de durée reste valide.
    fn t0() -> Instant {
        Instant::now() - Duration::from_secs(3600)
    }

    #[test]
    fn descendre_exige_deux_secondes_sous_le_barreau() {
        let mut h = Hysteresis::new(0, t0());

        // À 1,9 s, pas encore.
        assert_eq!(h.observer(1, t0() + Duration::from_millis(1900)), None);
        // À 2,0 s, on descend.
        assert_eq!(h.observer(1, t0() + Duration::from_millis(2000)), Some(1));
    }

    #[test]
    fn un_repit_remet_le_compteur_de_descente_a_zero() {
        let mut h = Hysteresis::new(0, t0());

        assert_eq!(h.observer(1, t0() + Duration::from_millis(1900)), None);
        // Une seule observation revenue au barreau courant annule la descente.
        assert_eq!(h.observer(0, t0() + Duration::from_millis(1950)), None);
        // Le compteur repart de 1950 ms : à 3000 ms il n'y a qu'1,05 s.
        assert_eq!(h.observer(1, t0() + Duration::from_millis(3000)), None);
        assert_eq!(h.observer(1, t0() + Duration::from_millis(3960)), Some(1));
    }

    #[test]
    fn remonter_exige_dix_secondes_et_non_deux() {
        // Départ au barreau 1, temps de séjour déjà écoulé.
        let mut h = Hysteresis::new(1, t0());
        let depart = t0() + Duration::from_secs(10);

        assert_eq!(h.observer(0, depart + Duration::from_millis(9900)), None);
        assert_eq!(h.observer(0, depart + Duration::from_millis(10_000)), Some(0));
    }

    #[test]
    fn le_temps_de_sejour_bloque_un_second_changement_trop_proche() {
        let mut h = Hysteresis::new(0, t0());

        // Première descente à 2 s.
        assert_eq!(h.observer(1, t0() + Duration::from_secs(2)), Some(1));

        // La condition de descente vers 2 est remplie 2 s plus tard (t = 4 s),
        // mais le temps de séjour de 5 s depuis le changement l'interdit.
        assert_eq!(h.observer(2, t0() + Duration::from_secs(4)), None);
        assert_eq!(h.observer(2, t0() + Duration::from_millis(6900)), None);
        // À t = 7 s, les 5 s de séjour sont écoulées ET la condition tient
        // depuis plus de 2 s.
        assert_eq!(h.observer(2, t0() + Duration::from_secs(7)), Some(2));
    }
```

- [ ] **Étape 2 : Lancer les tests pour vérifier qu'ils échouent**

```bash
cargo test -p agent congestion
```

Attendu : ÉCHEC de compilation — `cannot find type 'Hysteresis' in this scope`.

- [ ] **Étape 3 : Écrire l'implémentation**

Ajouter dans `agent/src/congestion.rs`, au-dessus du module de tests :

```rust
use std::time::{Duration, Instant};

/// Durée pendant laquelle la condition doit tenir avant de DESCENDRE.
const DELAI_DESCENTE: Duration = Duration::from_secs(2);
/// Durée pendant laquelle la condition doit tenir avant de REMONTER.
///
/// Cinq fois plus long que la descente, et c'est délibéré : une estimation
/// qui oscille autour d'un seuil ferait sinon battre l'encodeur, et chaque
/// battement coûte une reconstruction du type de sortie et une image clé.
/// On dégrade vite pour rester fluide, on restaure lentement pour rester
/// stable.
const DELAI_REMONTEE: Duration = Duration::from_secs(10);
/// Durée minimale entre deux changements de barreau, quelle que soit la
/// condition. Filet contre un aller-retour rapide autour d'un seuil.
const SEJOUR_MINIMAL: Duration = Duration::from_secs(5);

/// Filtre temporel asymétrique sur un indice de barreau.
///
/// Rend `Some(nouvel_indice)` à l'instant précis où un changement est retenu,
/// et `None` sinon. L'appelant n'a rien à mémoriser.
///
/// **À ne pas confondre avec `cursor::Hysteresis`**, qui compte des
/// observations booléennes consécutives : ici le filtre est temporel,
/// asymétrique, et porte sur une échelle ordonnée.
pub struct Hysteresis {
    courant: usize,
    /// Barreau visé de façon continue depuis `vise_depuis`, s'il diffère du
    /// courant.
    vise: Option<(usize, Instant)>,
    /// Instant du dernier changement retenu.
    dernier_changement: Instant,
}

impl Hysteresis {
    pub fn new(barreau_initial: usize, now: Instant) -> Self {
        Self {
            courant: barreau_initial,
            vise: None,
            // Placé de façon à ce que le temps de séjour soit déjà écoulé au
            // démarrage : la toute première adaptation ne doit pas attendre
            // 5 s de plus que sa propre condition.
            dernier_changement: now - SEJOUR_MINIMAL,
        }
    }

    pub fn observer(&mut self, vise: usize, now: Instant) -> Option<usize> {
        if vise == self.courant {
            // Retour au barreau courant : toute intention de changement en
            // cours est annulée.
            self.vise = None;
            return None;
        }

        // Un barreau visé DIFFÉRENT de celui déjà en cours d'observation
        // redémarre le décompte : la condition n'a pas « tenu », elle a
        // changé de cible.
        let depuis = match self.vise {
            Some((precedent, depuis)) if precedent == vise => depuis,
            _ => {
                self.vise = Some((vise, now));
                now
            }
        };

        // Indices croissants = résolutions décroissantes : viser plus grand
        // que le courant, c'est descendre.
        let delai = if vise > self.courant { DELAI_DESCENTE } else { DELAI_REMONTEE };
        if now.duration_since(depuis) < delai {
            return None;
        }
        if now.duration_since(self.dernier_changement) < SEJOUR_MINIMAL {
            return None;
        }

        self.courant = vise;
        self.vise = None;
        self.dernier_changement = now;
        Some(vise)
    }
}
```

- [ ] **Étape 4 : Lancer les tests pour vérifier qu'ils passent**

```bash
cargo test -p agent congestion
```

Attendu : SUCCÈS, 6 tests.

- [ ] **Étape 5 : Commit**

```bash
git add agent/src/congestion.rs
git commit -m "feat(reseau): hystérésis asymétrique et temps de séjour sur l'échelle"
```

---

## Tâche 5 : Contrôleur — débit, marge, perte et absence d'estimation

**Fichiers :**
- Modifier : `agent/src/congestion.rs`

**Interfaces :**
- Consomme : `Echelle`, `Hysteresis` (tâches 3, 4).
- Produit : `congestion::{Controleur, Config, Observation, Decision, Qualite}`,
  `Controleur::new(Config, Instant)`, `observer(Observation) -> Option<Decision>`,
  `courant() -> Decision`. Consommé par la tâche 9.

- [ ] **Étape 1 : Écrire les tests qui échouent**

Ajouter dans le module `tests` :

```rust
    fn config() -> Config {
        Config {
            plafond_bps: 12_000_000,
            audio_bps: 128_000,
            source: (1920, 1080),
            fps: 60,
        }
    }

    fn obs(estimate_bps: Option<u32>, loss: Option<f32>, at: Instant) -> Observation {
        Observation { estimate_bps, rtt: None, loss, at }
    }

    #[test]
    fn sans_estimation_le_debit_reste_au_plafond_et_l_adaptation_est_indisponible() {
        let mut c = Controleur::new(config(), t0());
        let d = c.courant();

        assert_eq!(d.video_bitrate_bps, 12_000_000);
        assert_eq!(d.encode_size, (1920, 1080));
        assert_eq!(d.adaptation, Adaptation::Indisponible);
        assert_eq!(d.qualite, Qualite::Bonne);

        // Cent observations sans estimation ne changent rien et ne
        // produisent aucune décision.
        for i in 0..100 {
            let at = t0() + Duration::from_millis(i * 100);
            assert_eq!(c.observer(obs(None, None, at)), None);
        }
        assert_eq!(c.courant().adaptation, Adaptation::Indisponible);
    }

    #[test]
    fn la_premiere_estimation_rend_l_adaptation_active() {
        let mut c = Controleur::new(config(), t0());
        let d = c
            .observer(obs(Some(9_000_000), None, t0() + Duration::from_secs(1)))
            .expect("la première estimation doit produire une décision");

        assert_eq!(d.adaptation, Adaptation::Active);
        // 9 Mb/s × 0,9 − 128 kb/s d'audio = 7,972 Mb/s.
        assert_eq!(d.video_bitrate_bps, 7_972_000);
        // 7,97 Mb/s finance encore le barreau 0 (minimum 6,2 Mb/s).
        assert_eq!(d.encode_size, (1920, 1080));
        assert_eq!(d.qualite, Qualite::Bonne);
    }

    #[test]
    fn le_debit_ne_bouge_pas_pour_moins_de_dix_pour_cent_d_ecart() {
        let mut c = Controleur::new(config(), t0());
        c.observer(obs(Some(9_000_000), None, t0() + Duration::from_secs(1)))
            .expect("première décision");

        // +5 % : sous le seuil, aucune décision.
        assert_eq!(c.observer(obs(Some(9_450_000), None, t0() + Duration::from_secs(3))), None);
        // +20 % : au-delà du seuil, décision produite.
        assert!(c.observer(obs(Some(10_800_000), None, t0() + Duration::from_secs(5))).is_some());
    }

    #[test]
    fn le_debit_ne_depasse_jamais_le_plafond() {
        let mut c = Controleur::new(config(), t0());
        let d = c
            .observer(obs(Some(80_000_000), None, t0() + Duration::from_secs(1)))
            .expect("décision");
        assert_eq!(d.video_bitrate_bps, 12_000_000, "le plafond BITRATE doit borner");
    }

    #[test]
    fn une_contrainte_durable_fait_descendre_un_barreau_et_marque_la_degradation() {
        let mut c = Controleur::new(config(), t0());
        c.observer(obs(Some(9_000_000), None, t0() + Duration::from_secs(1)));

        // 5 Mb/s : sous le minimum du barreau 0 (6,2 Mb/s). Il faut 2 s.
        let mut derniere = None;
        for i in 10..40 {
            let at = t0() + Duration::from_millis(i * 200);
            if let Some(d) = c.observer(obs(Some(5_000_000), None, at)) {
                derniere = Some(d);
            }
        }
        let d = derniere.expect("une décision devait tomber");
        assert_ne!(d.encode_size, (1920, 1080), "la résolution devait descendre");
        assert_eq!(d.qualite, Qualite::Degradee);
    }

    #[test]
    fn sous_le_plancher_la_qualite_est_declaree_insuffisante() {
        let mut c = Controleur::new(config(), t0());
        c.observer(obs(Some(9_000_000), None, t0() + Duration::from_secs(1)));

        let mut derniere = None;
        for i in 10..200 {
            let at = t0() + Duration::from_millis(i * 200);
            if let Some(d) = c.observer(obs(Some(300_000), None, at)) {
                derniere = Some(d);
            }
        }
        let d = derniere.expect("une décision devait tomber");
        assert_eq!(d.qualite, Qualite::Insuffisante);
        // On est descendu au dernier barreau, pas plus bas : la cadence n'est
        // jamais sacrifiée automatiquement.
        let echelle = Echelle::depuis((1920, 1080), 60);
        assert_eq!(d.encode_size, echelle.barreaux().last().unwrap().taille);
    }

    #[test]
    fn la_perte_est_convertie_en_pourcentage_et_plafonnee_a_vingt_cinq() {
        let mut c = Controleur::new(config(), t0());

        let d = c
            .observer(obs(Some(9_000_000), Some(0.03), t0() + Duration::from_secs(1)))
            .expect("décision");
        assert_eq!(d.opus_loss_perc, 3);

        // 60 % de perte : plafonné à 25, au-delà duquel la redondance coûte
        // plus qu'elle ne sauve.
        let d = c
            .observer(obs(Some(9_000_000), Some(0.60), t0() + Duration::from_secs(3)))
            .expect("décision");
        assert_eq!(d.opus_loss_perc, 25);
    }
```

- [ ] **Étape 2 : Lancer les tests pour vérifier qu'ils échouent**

```bash
cargo test -p agent congestion
```

Attendu : ÉCHEC de compilation — `cannot find type 'Controleur' in this scope`.

- [ ] **Étape 3 : Écrire l'implémentation**

Ajouter dans `agent/src/congestion.rs` :

```rust
/// Part de l'estimation qu'on s'autorise à consommer.
///
/// Les 10 % restants laissent la place aux retransmissions RTX et aux paquets
/// de sondage que le sous-système BWE émet pour tester à la hausse. Viser
/// 100 % de l'estimation, c'est garantir de la dépasser.
const MARGE: f32 = 0.9;

/// Écart relatif en dessous duquel on ne reconfigure pas le débit. Sans lui,
/// une estimation qui frémit ferait écrire l'encodeur à chaque seconde.
const ECART_MINIMAL_DEBIT: f32 = 0.10;

/// Plafond du pourcentage de perte déclaré à Opus. Au-delà, la redondance
/// LBRR coûte plus de débit qu'elle n'en sauve.
const PERTE_MAX_OPUS: i32 = 25;

/// Réglages figés d'une session.
#[derive(Debug, Clone, Copy)]
pub struct Config {
    /// Plafond de débit vidéo, en bits par seconde (variable `BITRATE`).
    /// Sert aussi de valeur de repli quand aucune estimation n'arrive.
    pub plafond_bps: u32,
    /// Budget réservé à la piste audio, retiré de l'estimation.
    pub audio_bps: u32,
    /// Taille de la source capturée, sommet de l'échelle.
    pub source: (u32, u32),
    pub fps: u32,
}

/// Ce que le transport observe, une fois par seconde.
#[derive(Debug, Clone, Copy)]
pub struct Observation {
    /// Estimation de bande passante sortante. `None` tant qu'aucune n'est
    /// arrivée — cas normal au démarrage, cas permanent si TWCC n'est pas
    /// négocié.
    pub estimate_bps: Option<u32>,
    pub rtt: Option<Duration>,
    /// Fraction de paquets perdus, entre 0 et 1.
    pub loss: Option<f32>,
    pub at: Instant,
}

/// État du lien tel qu'on l'annonce à l'utilisateur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Qualite {
    /// Barreau le plus haut.
    Bonne,
    /// Résolution réduite : l'utilisateur doit savoir pourquoi l'image a molli.
    Degradee,
    /// Plancher atteint. On ne dégrade plus — on le dit.
    Insuffisante,
}

/// Le contrôleur reçoit-il de quoi s'asservir ?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Adaptation {
    Active,
    /// Aucune estimation n'est jamais arrivée. Le débit reste au plafond, et
    /// ce fait doit être annoncé — un silence ressemblerait à « tout va bien ».
    Indisponible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decision {
    pub video_bitrate_bps: u32,
    pub encode_size: (u32, u32),
    pub opus_loss_perc: i32,
    pub qualite: Qualite,
    pub adaptation: Adaptation,
}

pub struct Controleur {
    config: Config,
    echelle: Echelle,
    hysteresis: Hysteresis,
    courant: Decision,
    /// Vrai dès la première estimation reçue.
    estimation_vue: bool,
}

impl Controleur {
    pub fn new(config: Config, now: Instant) -> Self {
        let echelle = Echelle::depuis(config.source, config.fps);
        let courant = Decision {
            video_bitrate_bps: config.plafond_bps,
            encode_size: echelle.barreaux()[0].taille,
            opus_loss_perc: 0,
            qualite: Qualite::Bonne,
            adaptation: Adaptation::Indisponible,
        };
        Self {
            config,
            echelle,
            hysteresis: Hysteresis::new(0, now),
            courant,
            estimation_vue: false,
        }
    }

    /// Décision actuellement appliquée. Sert au démarrage, avant toute
    /// observation, et à alimenter le message d'état du lien.
    pub fn courant(&self) -> Decision {
        self.courant
    }

    pub fn observer(&mut self, o: Observation) -> Option<Decision> {
        let Some(estimate) = o.estimate_bps else {
            // Sans estimation, rien à asservir. On ne touche à rien et on ne
            // produit aucune décision : le débit de repli est déjà celui
            // posé à la construction.
            return None;
        };
        self.estimation_vue = true;

        // Part vidéo : marge de sécurité, moins le budget audio, borné au
        // plafond. `saturating_sub` : une estimation plus basse que le seul
        // budget audio ne doit pas déborder.
        let disponible = ((estimate as f32 * MARGE) as u32)
            .saturating_sub(self.config.audio_bps)
            .min(self.config.plafond_bps);

        let vise = self.echelle.barreau_finance(disponible);
        if let Some(nouveau) = self.hysteresis.observer(vise, o.at) {
            self.courant.encode_size = self.echelle.barreaux()[nouveau].taille;
        }

        let dernier = self.echelle.barreaux().len() - 1;
        let barreau_applique = self
            .echelle
            .barreaux()
            .iter()
            .position(|b| b.taille == self.courant.encode_size)
            .unwrap_or(0);

        let qualite = if barreau_applique == dernier
            && disponible < self.echelle.barreaux()[dernier].min_bps
        {
            Qualite::Insuffisante
        } else if barreau_applique > 0 {
            Qualite::Degradee
        } else {
            Qualite::Bonne
        };

        let perte = o
            .loss
            .map(|l| ((l * 100.0).round() as i32).clamp(0, PERTE_MAX_OPUS))
            .unwrap_or(self.courant.opus_loss_perc);

        let debit_change = ecart_relatif(self.courant.video_bitrate_bps, disponible)
            >= ECART_MINIMAL_DEBIT;
        let change = debit_change
            || qualite != self.courant.qualite
            || perte != self.courant.opus_loss_perc
            || self.courant.adaptation != Adaptation::Active
            || self.courant.encode_size != self.echelle.barreaux()[barreau_applique].taille;

        if debit_change {
            self.courant.video_bitrate_bps = disponible;
        }
        self.courant.qualite = qualite;
        self.courant.opus_loss_perc = perte;
        self.courant.adaptation = Adaptation::Active;

        change.then_some(self.courant)
    }
}

/// Écart relatif entre deux débits, rapporté au plus grand des deux pour
/// rester symétrique — sinon une division par un `avant` nul exploserait, et
/// une hausse de 1 à 2 ne pèserait pas comme une baisse de 2 à 1.
fn ecart_relatif(avant: u32, apres: u32) -> f32 {
    let max = avant.max(apres);
    if max == 0 {
        return 0.0;
    }
    (avant as f32 - apres as f32).abs() / max as f32
}
```

- [ ] **Étape 4 : Lancer les tests pour vérifier qu'ils passent**

```bash
cargo test -p agent congestion
```

Attendu : SUCCÈS, 13 tests.

- [ ] **Étape 5 : Vérifier l'absence d'avertissement**

```bash
cargo clippy -p agent -- -D warnings 2>&1 | tail -20
```

Attendu : aucune erreur. `Observation.rtt` n'est pas encore lu — s'il déclenche
un avertissement de champ mort, ne PAS le supprimer : il est consommé par le
journal de la tâche 9. Ajouter à la place, sur le champ :

```rust
    /// Non consommé par la décision : journalisé par `transport.rs` pour que
    /// la recette dispose du RTT vu par l'agent, à confronter à celui que le
    /// navigateur rapporte.
    pub rtt: Option<Duration>,
```

- [ ] **Étape 6 : Commit**

```bash
git add agent/src/congestion.rs
git commit -m "feat(reseau): contrôleur de congestion, débit borné et qualité déclarée"
```

---

## Tâche 6 : Encodeur — taille d'encodage distincte, débit à chaud

Le convertisseur BGRA→NV12 (`create_color_converter`) reçoit aujourd'hui une
seule taille pour son entrée et sa sortie. Les séparer est ce qui permet de
réduire la résolution transportée **sans toucher à la fenêtre Windows**.

**Fichiers :**
- Modifier : `agent/src/encode.rs`

**Interfaces :**
- Produit : `H264Encoder::new(device, capture: (u32,u32), encode: (u32,u32),
  fps, bitrate)` (signature changée), `H264Encoder::set_bitrate(&mut self, u32)
  -> Result<()>`, `H264Encoder::encode_size(&self) -> (u32,u32)`. Consommé par
  la tâche 7.

> **Ce module ne compile que sous Windows** (`#[cfg(windows)]` dans `main.rs`).
> La vérification se fait sur la VM : `scripts/build-agent.sh`. Ne pas tenter de
> compilation croisée — `aws-lc-sys` et `audiopus_sys` compilent du C pour la
> cible Windows, ce qu'aucun cross-compilateur de l'hôte ne sait faire (défaut
> de plan déjà rencontré au chantier A).

- [ ] **Étape 1 : Séparer la taille de capture de la taille d'encodage**

Dans `create_color_converter`, remplacer la signature et les deux types :

```rust
fn create_color_converter(
    device_manager: &IMFDXGIDeviceManager,
    capture: (u32, u32),
    encode: (u32, u32),
    fps: u32,
) -> Result<IMFTransform> {
```

Dans le corps, le **type d'entrée** garde la taille de capture :

```rust
        input_type.SetUINT64(&MF_MT_FRAME_SIZE, pack_u64(capture.0, capture.1))?;
```

et le **type de sortie** prend la taille d'encodage — c'est cette seule ligne
qui fait mettre à l'échelle par le Video Processor MFT :

```rust
        output_type.SetUINT64(&MF_MT_FRAME_SIZE, pack_u64(encode.0, encode.1))?;
```

- [ ] **Étape 2 : Répercuter sur `H264Encoder::new`**

```rust
    pub fn new(
        device: &ID3D11Device,
        capture: (u32, u32),
        encode: (u32, u32),
        fps: u32,
        bitrate: u32,
    ) -> Result<Self> {
```

Dans le corps, l'encodeur travaille **entièrement** en taille d'encodage — c'est
le convertisseur qui a déjà réduit :

```rust
        configure_output(&transform, encode.0, encode.1, fps, bitrate)?;
        configure_input(&transform, encode.0, encode.1, fps)?;
        configure_rate_control(&transform, bitrate)?;
```

et plus bas :

```rust
        let converter = create_color_converter(&device_manager, capture, encode, fps)?;
```

Remplacer les champs `width`/`height` de la structure par les deux tailles.
Dans la déclaration de `pub struct H264Encoder` :

```rust
    /// Taille des textures BGRA remises par la capture — l'entrée du
    /// convertisseur.
    capture: (u32, u32),
    /// Taille réellement encodée et transportée — la sortie du convertisseur
    /// et l'entrée de l'encodeur. Peut être plus petite que `capture` : c'est
    /// le levier de résolution adaptative, et il ne touche pas à la fenêtre
    /// Windows (contrairement à `WindowsSource::resize`).
    encode: (u32, u32),
```

et dans le littéral `Ok(Self { … })`, remplacer `width, height,` par
`capture, encode,`.

- [ ] **Étape 3 : Réparer les usages internes de `width`/`height`**

```bash
grep -n "self\.width\|self\.height" agent/src/encode.rs
```

Pour chaque occurrence, choisir `self.encode.0` / `self.encode.1` — toutes
concernent l'encodeur ou ses textures NV12 de sortie, jamais l'entrée BGRA. Si
une occurrence concerne une texture **d'entrée** BGRA, elle prend
`self.capture.0` / `self.capture.1` : la trancher au cas par cas, en lisant le
contexte, plutôt que par un remplacement global.

Puis remplacer la méthode `dimensions` :

```rust
    /// Taille réellement encodée. Distincte de la taille capturée depuis que
    /// la résolution s'adapte au lien.
    pub fn encode_size(&self) -> (u32, u32) {
        self.encode
    }
```

- [ ] **Étape 4 : Ajouter le débit à chaud**

Après `request_keyframe`, dans `impl H264Encoder` :

```rust
    /// Change le débit cible sans reconstruire l'encodeur.
    ///
    /// `ICodecAPI::SetValue` à chaud est déjà éprouvé sur ce pilote par
    /// `request_keyframe`, qui écrit `AVEncVideoForceKeyFrame` en cours de
    /// session sur ce même objet.
    ///
    /// Un refus du pilote est rendu à l'appelant plutôt que journalisé ici :
    /// c'est `WindowsSource` qui sait s'il doit continuer par la résolution
    /// (voir tâche 7).
    pub fn set_bitrate(&mut self, bitrate: u32) -> Result<()> {
        let codec: ICodecAPI = self.transform.cast()?;
        let rate = variant_u32(bitrate);
        unsafe { codec.SetValue(&CODECAPI_AVEncCommonMeanBitRate, &rate) }
            .context("réglage à chaud du débit d'encodage")?;
        Ok(())
    }
```

- [ ] **Étape 5 : Compiler sur la VM**

```bash
virsh list --all   # démarrer si besoin, cf. contraintes globales
set -a && source .env && set +a
scripts/build-agent.sh
```

Attendu : compilation réussie. Les erreurs attendues à ce stade sont dans
`windows_source.rs`, qui appelle encore l'ancienne signature — elles sont
réparées à la tâche 7. Si `build-agent.sh` s'arrête sur ces erreurs-là et
seulement celles-là, l'étape est concluante ; noter les lignes concernées.

- [ ] **Étape 6 : Commit**

```bash
git add agent/src/encode.rs
git commit -m "feat(reseau): taille d'encodage distincte de la capture, débit modifiable à chaud"
```

---

## Tâche 7 : Source vidéo — appliquer les décisions

**Fichiers :**
- Modifier : `agent/src/source.rs`
- Modifier : `agent/src/windows_source.rs`

**Interfaces :**
- Consomme : `H264Encoder::new(device, capture, encode, fps, bitrate)`,
  `set_bitrate`, `encode_size` (tâche 6).
- Produit : `VideoSource::set_bitrate(&mut self, u32) -> Result<()>` et
  `VideoSource::set_encode_size(&mut self, u32, u32) -> Result<()>`, toutes deux
  avec une implémentation par défaut sans effet. Consommé par la tâche 9.

- [ ] **Étape 1 : Étendre le trait**

Dans `agent/src/source.rs`, après `request_keyframe` :

```rust
    /// Change le débit d'encodage sans reconstruire quoi que ce soit.
    ///
    /// Sans effet par défaut : une source fichier n'encode rien.
    fn set_bitrate(&mut self, _bitrate: u32) -> anyhow::Result<()> {
        Ok(())
    }

    /// Change la taille RÉELLEMENT ENCODÉE, sans toucher à la fenêtre
    /// capturée.
    ///
    /// À ne pas confondre avec `resize`, qui redimensionne la vraie fenêtre
    /// Windows parce que l'utilisateur a tiré un bord. Ici la fenêtre ne
    /// bouge pas : seul le flux transporté maigrit, parce que le lien ne
    /// porte plus la pleine résolution.
    ///
    /// Sans effet par défaut.
    fn set_encode_size(&mut self, _width: u32, _height: u32) -> anyhow::Result<()> {
        Ok(())
    }
```

- [ ] **Étape 2 : Réparer l'appel au constructeur**

Dans `WindowsSource::new`, remplacer :

```rust
        let mut encoder = H264Encoder::new(capture.device(), width, height, fps, bitrate)?;
```

par (au démarrage, on encode à la pleine taille capturée) :

```rust
        let mut encoder =
            H264Encoder::new(capture.device(), (width, height), (width, height), fps, bitrate)?;
```

Faire de même dans `WindowsSource::resize` — chercher l'autre appel :

```bash
grep -n "H264Encoder::new" agent/src/windows_source.rs
```

et lui donner la même forme, en conservant la **taille d'encodage courante**
plutôt que de la réinitialiser :

```rust
        let encode = self.encoder.encode_size();
        // … dans la fabrique de reconstruction :
        H264Encoder::new(capture.device(), (width, height), encode, fps, bitrate)
```

- [ ] **Étape 3 : Implémenter les deux méthodes**

Dans le bloc `impl VideoSource for WindowsSource`, après `request_keyframe` :

```rust
    fn set_bitrate(&mut self, bitrate: u32) -> Result<()> {
        // Mémorisé même en cas d'échec : c'est ce débit-là qu'une
        // reconstruction ultérieure de l'encodeur devra reprendre.
        self.bitrate = bitrate;
        self.encoder.set_bitrate(bitrate)
    }

    fn set_encode_size(&mut self, width: u32, height: u32) -> Result<()> {
        WindowsSource::set_encode_size(self, width, height)
    }
```

Et dans le bloc `impl WindowsSource` (pas le bloc de trait), la vraie
implémentation :

```rust
    /// Reconstruit l'encodeur à une nouvelle taille de sortie, **sans toucher
    /// à la capture ni à la fenêtre**.
    ///
    /// Media Foundation n'autorise pas le changement de résolution en cours
    /// de route : il faut un encodeur neuf (même contrainte que `resize`, voir
    /// son commentaire). Mais contrairement à `resize`, la capture DXGI reste
    /// vivante — c'est l'entrée du convertisseur, elle n'a pas changé. Aucune
    /// contrainte de duplication DXGI ici, donc aucun besoin de
    /// `rebuild_or_recover`.
    ///
    /// L'horodatage n'est pas réinitialisé : `last_pts_90k` est conservé, le
    /// décodeur du navigateur rejetterait un retour en arrière.
    pub fn set_encode_size(&mut self, width: u32, height: u32) -> Result<()> {
        let (width, height) = (width.max(2) & !1, height.max(2) & !1);
        if (width, height) == self.encoder.encode_size() {
            return Ok(());
        }

        let device = self.capture_mut().device().clone();
        let mut encoder = H264Encoder::new(
            &device,
            (self.width, self.height),
            (width, height),
            self.fps,
            self.bitrate,
        )?;
        // Un encodeur neuf doit commencer par une image clé : sans elle, le
        // décodeur du navigateur n'a aucun point d'entrée dans le nouveau
        // flux et rend un écran gris jusqu'à la prochaine.
        encoder.request_keyframe()?;

        self.encoder = encoder;
        // L'encodeur neuf n'a rien produit : le budget de sondage de
        // démarrage doit repartir, comme après `resize`.
        self.encoder_warmed_up = false;
        tracing::info!(width, height, "taille d'encodage changée sans toucher à la fenêtre");
        Ok(())
    }
```

- [ ] **Étape 4 : Compiler sur la VM**

```bash
scripts/build-agent.sh
```

Attendu : compilation réussie, sans erreur.

- [ ] **Étape 5 : Vérifier que rien n'est cassé sur LAN**

```bash
sudo scripts/netem.sh lan
scripts/run-agent.sh
cd client && npm run verify
```

Attendu : la session s'établit, l'image arrive. Aucune adaptation n'est encore
câblée (tâche 9) : ce contrôle vérifie seulement que la refonte des tailles n'a
rien cassé.

- [ ] **Étape 6 : Commit**

```bash
git add agent/src/source.rs agent/src/windows_source.rs
git commit -m "feat(reseau): la source applique débit et taille d'encodage sans bouger la fenêtre"
```

---

## Tâche 8 : Opus — réveiller le FEC in-band

`set_inband_fec(true)` est déjà appelé, mais libopus ne produit de redondance
LBRR que si le pourcentage de perte déclaré est strictement positif. Il vaut 0
et rien ne l'écrit : le FEC coche la case sans rien émettre.

**Fichiers :**
- Modifier : `agent/src/opus.rs`

**Interfaces :**
- Produit : `OpusEncoder::set_packet_loss_perc(&mut self, i32) -> Result<()>`.
  Consommé par la tâche 9.

- [ ] **Étape 1 : Écrire le test qui échoue**

Dans le module `tests` de `agent/src/opus.rs` :

```rust
    #[test]
    fn le_pourcentage_de_perte_est_borne_et_relu() {
        let mut enc = OpusEncoder::new().expect("encodeur");

        enc.set_packet_loss_perc(0).expect("0 accepté");
        enc.set_packet_loss_perc(25).expect("25 accepté");

        // Hors bornes : borné plutôt que refusé. Le contrôleur borne déjà,
        // mais cette fonction est publique et ne doit pas laisser passer une
        // valeur que libopus rejetterait avec une erreur opaque.
        enc.set_packet_loss_perc(-5).expect("valeur négative bornée");
        enc.set_packet_loss_perc(300).expect("valeur excessive bornée");
    }

    #[test]
    fn une_perte_declaree_grossit_les_paquets_encodes() {
        // Preuve que le FEC n'est plus inerte : à contenu identique, déclarer
        // de la perte fait produire des paquets plus gros, la redondance LBRR
        // étant alors réellement émise.
        //
        // Un signal NON silencieux est indispensable : sous DTX, le silence
        // retombe à 1 octet par trame quoi qu'on déclare, et la mesure ne
        // montrerait rien.
        let pcm: Vec<i16> = (0..FRAME_INTERLEAVED)
            .map(|i| ((i as f32 * 0.05).sin() * 8000.0) as i16)
            .collect();

        let mut sans = OpusEncoder::new().expect("encodeur");
        let mut avec = OpusEncoder::new().expect("encodeur");
        avec.set_packet_loss_perc(20).expect("perte déclarée");

        // On mesure en régime établi : les premières trames ne sont pas
        // représentatives (l'encodeur converge sur plusieurs dizaines de
        // trames, constaté au chantier A pour le DTX).
        let mut total_sans = 0usize;
        let mut total_avec = 0usize;
        for _ in 0..100 {
            total_sans += sans.encode(&pcm).expect("encodage").len();
            total_avec += avec.encode(&pcm).expect("encodage").len();
        }

        assert!(
            total_avec > total_sans,
            "le FEC ne produit rien : {total_avec} octets avec perte déclarée \
             contre {total_sans} sans — set_inband_fec seul est inerte"
        );
    }
```

- [ ] **Étape 2 : Lancer les tests pour vérifier qu'ils échouent**

```bash
cargo test -p agent opus
```

Attendu : ÉCHEC de compilation — `no method named 'set_packet_loss_perc'`.

- [ ] **Étape 3 : Écrire l'implémentation**

Dans `impl OpusEncoder`, après `new` :

```rust
    /// Déclare à l'encodeur le taux de perte observé sur le lien, en pour
    /// cent.
    ///
    /// **C'est ce réglage qui rend le FEC in-band opérant.** `set_inband_fec`
    /// seul ne fait qu'autoriser la redondance LBRR ; libopus ne l'émet que si
    /// une perte non nulle est déclarée. Sans cet appel, le FEC activé à la
    /// construction ne produit rien.
    ///
    /// La valeur est bornée à [0, 100] : libopus refuse le reste avec une
    /// erreur opaque, et l'appelant n'a pas à connaître cette borne.
    pub fn set_packet_loss_perc(&mut self, perc: i32) -> Result<()> {
        self.inner
            .set_packet_loss_perc(perc.clamp(0, 100))
            .context("réglage du taux de perte déclaré à Opus")
    }
```

- [ ] **Étape 4 : Lancer les tests pour vérifier qu'ils passent**

```bash
cargo test -p agent opus
```

Attendu : SUCCÈS. Si `une_perte_declaree_grossit_les_paquets_encodes` échoue
avec des totaux égaux, ne PAS assouplir l'assertion : c'est le symptôme que le
FEC reste inerte, et c'est exactement ce que la tâche doit corriger. Vérifier
alors que `set_inband_fec(true)` est bien appelé AVANT la première trame.

- [ ] **Étape 5 : Commit**

```bash
git add agent/src/opus.rs
git commit -m "fix(audio): le FEC in-band ne produisait rien faute de perte déclarée"
```

---

## Tâche 9 : Transport — activer le BWE, récolter, appliquer

**Fichiers :**
- Modifier : `agent/src/transport.rs`
- Modifier : `agent/src/main.rs`

**Interfaces :**
- Consomme : `congestion::{Controleur, Config, Observation, Decision}` (tâche 5),
  `VideoSource::{set_bitrate, set_encode_size}` (tâche 7),
  `OpusEncoder::set_packet_loss_perc` (tâche 8).
- Produit : une décision appliquée, et `Session::decision_courante() ->
  congestion::Decision`. Consommé par la tâche 10.

- [ ] **Étape 1 : Activer le BWE sur le constructeur `Rtc`**

Dans `Session::new` (`transport.rs:467`) :

```rust
        let mut rtc = Rtc::builder()
            .clear_codecs()
            .enable_h264(true)
            .enable_opus(true)
            // Sans cet appel, `Event::EgressBitrateEstimate` n'est JAMAIS
            // émis et tout l'asservissement reste muet. L'estimation
            // initiale est volontairement modeste : le sous-système sonde à
            // la hausse vers `set_desired_bitrate` (posé plus bas), et
            // partir trop haut ferait saturer le lien avant la première
            // correction.
            .enable_bwe(Some(Bitrate::bps(ESTIMATION_INITIALE_BPS as u64)))
            .set_stats_interval(Some(Duration::from_secs(1)))
            .build(Instant::now());

        // Cible que le sondage cherche à atteindre : le plafond configuré.
        rtc.bwe().set_desired_bitrate(Bitrate::bps(plafond_bps as u64));
```

Ajouter en tête de fichier :

```rust
use str0m::bwe::Bitrate;
```

et près des autres constantes :

```rust
/// Estimation de bande passante de départ, avant toute rétroaction du pair.
///
/// Compromis mesuré à la tâche 12 : trop bas, le démarrage sur LAN met du
/// temps à rejoindre le plafond et la recette perd des images par seconde ;
/// trop haut, le premier instant d'une session sur lien étroit sature avant
/// la première correction. 2,5 Mb/s est le point de départ, à confirmer.
const ESTIMATION_INITIALE_BPS: u32 = 2_500_000;
```

- [ ] **Étape 2 : Porter le plafond jusqu'au constructeur**

`Session::new` prend un paramètre de plus. Modifier sa signature :

```rust
    pub fn new(
        source: Box<dyn VideoSource + Send>,
        local_ip: IpAddr,
        clock_origin: Instant,
        plafond_bps: u32,
    ) -> Result<Self> {
```

et dans `agent/src/main.rs`, à l'appel (`main.rs:943`) :

```rust
    let mut session = Session::new(source, config.local_ip, clock_origin, bitrate)?;
```

Si `bitrate` n'est pas dans la portée de cet appel, le remonter : il est lu à
`main.rs:900` dans la branche qui construit `WindowsSource`. Le sortir de cette
branche vers une liaison de portée supérieure, avec la même valeur par défaut,
plutôt que de le relire deux fois depuis l'environnement.

- [ ] **Étape 3 : Ajouter le contrôleur à la session**

Dans la déclaration de `struct Session`, à côté de `pending_resize` :

```rust
    /// Contrôleur de congestion. Alimenté par `Event::EgressBitrateEstimate`
    /// et `Event::MediaEgressStats`, tous deux déjà émis par str0m — le
    /// second l'était même déjà avant ce chantier, et tombait dans le `_ =>
    /// {}` de `handle_event`.
    congestion: congestion::Controleur,
    /// Dernière estimation reçue, en attente d'être confrontée aux
    /// statistiques. Les deux événements n'arrivent pas ensemble.
    derniere_estimation_bps: Option<u32>,
    /// Décision décidée mais pas encore appliquée. Appliquée dans
    /// `act_on_timeout`, jamais depuis `handle_event` — reconstruire
    /// l'encodeur pendant le drainage de `poll_output` romprait l'invariant
    /// de str0m (une seule mutation par appel), exactement comme pour
    /// `pending_resize`.
    pending_decision: Option<congestion::Decision>,
    /// Vrai une fois que l'indisponibilité de l'adaptation a été journalisée.
    /// Une condition permanente ne se journalise pas chaque seconde.
    absence_bwe_signalee: bool,
```

Dans le littéral de construction, à côté de `pending_resize: None,` :

```rust
            congestion: congestion::Controleur::new(
                congestion::Config {
                    plafond_bps,
                    audio_bps: 128_000,
                    source: dimensions,
                    fps: 60,
                },
                Instant::now(),
            ),
            derniere_estimation_bps: None,
            pending_decision: None,
            absence_bwe_signalee: false,
```

et ajouter en tête de fichier :

```rust
use crate::congestion;
```

- [ ] **Étape 4 : Récolter les deux événements**

Dans `handle_event`, **avant** le bras `_ => {}` :

```rust
            Event::EgressBitrateEstimate(kind) => {
                // Les deux variantes portent une estimation ; seule REMB
                // nomme en plus le `mid` concerné, dont on n'a pas l'usage
                // avec une piste vidéo unique.
                let bps = match kind {
                    str0m::bwe::BweKind::Twcc(b) => b.as_u64(),
                    str0m::bwe::BweKind::Remb(_, b) => b.as_u64(),
                };
                self.derniere_estimation_bps = Some(bps as u32);
            }
            Event::MediaEgressStats(stats) => {
                // Seule la piste vidéo alimente la décision : l'audio a un
                // débit fixe et son budget est déjà retiré par le contrôleur.
                if Some(stats.mid) != self.video_mid {
                    return Tick::Continue;
                }
                let observation = congestion::Observation {
                    estimate_bps: self.derniere_estimation_bps,
                    rtt: stats.rtt,
                    loss: stats.loss,
                    at: Instant::now(),
                };
                if observation.estimate_bps.is_none() && !self.absence_bwe_signalee {
                    self.absence_bwe_signalee = true;
                    tracing::warn!(
                        "aucune estimation de bande passante reçue : l'adaptation reste \
                         indisponible et le débit demeure au plafond configuré"
                    );
                }
                tracing::debug!(
                    estimation = ?observation.estimate_bps,
                    rtt = ?observation.rtt,
                    perte = ?observation.loss,
                    "observation réseau"
                );
                if let Some(decision) = self.congestion.observer(observation) {
                    // Mémorisée, pas appliquée : voir le commentaire du champ.
                    self.pending_decision = Some(decision);
                }
            }
```

> **Champ de perte : vérifié.** `MediaEgressStats.loss` est un
> `Option<f32>` déjà exprimé en fraction (`str0m-0.21.0/src/stats.rs:163`,
> « Fraction of packets lost averaged from the RTCP receiver reports
> received »). Il se passe donc tel quel à `Observation.loss`, sans
> conversion — ce n'est pas le format brut en huitièmes de pour cent du
> Receiver Report, str0m l'a déjà normalisé.

- [ ] **Étape 5 : Appliquer la décision dans `act_on_timeout`**

Insérer une branche **juste avant** la branche `a1` du redimensionnement, avec
le même raisonnement que celle-ci (opération potentiellement longue, une seule
action par tour) :

```rust
        // a0ter) Décision d'adaptation en attente. Traitée avant la branche
        // vidéo et avant le redimensionnement : reconfigurer l'encodeur avec
        // une image en vol coûterait cette image.
        //
        // Ne mute jamais `Rtc` — seuls la source vidéo et l'encodeur audio
        // sont touchés — donc cette branche respecte l'invariant de drainage.
        if let Some(decision) = self.pending_decision.take() {
            if let Err(e) = self.source.set_bitrate(decision.video_bitrate_bps) {
                // L'encodeur refuse le débit à chaud : on garde le débit
                // courant et on continue d'adapter par la résolution. Une
                // seule ligne, pas une par seconde.
                if !self.refus_debit_signale {
                    self.refus_debit_signale = true;
                    tracing::warn!(erreur = %e, "l'encodeur refuse le réglage du débit à chaud");
                }
            }
            if decision.encode_size != self.encode_size_appliquee {
                match self.source.set_encode_size(decision.encode_size.0, decision.encode_size.1) {
                    Ok(()) => self.encode_size_appliquee = decision.encode_size,
                    Err(e) => {
                        // On reste au barreau courant. La session vit.
                        tracing::warn!(
                            erreur = %e,
                            largeur = decision.encode_size.0,
                            hauteur = decision.encode_size.1,
                            "changement de taille d'encodage refusé, barreau conservé"
                        );
                    }
                }
            }
            if let Some(audio) = self.audio_source.as_mut() {
                if let Err(e) = audio.set_packet_loss_perc(decision.opus_loss_perc) {
                    tracing::warn!(erreur = %e, "réglage du taux de perte Opus refusé");
                }
            }
            return Ok(Tick::Continue);
        }
```

Ajouter les deux champs manquants à `struct Session`, à côté des précédents :

```rust
    /// Taille d'encodage réellement appliquée. Distincte de celle décidée :
    /// un refus de l'encodeur laisse la décision non appliquée, et il ne faut
    /// pas la retenter à chaque tour.
    encode_size_appliquee: (u32, u32),
    /// Vrai une fois le refus du débit à chaud journalisé.
    refus_debit_signale: bool,
```

initialisés par `encode_size_appliquee: dimensions,` et
`refus_debit_signale: false,`.

> **`AudioSource` n'expose pas encore `set_packet_loss_perc`.** Ajouter au
> trait, dans le même fichier que sa définition (`agent/src/source.rs` ou
> `agent/src/audio.rs` — le localiser par `grep -rn "trait AudioSource"
> agent/src/`), une méthode par défaut sans effet :
> ```rust
>     /// Déclare le taux de perte observé, pour que le FEC in-band Opus
>     /// produise réellement de la redondance. Sans effet par défaut.
>     fn set_packet_loss_perc(&mut self, _perc: i32) -> anyhow::Result<()> {
>         Ok(())
>     }
> ```
> puis l'implémenter dans la source Windows en relayant vers
> `OpusEncoder::set_packet_loss_perc`.

- [ ] **Étape 6 : Exposer la décision courante**

Dans `impl Session`, après `queue_control` :

```rust
    /// Décision d'adaptation actuellement retenue. Alimente le message d'état
    /// du lien envoyé au navigateur (tâche 10).
    pub fn decision_courante(&self) -> congestion::Decision {
        self.congestion.courant()
    }
```

- [ ] **Étape 7 : Compiler et lancer les tests**

```bash
cargo test -p agent          # sur l'hôte : les modules purs
scripts/build-agent.sh       # sur la VM : le code Windows
```

Attendu : succès des deux côtés.

- [ ] **Étape 8 : Vérifier que l'adaptation réagit réellement**

```bash
scripts/run-agent.sh
sudo scripts/netem.sh congestionné
```

Connecter le client, puis relever dans le journal de l'agent :

```bash
grep -E "observation réseau|taille d'encodage changée" /chemin/vers/agent.log | head -20
```

Attendu : des lignes `observation réseau` avec une estimation **non nulle et
inférieure à 3 Mb/s**, puis au moins une ligne `taille d'encodage changée` dans
les 10 secondes. Si l'estimation reste `None`, TWCC n'est pas négocié — se
reporter au constat de la tâche 2, c'est le cas qu'elle devait lever.

```bash
sudo scripts/netem.sh off
```

- [ ] **Étape 9 : Commit**

```bash
git add agent/src/transport.rs agent/src/main.rs agent/src/source.rs agent/src/audio.rs
git commit -m "feat(reseau): asservir débit, résolution et FEC à ce que le pair rapporte"
```

---

## Tâche 10 : Protocole — `AgentControl::Link`, version 3

**Fichiers :**
- Modifier : `proto/src/control.rs`
- Modifier : `proto/ts/control.ts`
- Modifier : `proto/vectors.json`
- Modifier : `agent/src/transport.rs`

**Interfaces :**
- Consomme : `Session::decision_courante()` (tâche 9).
- Produit : `AgentControl::link(bitrate, encode_size, qualite, adaptation)` côté
  Rust ; `LinkMessage` côté TypeScript. Consommé par la tâche 11.

- [ ] **Étape 1 : Écrire les tests qui échouent**

Dans le module `tests` de `proto/src/control.rs` :

```rust
    #[test]
    fn serialise_l_etat_du_lien() {
        let json = serde_json::to_string(&AgentControl::link(
            4_000_000,
            (1280, 720),
            LinkQuality::Degradee,
            LinkAdaptation::Active,
        ))
        .expect("sérialisation");
        assert_eq!(
            json,
            r#"{"type":"link","v":3,"bitrate":4000000,"width":1280,"height":720,"quality":"degradee","adaptation":"active"}"#
        );
    }

    #[test]
    fn deserialise_l_etat_du_lien() {
        let msg: AgentControl = serde_json::from_str(
            r#"{"type":"link","v":3,"bitrate":1500000,"width":960,"height":540,"quality":"insuffisante","adaptation":"indisponible"}"#,
        )
        .expect("désérialisation");
        assert_eq!(
            msg,
            AgentControl::link(
                1_500_000,
                (960, 540),
                LinkQuality::Insuffisante,
                LinkAdaptation::Indisponible
            )
        );
    }
```

- [ ] **Étape 2 : Lancer les tests pour vérifier qu'ils échouent**

```bash
cargo test -p proto
```

Attendu : ÉCHEC de compilation — `cannot find function 'link'`. Les tests
existants passent encore, avec `"v":2`.

- [ ] **Étape 3 : Passer la version à 3 et ajouter le variant**

Dans `proto/src/control.rs` :

```rust
/// v3 (chantier C) : ajout de `Link`.
pub const CONTROL_VERSION: u8 = 3;
```

Ajouter les deux énumérations, au-dessus de `AgentControl` :

```rust
/// Ce que l'utilisateur doit comprendre de l'état du lien.
///
/// Trois valeurs et non un booléen : « dégradé » et « insuffisant » sont deux
/// situations distinctes, et la seconde ne se déduit pas de la première par
/// une négation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LinkQuality {
    /// Pleine résolution, lien confortable.
    Bonne,
    /// Résolution réduite pour tenir le lien.
    Degradee,
    /// Plancher atteint : le lien ne permet plus le jeu nerveux. C'est
    /// l'avertissement explicite exigé par le cadrage jeu (§3).
    Insuffisante,
}

/// L'agent reçoit-il de quoi s'asservir ?
///
/// Indépendant de `LinkQuality` : une session sans estimation de bande
/// passante peut très bien tourner en `Bonne` sur un lien large.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LinkAdaptation {
    Active,
    /// Aucune estimation ne parvient à l'agent : le débit reste figé au
    /// plafond configuré. À dire, pas à taire.
    Indisponible,
}
```

Dans l'énumération `AgentControl`, après `Capabilities` :

```rust
    /// État du lien réseau, émis à chaque changement de décision
    /// d'adaptation — donc rarement, pas à chaque seconde.
    Link {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        bitrate: u32,
        width: u32,
        height: u32,
        quality: LinkQuality,
        adaptation: LinkAdaptation,
    },
```

et dans `impl AgentControl` :

```rust
    pub fn link(
        bitrate: u32,
        taille: (u32, u32),
        quality: LinkQuality,
        adaptation: LinkAdaptation,
    ) -> Self {
        AgentControl::Link {
            version: CONTROL_VERSION,
            bitrate,
            width: taille.0,
            height: taille.1,
            quality,
            adaptation,
        }
    }
```

- [ ] **Étape 4 : Réparer les tests existants**

Tous les tests qui codent `"v":2` en dur échouent maintenant. Les mettre à `3` :

```bash
grep -n '"v":2' proto/src/control.rs
```

Remplacer chaque occurrence par `"v":3`. Ne PAS les rendre insensibles à la
version : c'est précisément le rôle de ces tests de figer le format émis.

- [ ] **Étape 5 : Lancer les tests pour vérifier qu'ils passent**

```bash
cargo test -p proto
```

Attendu : SUCCÈS, tous les tests.

- [ ] **Étape 6 : Mettre le miroir TypeScript à jour**

Dans `proto/ts/control.ts` :

```typescript
export const CONTROL_VERSION = 3;

export type LinkQuality = 'bonne' | 'degradee' | 'insuffisante';
export type LinkAdaptation = 'active' | 'indisponible';

export interface LinkMessage {
    v: number;
    type: 'link';
    bitrate: number;
    width: number;
    height: number;
    quality: LinkQuality;
    adaptation: LinkAdaptation;
}
```

et ajouter `LinkMessage` à l'union `AgentControl` du même fichier.

- [ ] **Étape 7 : Mettre les vecteurs partagés à jour**

```bash
grep -n '"v": *2\|"v":2' proto/vectors.json | head
```

Porter chaque `v` à 3, et ajouter un vecteur pour le nouveau message, sur le
modèle exact des vecteurs existants du fichier (mêmes clés, même mise en forme).

```bash
cargo test -p proto && cd client && npm test && cd ..
```

Attendu : SUCCÈS des deux côtés. Un échec côté TypeScript sur un vecteur signale
une divergence entre les deux miroirs — la corriger, ne pas désactiver le test.

- [ ] **Étape 8 : Émettre le message depuis le transport**

Dans `transport.rs`, à la fin de la branche `a0ter` (tâche 9, étape 5), juste
avant le `return Ok(Tick::Continue);` :

```rust
            self.queue_control(AgentControl::link(
                decision.video_bitrate_bps,
                decision.encode_size,
                match decision.qualite {
                    congestion::Qualite::Bonne => proto::control::LinkQuality::Bonne,
                    congestion::Qualite::Degradee => proto::control::LinkQuality::Degradee,
                    congestion::Qualite::Insuffisante => proto::control::LinkQuality::Insuffisante,
                },
                match decision.adaptation {
                    congestion::Adaptation::Active => proto::control::LinkAdaptation::Active,
                    congestion::Adaptation::Indisponible => {
                        proto::control::LinkAdaptation::Indisponible
                    }
                },
            ));
```

Étendre aussi le `match` de journalisation `type_message` (`transport.rs`,
branche `a`) avec le nouveau variant, sans quoi la compilation échoue :

```rust
                    AgentControl::Link { .. } => "link",
```

- [ ] **Étape 9 : Compiler et committer**

```bash
cargo test -p agent && cargo test -p proto
scripts/build-agent.sh
git add proto/src/control.rs proto/ts/control.ts proto/vectors.json agent/src/transport.rs
git commit -m "feat(reseau): message d'état du lien, protocole de contrôle en version 3"
```

---

## Tâche 11 : Client — latence de restitution et indicateur

**Fichiers :**
- Modifier : `client/src/webrtc.ts`
- Créer : `client/src/lien.ts`
- Créer : `client/src/lien.test.ts`
- Modifier : `client/src/main.ts`

**Interfaces :**
- Consomme : `LinkMessage` (tâche 10).
- Produit : `texteLien(message: LinkMessage): TexteLien`.

- [ ] **Étape 1 : Écrire le test qui échoue**

Créer `client/src/lien.test.ts` :

```typescript
import { describe, expect, it } from 'vitest';
import { texteLien } from './lien';
import type { LinkMessage } from '../../proto/ts/control';

function lien(partiel: Partial<LinkMessage>): LinkMessage {
    return {
        v: 3,
        type: 'link',
        bitrate: 8_000_000,
        width: 1920,
        height: 1080,
        quality: 'bonne',
        adaptation: 'active',
        ...partiel,
    };
}

describe('texteLien', () => {
    it('ne signale rien de particulier quand tout va bien', () => {
        const t = texteLien(lien({}));
        expect(t.alerte).toBe(false);
        expect(t.resume).toContain('8.0 Mb/s');
    });

    it('dit pourquoi l’image a molli quand la résolution est réduite', () => {
        const t = texteLien(lien({ quality: 'degradee', width: 1280, height: 720 }));
        expect(t.alerte).toBe(true);
        expect(t.resume).toContain('1280×720');
        // Le texte doit nommer la CAUSE, pas seulement l'effet : un
        // utilisateur qui lit « 1280×720 » sans explication croit à un bug.
        expect(t.resume.toLowerCase()).toContain('réseau');
    });

    it('avertit explicitement quand le lien ne permet plus le jeu nerveux', () => {
        const t = texteLien(lien({ quality: 'insuffisante' }));
        expect(t.alerte).toBe(true);
        expect(t.resume.toLowerCase()).toContain('insuffisant');
    });

    it('distingue une adaptation indisponible d’un lien dégradé', () => {
        const t = texteLien(lien({ adaptation: 'indisponible' }));
        // Pas une alerte : le lien peut très bien être excellent.
        expect(t.alerte).toBe(false);
        expect(t.resume.toLowerCase()).toContain('adaptation indisponible');
    });
});
```

- [ ] **Étape 2 : Lancer le test pour vérifier qu'il échoue**

```bash
cd client && npm test -- lien
```

Attendu : ÉCHEC — `Failed to resolve import "./lien"`.

- [ ] **Étape 3 : Écrire l'implémentation**

Créer `client/src/lien.ts` :

```typescript
// Traduction de l'état du lien annoncé par l'agent en texte affichable.
//
// Séparé de tout DOM pour être testable, comme `status.ts` et `audio.ts`.
// L'agent décide ; ce module ne fait que dire, en français, ce qu'il a décidé.

import type { LinkMessage } from '../../proto/ts/control';

export interface TexteLien {
    resume: string;
    /// Vrai quand l'utilisateur doit être averti : image dégradée ou lien
    /// insuffisant. Une adaptation indisponible n'est PAS une alerte — le
    /// lien peut être excellent, seul l'asservissement manque.
    alerte: boolean;
}

export function texteLien(message: LinkMessage): TexteLien {
    const mbps = (message.bitrate / 1_000_000).toFixed(1);
    const taille = `${message.width}×${message.height}`;

    if (message.quality === 'insuffisante') {
        return {
            resume: `Réseau insuffisant pour le jeu nerveux — ${taille}, ${mbps} Mb/s`,
            alerte: true,
        };
    }
    if (message.quality === 'degradee') {
        return {
            resume: `Image réduite par le réseau — ${taille}, ${mbps} Mb/s`,
            alerte: true,
        };
    }
    if (message.adaptation === 'indisponible') {
        return {
            resume: `${taille}, ${mbps} Mb/s — adaptation indisponible`,
            alerte: false,
        };
    }
    return { resume: `${taille}, ${mbps} Mb/s`, alerte: false };
}
```

- [ ] **Étape 4 : Lancer le test pour vérifier qu'il passe**

```bash
cd client && npm test -- lien
```

Attendu : SUCCÈS, 4 tests.

- [ ] **Étape 5 : Poser `playoutDelayHint` sur le receiver vidéo**

Dans `client/src/webrtc.ts`, dans l'écouteur `track` déjà présent :

```typescript
    pc.addEventListener('track', (event) => {
        flux.addTrack(event.track);
        // Latence de restitution : demander au navigateur de ne pas
        // constituer de tampon de gigue au-delà du strict nécessaire.
        //
        // Ce n'est pas gratuit — sur un lien qui gigue, ce tampon est ce qui
        // lisse la restitution, et le raboter échange de la latence contre du
        // saccadement. Mesuré sous chaque profil netem à la recette.
        //
        // Chromium seulement : ailleurs la propriété n'existe pas et
        // l'affectation est sans effet. D'où l'accès défensif plutôt qu'un
        // `receiver.playoutDelayHint = 0` direct, qui lèverait en mode strict
        // sur un objet scellé.
        if (event.track.kind === 'video' && 'playoutDelayHint' in event.receiver) {
            (event.receiver as RTCRtpReceiver & { playoutDelayHint: number }).playoutDelayHint = 0;
        }
        if (options.video.srcObject !== flux) {
            options.video.srcObject = flux;
        }
        status(`flux reçu (${flux.getTracks().length} piste(s))`);
    });
```

- [ ] **Étape 6 : Câbler l'indicateur**

Dans `client/src/main.ts`, localiser le traitement des messages de contrôle :

```bash
grep -n "onControl" client/src/main.ts
```

Ajouter une branche pour le nouveau type, dans le même style que les branches
existantes :

```typescript
            if (message.type === 'link') {
                const t = texteLien(message);
                // Le bandeau de statut protège déjà les messages terminaux :
                // un avertissement réseau n'écrasera pas une fin de session.
                statut.afficher(t.resume);
            }
```

et l'import correspondant en tête de fichier :

```typescript
import { texteLien } from './lien';
```

- [ ] **Étape 7 : Vérifier types et tests**

```bash
cd client && npm run typecheck && npm test
```

Attendu : SUCCÈS des deux.

- [ ] **Étape 8 : Commit**

```bash
git add client/src/lien.ts client/src/lien.test.ts client/src/webrtc.ts client/src/main.ts
git commit -m "feat(client): indicateur d'état du lien et latence de restitution réduite"
```

---

## Tâche 12 : Recette par profils et non-régression LAN

**Fichiers :**
- Modifier : `docs/superpowers/plans/2026-07-29-reseau-adaptatif-resultats.md`
- Éventuellement : `agent/src/congestion.rs` (ajustement de `BPP_MIN`)

**Interfaces :**
- Consomme : tout ce qui précède.
- Produit : le document de résultats. Aucune interface de code.

- [ ] **Étape 1 : Non-régression LAN, en premier**

Elle prime sur le reste : la mesurer avant les profils dégradés, pour ne pas
découvrir une régression après trois heures de réglage.

```bash
virsh list --all
sudo scripts/netem.sh lan
scripts/run-agent.sh
cd client && npm run verify
```

Puis la mesure complète, avec le même harnais que la recette du chantier 0 :

```bash
node client/recette/harness.mjs
```

Relever débit et latence. **Seuils : ≥ 55 i/s et médiane < 50 ms.** En dessous,
ne pas continuer : chercher d'abord si `ESTIMATION_INITIALE_BPS` est trop basse
(le débit met alors du temps à rejoindre le plafond) en relevant combien de
secondes s'écoulent avant que le journal n'annonce un débit proche de 12 Mb/s.
Ajuster cette constante, remesurer, et **consigner l'ajustement** dans le
document de résultats.

- [ ] **Étape 2 : Un passage par profil**

Pour chaque profil parmi `adsl`, `4g`, `congestionné`, `effondrement` :

```bash
sudo scripts/netem.sh <profil>
scripts/run-agent.sh          # agent neuf à chaque profil : le journal est
                              # écrasé à chaque démarrage (Tee-Object), et une
                              # session dégradée d'un essai précédent
                              # fausserait le suivant
cd client && npm run verify
```

Relever, pour chacun, dans le journal de l'agent et dans l'overlay du client :

| Grandeur | Où |
| --- | --- |
| Estimation BWE atteinte | journal, `observation réseau` |
| Débit d'encodage retenu | journal, `observation réseau` |
| Barreau atteint et instants des changements | journal, `taille d'encodage changée` |
| Nombre de changements de barreau en 60 s | journal, comptage |
| Latence médiane et débit d'images | overlay client / harnais |
| Texte de l'indicateur | overlay client |

- [ ] **Étape 3 : Vérifier les quatre propriétés attendues**

1. **Le débit suit.** L'estimation retenue doit rester sous le débit posé par
   netem, marge comprise. Un débit d'encodage supérieur au débit du lien est un
   échec de la marge du §5.5 de la spec.
2. **La résolution descend et remonte sans battre.** Au plus **deux** changements
   de barreau en 60 s à profil constant. Au-delà, l'hystérésis est trop courte :
   allonger `DELAI_REMONTEE`, remesurer, consigner.
3. **L'indicateur dit vrai.** Sous `congestionné`, il doit annoncer « Image
   réduite par le réseau » ; sous `effondrement`, « Réseau insuffisant ». Un
   indicateur qui reste au vert sous `effondrement` est le pire défaut possible
   de ce chantier — il fait passer un lien saturé pour un bug de l'application.
4. **Le FEC opère.** Sous `4g` (1 % de perte), l'audio ne doit pas produire de
   coupure audible, et le débit audio relevé par l'overlay doit être
   sensiblement supérieur aux 128 kb/s nominaux — c'est la signature de la
   redondance LBRR réellement émise.

- [ ] **Étape 4 : Ajuster `BPP_MIN` si l'échelle tombe mal**

`BPP_MIN` est le réglage annoncé comme non mesuré (tâche 3). Si sous `adsl`
(8 Mb/s) le contrôleur descend d'un barreau alors que l'image en pleine
résolution était acceptable, la valeur est trop haute ; s'il reste en pleine
résolution alors que l'image est visiblement dégradée, elle est trop basse.
Ajuster, relancer les tests unitaires (les valeurs attendues du test
`le_barreau_finance_est_le_plus_haut_que_le_debit_paie` sont exprimées
relativement à `min_bps`, donc elles suivent), remesurer, et **consigner la
valeur retenue avec ce qui l'a justifiée**.

- [ ] **Étape 5 : Passe sur lien réel**

Une session depuis l'extérieur (ChromeOS via Pomerium, ou partage de connexion
mobile). Relever les mêmes grandeurs. Cette passe **confirme**, elle ne sert pas
de base de réglage : elle n'est pas reproductible, et un chiffre isolé ne doit
pas faire modifier une constante réglée sur le banc.

- [ ] **Étape 6 : Écrire les résultats**

Compléter `docs/superpowers/plans/2026-07-29-reseau-adaptatif-resultats.md` :

- §2 tableau par profil, avec les six grandeurs de l'étape 2 ;
- §3 verdict sur chacune des quatre propriétés de l'étape 3, sans arrondir vers
  le haut : une propriété à moitié tenue s'écrit « partielle », pas « tenue » ;
- §4 non-régression LAN, chiffres avant et après ;
- §5 réglages ajustés (`BPP_MIN`, `ESTIMATION_INITIALE_BPS`, délais
  d'hystérésis) et ce qui a justifié chaque ajustement ;
- §6 ce que la mesure a coûté : outils jetables créés, faux départs, mesures
  refaites — comme les recettes des chantiers A et B.

- [ ] **Étape 7 : Retirer le banc et committer**

```bash
sudo scripts/netem.sh off
git add docs/superpowers/plans/2026-07-29-reseau-adaptatif-resultats.md agent/src/congestion.rs
git commit -m "docs(reseau): résultats de la recette d'adaptation, par profil"
```

---

## Fin du volet 1

À ce stade, le pipeline s'asservit à ce que le lien porte et le dit à
l'utilisateur — mais il ne joint toujours le pair que sur le réseau local. C'est
l'objet du volet 2, `docs/superpowers/plans/2026-07-29-traversee-nat.md`.
