# Chantier C, volet 2 — Traversée NAT : plan d'implémentation

> **Pour les agents :** SOUS-COMPÉTENCE REQUISE : utiliser
> superpowers:subagent-driven-development (recommandé) ou
> superpowers:executing-plans pour dérouler ce plan tâche par tâche. Les étapes
> emploient des cases à cocher (`- [ ]`).

**Objectif :** joindre le pair depuis n'importe quel réseau, y compris derrière
un NAT symétrique des deux côtés, en faisant de l'agent un client TURN à part
entière.

**Architecture :** un module pur (`agent/src/turn.rs`) porte la machine à états
TURN et l'encapsulation ChannelData, sans jamais toucher un socket ; le
transport route chaque paquet vers le socket direct ou vers le relais selon la
source que str0m lui indique ; le serveur de signaling délivre aux deux pairs des
identifiants TURN éphémères.

**Pile technique :** Rust (str0m 0.21, `is 0.10` pour la lecture des messages
STUN, `hmac`/`sha1`/`md-5` pour l'authentification longue durée), coturn,
TypeScript côté signaling et client.

**Spec :** `docs/superpowers/specs/2026-07-29-reseau-adaptatif-design.md` §6

**Volet 1 (adaptation) :** plan séparé,
`docs/superpowers/plans/2026-07-29-reseau-adaptatif.md`. Les deux volets sont
indépendants ; celui-ci n'en dépend pas et peut être mené en parallèle sur une
branche distincte.

## Contraintes globales

- **Branche** : `chantier-traversee-nat`, créée depuis `main`. Un commit par
  étape « Commit » du plan.
- **TURN sur UDP seulement.** Les réseaux qui bloquent tout UDP sortant ne sont
  pas couverts, et c'est une limite assumée de la spec (§11), pas un oubli. Ne
  pas ajouter TURN/TCP en cours de route.
- **Pas de trickle ICE.** Le client envoie une offre unique après collecte
  complète (`webrtc.ts:213`) et l'agent répond une fois. L'allocation TURN doit
  donc **précéder** la réponse à l'offre.
- **Toute panne dégrade en le disant, aucune ne tue la session.** Une allocation
  qui échoue donne une session sans relais, pas une absence de session.
- **`turn.rs` ne touche ni socket, ni Windows, ni `Rtc`.** Il consomme des
  octets et rend des octets. C'est ce qui rend la machine à états testable sur
  Linux, sans coturn — la tâche 5 s'appuie entièrement sur cette propriété.
- **Le secret partagé de coturn ne quitte jamais le serveur de signaling.** Ni le
  navigateur ni l'agent ne le voient : seulement un mot de passe dérivé et daté.
- **Ne jamais committer d'identifiant réel.** Le secret coturn va dans `.env`
  (déjà ignoré par git) et dans `docker-compose.yml` par référence
  d'environnement, jamais en clair.
- **Français** dans les commentaires, la documentation et les messages de
  journal.

---

## Structure des fichiers

| Fichier | Responsabilité | Tâches |
| --- | --- | --- |
| `docker-compose.yml` | **Modifier.** Service coturn en `use-auth-secret` | 1 |
| `signaling/src/server.ts` | **Modifier.** Délivrer la configuration ICE à la déclaration de rôle | 2 |
| `signaling/src/ice.ts` | **Créer.** Dérivation des identifiants éphémères, testable sans WebSocket | 2 |
| `signaling/src/ice.test.ts` | **Créer.** Tests de la dérivation | 2 |
| `agent/Cargo.toml` | **Modifier.** Dépendances `is`, `hmac`, `sha1`, `md-5` | 3 |
| `agent/src/turn.rs` | **Créer.** Sérialisation des requêtes, machine à états, ChannelData | 3, 4, 5 |
| `agent/src/main.rs` | **Modifier.** Déclarer `mod turn` ; recevoir la configuration ICE | 3, 7 |
| `agent/src/signaling.rs` | **Modifier.** Remonter la configuration ICE reçue | 7 |
| `agent/src/transport.rs` | **Modifier.** Routage direct/relayé, candidat relayé, séquence de démarrage | 6, 7 |
| `client/src/webrtc.ts` | **Modifier.** Consommer les `iceServers` du signaling | 8 |
| `docs/superpowers/plans/2026-07-29-traversee-nat-resultats.md` | **Créer.** Recette | 9 |

---

## Tâche 1 : Déployer coturn

**Fichiers :**
- Modifier : `docker-compose.yml`
- Modifier : `.env` (non committé)

**Interfaces :**
- Produit : un serveur TURN joignable, en mode `use-auth-secret`, avec un secret
  partagé lu depuis l'environnement. Consommé par les tâches 2 et 9.

- [ ] **Étape 1 : Ajouter le service**

Dans `docker-compose.yml`, ajouter un service. Le mode **réseau hôte** est
indispensable : coturn alloue des ports de relais dans une plage étendue, et les
publier un par un à travers le NAT de Docker ne fonctionne pas.

```yaml
  coturn:
    image: coturn/coturn:4.6
    network_mode: host
    restart: unless-stopped
    command:
      - --listening-port=3478
      - --fingerprint
      - --use-auth-secret
      - --static-auth-secret=${TURN_SECRET}
      - --realm=${TURN_REALM}
      - --min-port=49160
      - --max-port=49200
      # Sans cette option, coturn refuse de démarrer s'il ne parvient pas à
      # déterminer son adresse externe. En déploiement local elle vaut
      # l'adresse de l'hôte ; en production, l'adresse publique.
      - --external-ip=${TURN_EXTERNAL_IP}
      - --no-tls
      - --no-dtls
      - --no-cli
      # Journal en clair sur la sortie standard : sans cela coturn écrit dans
      # un fichier interne au conteneur, invisible à `docker compose logs`, et
      # le diagnostic des tâches 4 et 9 devient aveugle.
      - --log-file=stdout
      - --verbose
```

- [ ] **Étape 2 : Renseigner l'environnement**

Ajouter à `.env` (fichier déjà ignoré par git — vérifier avec
`git check-ignore -v .env`) :

```
TURN_SECRET=<générer : openssl rand -hex 32>
TURN_REALM=guacamole.local
TURN_EXTERNAL_IP=192.168.3.1
```

Générer le secret sans le recopier à la main :

```bash
echo "TURN_SECRET=$(openssl rand -hex 32)" >> .env
```

- [ ] **Étape 3 : Démarrer et vérifier que le port écoute**

```bash
docker compose up -d coturn
docker compose logs coturn | tail -20
ss -lunp | grep 3478
```

Attendu : le journal ne contient aucune ligne `ERROR`, et le port 3478/UDP
écoute.

- [ ] **Étape 4 : Prouver qu'une allocation réussit, avant d'écrire la moindre ligne de Rust**

C'est le point de contrôle qui évite de déboguer un client tout neuf contre un
serveur mal configuré. `turnutils_uclient` est livré avec coturn.

Fabriquer un identifiant éphémère à la main, exactement comme le fera le
signaling à la tâche 2 :

```bash
set -a && source .env && set +a
USER="$(( $(date +%s) + 3600 )):essai"
PASS=$(printf '%s' "$USER" | openssl dgst -binary -sha1 -hmac "$TURN_SECRET" | base64)
echo "utilisateur=$USER"

docker compose exec coturn turnutils_uclient \
    -u "$USER" -w "$PASS" -p 3478 -n 2 -y 192.168.3.1
```

Attendu : la sortie annonce une allocation réussie et des paquets échangés
(`total transmit ... packets`). Une erreur `401` persistante signale un secret
qui ne correspond pas ; une erreur `400` signale une requête mal formée — mais
`turnutils_uclient` étant l'outil de référence de coturn, une 400 ici pointe
la configuration du serveur, pas le client.

**Ne pas passer à la suite tant que cette étape n'est pas concluante.** Tout ce
que les tâches 3 à 7 construisent suppose ce serveur fonctionnel.

- [ ] **Étape 5 : Commit**

```bash
git add docker-compose.yml
git commit -m "feat(nat): serveur TURN coturn en identifiants éphémères"
```

---

## Tâche 2 : Le signaling délivre la configuration ICE

**Fichiers :**
- Créer : `signaling/src/ice.ts`
- Créer : `signaling/src/ice.test.ts`
- Modifier : `signaling/src/server.ts`

**Interfaces :**
- Produit : `derverIdentifiants(secret, session, dureeSecondes, maintenant) ->
  { username, credential }` et `configurationIce(env, session, maintenant) ->
  { iceServers } | undefined`. Le serveur émet
  `{ type: 'ice-config', iceServers: [...] }` au pair juste après sa déclaration
  de rôle. Consommé par les tâches 7 et 8.

- [ ] **Étape 1 : Écrire les tests qui échouent**

Créer `signaling/src/ice.test.ts` :

```typescript
import { createHmac } from 'node:crypto';
import { describe, expect, it } from 'vitest';
import { configurationIce, deriverIdentifiants } from './ice';

describe('deriverIdentifiants', () => {
    it("préfixe le nom d'utilisateur par l'instant d'expiration", () => {
        // `maintenant` est en MILLISECONDES (comme `Date.now()`), la durée en
        // SECONDES : 1 000 000 ms = 1 000 s, plus 3 600 s, donc 4 600.
        // Confondre les deux unités produit des identifiants valides mille
        // fois trop longtemps — d'où ce test sur une valeur exacte.
        const { username } = deriverIdentifiants('secret', 'ma-session', 3600, 1_000_000);
        // Format imposé par coturn en mode use-auth-secret : <expiration>:<qui>
        expect(username).toBe('4600:ma-session');
    });

    it('dérive le mot de passe par HMAC-SHA1 du nom, encodé en base64', () => {
        const { username, credential } = deriverIdentifiants('secret', 's', 60, 0);
        const attendu = createHmac('sha1', 'secret').update(username).digest('base64');
        expect(credential).toBe(attendu);
    });

    it('produit des identifiants différents pour deux sessions', () => {
        const a = deriverIdentifiants('secret', 'a', 60, 0);
        const b = deriverIdentifiants('secret', 'b', 60, 0);
        expect(a.credential).not.toBe(b.credential);
    });
});

describe('configurationIce', () => {
    it('rend undefined quand aucun serveur TURN n’est configuré', () => {
        // Absence de configuration = déploiement local sans TURN. Ce n'est
        // pas une erreur : la session doit continuer avec les seuls
        // candidats hôtes.
        expect(configurationIce({}, 's', 0)).toBeUndefined();
    });

    it('rend une entrée iceServers complète quand tout est configuré', () => {
        const config = configurationIce(
            { TURN_URL: 'turn:192.168.3.1:3478', TURN_SECRET: 'secret' },
            'ma-session',
            0,
        );
        expect(config).toEqual({
            iceServers: [
                {
                    urls: 'turn:192.168.3.1:3478',
                    username: '86400:ma-session',
                    credential: expect.any(String),
                },
            ],
        });
    });

    it('rend undefined si l’URL est là mais pas le secret', () => {
        // Une configuration à moitié posée est une erreur de déploiement.
        // Émettre une entrée sans identifiants valides ferait échouer toutes
        // les allocations en 401, avec un diagnostic bien plus obscur.
        expect(configurationIce({ TURN_URL: 'turn:x:3478' }, 's', 0)).toBeUndefined();
    });
});
```

- [ ] **Étape 2 : Lancer les tests pour vérifier qu'ils échouent**

```bash
cd signaling && npm test -- ice
```

Attendu : ÉCHEC — `Failed to resolve import "./ice"`.

- [ ] **Étape 3 : Écrire l'implémentation**

Créer `signaling/src/ice.ts` :

```typescript
// Identifiants TURN éphémères et configuration ICE délivrée aux deux pairs.
//
// Le secret partagé avec coturn ne quitte JAMAIS ce processus : les pairs ne
// reçoivent qu'un mot de passe dérivé, daté, et propre à leur session. Un
// identifiant intercepté expire tout seul.
//
// Séparé du serveur WebSocket pour être testable sans ouvrir de socket.

import { createHmac } from 'node:crypto';

/// Durée de validité par défaut d'un identifiant, en secondes.
///
/// Généreuse à dessein : l'identifiant sert à ALLOUER, et une allocation se
/// rafraîchit ensuite avec les mêmes identifiants. Une durée courte ferait
/// échouer le rafraîchissement au milieu d'une longue session de jeu.
const DUREE_SECONDES = 86_400;

export interface Identifiants {
    username: string;
    credential: string;
}

export interface ConfigurationIce {
    iceServers: Array<{ urls: string; username: string; credential: string }>;
}

/// Fabrique un couple identifiant/mot de passe accepté par coturn en mode
/// `use-auth-secret` (`--static-auth-secret`).
///
/// `maintenant` est passé en paramètre plutôt que lu de l'horloge : c'est ce
/// qui rend la dérivation testable avec une valeur attendue exacte.
export function deriverIdentifiants(
    secret: string,
    session: string,
    dureeSecondes: number,
    maintenant: number,
): Identifiants {
    const expiration = Math.floor(maintenant / 1000) + dureeSecondes;
    const username = `${expiration}:${session}`;
    const credential = createHmac('sha1', secret).update(username).digest('base64');
    return { username, credential };
}

/// Configuration ICE à envoyer aux deux pairs, ou `undefined` si aucun serveur
/// TURN n'est configuré.
///
/// Les deux variables doivent être présentes ensemble : une configuration à
/// moitié posée produirait des allocations refusées en 401, avec un
/// diagnostic beaucoup plus obscur qu'une absence franche de relais.
export function configurationIce(
    env: Record<string, string | undefined>,
    session: string,
    maintenant: number,
): ConfigurationIce | undefined {
    const urls = env.TURN_URL;
    const secret = env.TURN_SECRET;
    if (!urls || !secret) return undefined;

    const { username, credential } = deriverIdentifiants(
        secret,
        session,
        DUREE_SECONDES,
        maintenant,
    );
    return { iceServers: [{ urls, username, credential }] };
}
```

- [ ] **Étape 4 : Lancer les tests pour vérifier qu'ils passent**

```bash
cd signaling && npm test -- ice
```

Attendu : SUCCÈS, 6 tests.

- [ ] **Étape 5 : Émettre la configuration à la déclaration de rôle**

Dans `signaling/src/server.ts`, importer :

```typescript
import { configurationIce } from './ice';
```

puis, dans le bloc `if (!role) { … }`, juste avant le `return;` final (après
`sessions.set(declaredSession, session);`) :

```typescript
                // Configuration ICE : envoyée à CHAQUE pair dès qu'il se
                // déclare, agent comme client. Les deux en ont besoin — le
                // relais TURN n'est utile que si les deux extrémités peuvent
                // l'employer.
                //
                // `Date.now()` est lu ici et non dans `configurationIce` :
                // cette dernière reste ainsi une fonction pure, testable
                // avec un instant fixé.
                const ice = configurationIce(process.env, declaredSession, Date.now());
                if (ice) {
                    send(socket, { type: 'ice-config', ...ice });
                } else {
                    // Trace explicite : une session sans relais qui échoue à
                    // se connecter depuis l'extérieur doit pouvoir être
                    // diagnostiquée sans relire le code.
                    console.warn(
                        'aucun serveur TURN configuré (TURN_URL/TURN_SECRET) : session sans relais',
                    );
                }
```

- [ ] **Étape 6 : Vérifier que rien d'existant ne casse**

```bash
cd signaling && npm test
```

Attendu : SUCCÈS de tous les tests, y compris `server.test.ts` et
`resilience.test.ts`. Ces tests ne définissent pas `TURN_URL`, donc
`configurationIce` rend `undefined` et aucun message supplémentaire n'est émis
— c'est exactement pourquoi le cas « non configuré » de l'étape 1 est testé.

Si un test échoue parce qu'il compte les messages reçus, ne pas l'assouplir :
lui faire ignorer explicitement le type `ice-config`, comme le client le fera.

- [ ] **Étape 7 : Commit**

```bash
git add signaling/src/ice.ts signaling/src/ice.test.ts signaling/src/server.ts
git commit -m "feat(nat): le signaling délivre des identifiants TURN éphémères aux deux pairs"
```

---

## Tâche 3 : Sérialiser les requêtes TURN

`is::stun` sait **lire** les réponses TURN mais ne sait pas **émettre** une
requête Allocate valide : l'attribut `REQUESTED-TRANSPORT` (0x0019) est absent
de sa table (spec §3.2). On écrit donc les quatre requêtes nous-mêmes, et on
lui délègue la lecture.

**Fichiers :**
- Modifier : `agent/Cargo.toml`
- Créer : `agent/src/turn.rs`
- Modifier : `agent/src/main.rs`

**Interfaces :**
- Produit : `turn::{Requete, Identifiants, cle_longue_duree, encoder_requete}`.
  Consommé par la tâche 4.

- [ ] **Étape 1 : Ajouter les dépendances**

Dans `agent/Cargo.toml`, sous `[dependencies]` :

```toml
# Codec STUN de str0m, en dépendance DIRECTE plutôt que par le module
# `str0m::ice` qui est `#[doc(hidden)]` — on ne bâtit pas sur une porte que
# l'auteur a explicitement fermée. Version alignée sur celle que Cargo.lock
# verrouille déjà comme dépendance transitive de str0m : aucun doublon dans
# l'arbre. Même précédent que `windows-core` plus bas.
is = "0.10"
# MESSAGE-INTEGRITY des requêtes TURN.
hmac = "0.12"
sha1 = "0.10"
# Clé longue durée : MD5(user:realm:pass), imposé par la RFC 5766. MD5 n'est
# employé ici QUE comme dérivation de clé normative, jamais comme primitive
# de sécurité.
#
# `md-5` (RustCrypto, API `Md5::digest`) et NON le crate `md5` d'Ivan Ukhov
# (API `md5::compute`) : le premier partage le trait `Digest` avec `sha1`
# déclaré juste au-dessus, donc une seule famille d'API dans ce module.
md-5 = "0.10"
```

Vérifier qu'aucune version en double n'apparaît :

```bash
cargo tree -p agent -i is
```

Attendu : une seule version, `is v0.10.0`.

- [ ] **Étape 2 : Écrire les tests qui échouent**

Créer `agent/src/turn.rs` :

```rust
//! Client TURN : machine à états et encapsulation, sans entrées-sorties.
//!
//! Ce module consomme des octets et rend des octets. Il ne possède aucun
//! socket, ne connaît pas `Rtc`, et ne dépend d'aucune API Windows — c'est ce
//! qui rend toute la machine à états testable sur Linux, sans coturn.
//!
//! Partage du travail avec `is::stun` : on LIT les réponses avec son parseur
//! (qui couvre tout le vocabulaire TURN dont on a besoin), on ÉCRIT les
//! requêtes soi-même. Son builder ne connaît pas `REQUESTED-TRANSPORT`
//! (0x0019), sans lequel un serveur conforme refuse toute allocation en 400.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_cle_longue_duree_est_le_md5_des_trois_champs() {
        use md5::Digest;

        // Forme imposée par la RFC 5766 §4 (qui reprend RFC 5389 §15.4) :
        // la clé d'intégrité vaut MD5("username:realm:password").
        let cle = cle_longue_duree("user", "example.org", "pass");
        let attendu = md5::Md5::digest(b"user:example.org:pass");
        assert_eq!(cle.as_slice(), attendu.as_slice());
        assert_eq!(cle.len(), 16, "un condensé MD5 fait 16 octets");
    }

    #[test]
    fn une_allocation_nue_porte_requested_transport_et_pas_d_integrite() {
        let trans_id = [7u8; 12];
        let paquet = encoder_requete(&Requete::AllocateNu, trans_id, None);

        // En-tête : type 0x0003 (Allocate, classe requête), cookie magique.
        assert_eq!(&paquet[0..2], &[0x00, 0x03], "méthode Allocate attendue");
        assert_eq!(&paquet[4..8], &[0x21, 0x12, 0xA4, 0x42], "cookie magique");
        assert_eq!(&paquet[8..20], &trans_id);

        // La longueur annoncée doit correspondre à ce qui suit l'en-tête.
        let longueur = u16::from_be_bytes([paquet[2], paquet[3]]) as usize;
        assert_eq!(longueur, paquet.len() - 20, "longueur d'en-tête incohérente");

        // REQUESTED-TRANSPORT = UDP (17), l'attribut que is::stun ne sait pas
        // écrire et sans lequel coturn répond 400.
        assert_eq!(
            &paquet[20..28],
            &[0x00, 0x19, 0x00, 0x04, 17, 0x00, 0x00, 0x00],
            "REQUESTED-TRANSPORT=UDP attendu en premier attribut"
        );

        // Aucune intégrité sur la requête nue : c'est elle qui provoque le
        // 401 porteur du realm et du nonce.
        assert_eq!(paquet.len(), 28, "aucun autre attribut attendu");
    }

    #[test]
    fn une_allocation_signee_est_relue_et_verifiee_par_is_stun() {
        // Le test le plus important du module : notre sérialiseur doit
        // produire un message que le parseur de référence accepte ET dont il
        // valide l'intégrité. C'est ce qui remplace un aller-retour avec un
        // vrai serveur.
        let ids = Identifiants {
            username: "user".into(),
            password: "pass".into(),
            realm: "example.org".into(),
            nonce: "abcdef".into(),
        };
        let cle = cle_longue_duree(&ids.username, &ids.realm, &ids.password);
        let paquet = encoder_requete(&Requete::AllocateSigne, [3u8; 12], Some((&ids, &cle)));

        let message = is::stun::StunMessage::parse(&paquet).expect("relu par is::stun");
        assert_eq!(message.username(), Some("user"));
        assert_eq!(message.realm(), Some("example.org"));
        assert_eq!(message.nonce(), Some("abcdef"));
        assert!(
            message.verify(&cle, sha1_hmac),
            "MESSAGE-INTEGRITY invalide : le serveur refuserait en 401"
        );
    }

    #[test]
    fn une_permission_porte_l_adresse_du_pair_en_xor() {
        let ids = Identifiants {
            username: "u".into(),
            password: "p".into(),
            realm: "r".into(),
            nonce: "n".into(),
        };
        let cle = cle_longue_duree(&ids.username, &ids.realm, &ids.password);
        let pair: std::net::SocketAddr = "203.0.113.7:5000".parse().unwrap();
        let paquet = encoder_requete(
            &Requete::CreatePermission { pair },
            [1u8; 12],
            Some((&ids, &cle)),
        );

        let message = is::stun::StunMessage::parse(&paquet).expect("relu par is::stun");
        // Le XOR de l'adresse est fait par nous et défait par le parseur :
        // si les deux ne s'accordent pas, cette égalité échoue.
        assert_eq!(message.xor_peer_address(), Some(pair));
        assert!(message.verify(&cle, sha1_hmac));
    }

    #[test]
    fn un_channel_bind_porte_le_numero_et_l_adresse() {
        let ids = Identifiants {
            username: "u".into(),
            password: "p".into(),
            realm: "r".into(),
            nonce: "n".into(),
        };
        let cle = cle_longue_duree(&ids.username, &ids.realm, &ids.password);
        let pair: std::net::SocketAddr = "198.51.100.9:1234".parse().unwrap();
        let paquet = encoder_requete(
            &Requete::ChannelBind { canal: 0x4000, pair },
            [2u8; 12],
            Some((&ids, &cle)),
        );

        let message = is::stun::StunMessage::parse(&paquet).expect("relu par is::stun");
        assert_eq!(message.channel_number(), Some(0x4000));
        assert_eq!(message.xor_peer_address(), Some(pair));
    }
}
```

- [ ] **Étape 3 : Lancer les tests pour vérifier qu'ils échouent**

Déclarer d'abord le module dans `agent/src/main.rs`, dans le bloc non
conditionné, en ordre alphabétique — après `mod transport;` :

```rust
mod turn;
```

Puis :

```bash
cargo test -p agent turn
```

Attendu : ÉCHEC de compilation — `cannot find function 'cle_longue_duree'`.

- [ ] **Étape 4 : Écrire l'implémentation**

Au-dessus du module de tests, dans `agent/src/turn.rs` :

```rust
use std::net::SocketAddr;

use hmac::{Hmac, Mac};

/// Cookie magique STUN (RFC 5389 §6).
const MAGIC: [u8; 4] = [0x21, 0x12, 0xA4, 0x42];

// Méthodes TURN. Classe « requête » valant 0b00, le type sur le fil est la
// méthode elle-même.
const METHODE_ALLOCATE: u16 = 0x0003;
const METHODE_REFRESH: u16 = 0x0004;
const METHODE_CREATE_PERMISSION: u16 = 0x0008;
const METHODE_CHANNEL_BIND: u16 = 0x0009;

// Attributs employés. `REQUESTED_TRANSPORT` est celui qui manque à
// `is::stun` et qui motive tout ce sérialiseur.
const ATTR_USERNAME: u16 = 0x0006;
const ATTR_MESSAGE_INTEGRITY: u16 = 0x0008;
const ATTR_CHANNEL_NUMBER: u16 = 0x000C;
const ATTR_LIFETIME: u16 = 0x000D;
const ATTR_XOR_PEER_ADDRESS: u16 = 0x0012;
const ATTR_REALM: u16 = 0x0014;
const ATTR_NONCE: u16 = 0x0015;
const ATTR_REQUESTED_TRANSPORT: u16 = 0x0019;

/// Numéro de protocole d'IANA pour UDP, valeur du champ `REQUESTED-TRANSPORT`.
const TRANSPORT_UDP: u8 = 17;

/// Durée de bail demandée à l'allocation, en secondes. Le serveur peut en
/// accorder une autre — c'est celle qu'il annonce qui fait foi, et le
/// rafraîchissement se cale dessus (voir tâche 4).
pub const BAIL_DEMANDE_S: u32 = 600;

/// Identifiants longue durée, tels que le serveur les impose dans sa
/// réponse 401.
#[derive(Debug, Clone)]
pub struct Identifiants {
    pub username: String,
    pub password: String,
    pub realm: String,
    pub nonce: String,
}

/// Requête à émettre. Chaque variante porte exactement ce qui la distingue.
pub enum Requete {
    /// Première tentative, sans identifiants : elle SERT à provoquer le 401
    /// qui révèle le realm et le nonce. Ce n'est pas un échec, c'est l'étape
    /// normale du protocole.
    AllocateNu,
    AllocateSigne,
    Refresh { duree_s: u32 },
    CreatePermission { pair: SocketAddr },
    ChannelBind { canal: u16, pair: SocketAddr },
}

/// HMAC-SHA1, dans la forme que réclament `is::stun::verify` et `to_bytes`.
pub fn sha1_hmac(cle: &[u8], morceaux: &[&[u8]]) -> [u8; 20] {
    let mut mac = Hmac::<sha1::Sha1>::new_from_slice(cle).expect("HMAC accepte toute longueur");
    for morceau in morceaux {
        mac.update(morceau);
    }
    mac.finalize().into_bytes().into()
}

/// Clé d'intégrité longue durée : `MD5(username:realm:password)` (RFC 5766
/// §4, qui reprend RFC 5389 §15.4).
///
/// MD5 est ici une dérivation de clé normative, pas un choix : le serveur
/// calcule la même, et toute autre fonction produirait un 401 systématique.
pub fn cle_longue_duree(username: &str, realm: &str, password: &str) -> Vec<u8> {
    use md5::Digest;
    md5::Md5::digest(format!("{username}:{realm}:{password}").as_bytes()).to_vec()
}

/// Sérialise une requête TURN complète, prête à être envoyée.
///
/// `identifiants` absent produit une requête nue (sans USERNAME/REALM/NONCE ni
/// MESSAGE-INTEGRITY) : c'est la forme de la première tentative d'allocation.
///
/// FINGERPRINT n'est pas émis : il est facultatif en TURN, et l'omettre évite
/// d'avoir à l'inclure dans le calcul d'intégrité.
pub fn encoder_requete(
    requete: &Requete,
    trans_id: [u8; 12],
    identifiants: Option<(&Identifiants, &[u8])>,
) -> Vec<u8> {
    let methode = match requete {
        Requete::AllocateNu | Requete::AllocateSigne => METHODE_ALLOCATE,
        Requete::Refresh { .. } => METHODE_REFRESH,
        Requete::CreatePermission { .. } => METHODE_CREATE_PERMISSION,
        Requete::ChannelBind { .. } => METHODE_CHANNEL_BIND,
    };

    let mut attributs: Vec<u8> = Vec::new();

    // L'ordre suit celui de la RFC : les attributs propres à la méthode, puis
    // les attributs d'authentification, puis MESSAGE-INTEGRITY en dernier.
    match requete {
        Requete::AllocateNu | Requete::AllocateSigne => {
            ecrire_attribut(
                &mut attributs,
                ATTR_REQUESTED_TRANSPORT,
                &[TRANSPORT_UDP, 0, 0, 0],
            );
            if matches!(requete, Requete::AllocateSigne) {
                ecrire_attribut(&mut attributs, ATTR_LIFETIME, &BAIL_DEMANDE_S.to_be_bytes());
            }
        }
        Requete::Refresh { duree_s } => {
            ecrire_attribut(&mut attributs, ATTR_LIFETIME, &duree_s.to_be_bytes());
        }
        Requete::CreatePermission { pair } => {
            ecrire_attribut(&mut attributs, ATTR_XOR_PEER_ADDRESS, &xor_adresse(*pair, &trans_id));
        }
        Requete::ChannelBind { canal, pair } => {
            ecrire_attribut(
                &mut attributs,
                ATTR_CHANNEL_NUMBER,
                &[(canal >> 8) as u8, *canal as u8, 0, 0],
            );
            ecrire_attribut(&mut attributs, ATTR_XOR_PEER_ADDRESS, &xor_adresse(*pair, &trans_id));
        }
    }

    if let Some((ids, _)) = identifiants {
        ecrire_attribut(&mut attributs, ATTR_USERNAME, ids.username.as_bytes());
        ecrire_attribut(&mut attributs, ATTR_REALM, ids.realm.as_bytes());
        ecrire_attribut(&mut attributs, ATTR_NONCE, ids.nonce.as_bytes());
    }

    let mut paquet = Vec::with_capacity(20 + attributs.len() + 24);
    paquet.extend_from_slice(&methode.to_be_bytes());
    // Longueur : renseignée après, une fois connue. Les 24 octets de
    // MESSAGE-INTEGRITY doivent être COMPTÉS dans la longueur au moment où
    // l'empreinte est calculée — c'est la subtilité qui fait échouer la
    // plupart des implémentations naïves.
    paquet.extend_from_slice(&[0, 0]);
    paquet.extend_from_slice(&MAGIC);
    paquet.extend_from_slice(&trans_id);
    paquet.extend_from_slice(&attributs);

    let Some((_, cle)) = identifiants else {
        let longueur = (paquet.len() - 20) as u16;
        paquet[2..4].copy_from_slice(&longueur.to_be_bytes());
        return paquet;
    };

    // Longueur annoncée AVANT le calcul : elle inclut déjà l'attribut
    // MESSAGE-INTEGRITY qui n'est pas encore écrit (4 octets d'en-tête + 20
    // d'empreinte).
    let longueur_avec_integrite = (paquet.len() - 20 + 24) as u16;
    paquet[2..4].copy_from_slice(&longueur_avec_integrite.to_be_bytes());

    let empreinte = sha1_hmac(cle, &[&paquet]);
    ecrire_attribut(&mut paquet, ATTR_MESSAGE_INTEGRITY, &empreinte);
    paquet
}

/// Écrit un attribut TLV, complété à un multiple de 4 octets.
fn ecrire_attribut(sortie: &mut Vec<u8>, type_: u16, valeur: &[u8]) {
    sortie.extend_from_slice(&type_.to_be_bytes());
    sortie.extend_from_slice(&(valeur.len() as u16).to_be_bytes());
    sortie.extend_from_slice(valeur);
    // Le remplissage n'est PAS compté dans la longueur annoncée de
    // l'attribut, mais il doit être présent sur le fil.
    let reste = valeur.len() % 4;
    if reste != 0 {
        sortie.extend_from_slice(&[0u8; 4][..4 - reste]);
    }
}

/// Encode une adresse au format XOR-MAPPED-ADDRESS (RFC 5389 §15.2).
///
/// Le port est masqué par les 16 bits de poids fort du cookie magique ;
/// l'adresse par le cookie entier en IPv4, ou par cookie ‖ identifiant de
/// transaction en IPv6.
fn xor_adresse(addr: SocketAddr, trans_id: &[u8; 12]) -> Vec<u8> {
    let mut sortie = vec![0u8, 0];
    let port = addr.port() ^ u16::from_be_bytes([MAGIC[0], MAGIC[1]]);
    match addr {
        SocketAddr::V4(v4) => {
            sortie[1] = 0x01;
            sortie.extend_from_slice(&port.to_be_bytes());
            for (i, octet) in v4.ip().octets().iter().enumerate() {
                sortie.push(octet ^ MAGIC[i]);
            }
        }
        SocketAddr::V6(v6) => {
            sortie[1] = 0x02;
            sortie.extend_from_slice(&port.to_be_bytes());
            let mut masque = [0u8; 16];
            masque[..4].copy_from_slice(&MAGIC);
            masque[4..].copy_from_slice(trans_id);
            for (i, octet) in v6.ip().octets().iter().enumerate() {
                sortie.push(octet ^ masque[i]);
            }
        }
    }
    sortie
}
```

- [ ] **Étape 5 : Lancer les tests pour vérifier qu'ils passent**

```bash
cargo test -p agent turn
```

Attendu : SUCCÈS, 5 tests. L'échec le plus probable est
`une_allocation_signee_est_relue_et_verifiee_par_is_stun` sur `verify` :
il signale que la longueur annoncée au moment du calcul d'empreinte est fausse
— relire le commentaire de `longueur_avec_integrite`, c'est le piège que ce
test existe pour attraper.

- [ ] **Étape 6 : Commit**

```bash
git add agent/Cargo.toml Cargo.lock agent/src/turn.rs agent/src/main.rs
git commit -m "feat(nat): sérialisation des requêtes TURN, relue par le parseur de str0m"
```

---

## Tâche 4 : Machine à états de l'allocation

**Fichiers :**
- Modifier : `agent/src/turn.rs`

**Interfaces :**
- Consomme : `Requete`, `Identifiants`, `encoder_requete`, `cle_longue_duree`
  (tâche 3).
- Produit : `turn::{TurnClient, Allocation}` avec `new(serveur, username,
  password)`, `poll_transmit() -> Option<Vec<u8>>`, `handle_packet(&[u8]) ->
  Result<Option<Relayed>>`, `poll_timeout() -> Option<Instant>`,
  `allocation() -> Option<Allocation>`. Consommé par les tâches 5, 6 et 7.

- [ ] **Étape 1 : Écrire les tests qui échouent**

Ajouter dans le module `tests` de `agent/src/turn.rs` :

```rust
    use std::time::{Duration, Instant};

    fn t0() -> Instant {
        Instant::now() - Duration::from_secs(3600)
    }

    fn serveur() -> SocketAddr {
        "192.0.2.1:3478".parse().unwrap()
    }

    /// Fabrique la réponse qu'un serveur produirait, pour piloter le client
    /// sans réseau. C'est ce qui remplace coturn dans ces tests.
    fn reponse(
        methode: u16,
        classe_succes: bool,
        trans_id: [u8; 12],
        attributs: &[(u16, Vec<u8>)],
    ) -> Vec<u8> {
        let mut corps = Vec::new();
        for (type_, valeur) in attributs {
            ecrire_attribut(&mut corps, *type_, valeur);
        }
        // Classe succès = 0b10 → bits 0x0100 ; classe erreur = 0b11 → 0x0110.
        let type_fil = methode | if classe_succes { 0x0100 } else { 0x0110 };
        let mut paquet = Vec::new();
        paquet.extend_from_slice(&type_fil.to_be_bytes());
        paquet.extend_from_slice(&((corps.len()) as u16).to_be_bytes());
        paquet.extend_from_slice(&MAGIC);
        paquet.extend_from_slice(&trans_id);
        paquet.extend_from_slice(&corps);
        paquet
    }

    fn trans_id_de(paquet: &[u8]) -> [u8; 12] {
        paquet[8..20].try_into().unwrap()
    }

    #[test]
    fn la_premiere_emission_est_une_allocation_nue() {
        let mut c = TurnClient::new(serveur(), "u".into(), "p".into(), t0());
        let paquet = c.poll_transmit().expect("une allocation doit partir");
        assert_eq!(&paquet[0..2], &[0x00, 0x03], "Allocate attendu");
        // Nue : rien après REQUESTED-TRANSPORT.
        assert_eq!(paquet.len(), 28);
        // Rien d'autre tant qu'aucune réponse n'est arrivée.
        assert!(c.poll_transmit().is_none(), "pas de rafale d'allocations");
        assert!(c.allocation().is_none());
    }

    #[test]
    fn un_401_declenche_une_allocation_signee() {
        let mut c = TurnClient::new(serveur(), "u".into(), "p".into(), t0());
        let nue = c.poll_transmit().expect("allocation nue");

        let refus = reponse(
            METHODE_ALLOCATE,
            false,
            trans_id_de(&nue),
            &[
                (ATTR_ERROR_CODE, vec![0, 0, 4, 1, b'U', b'n', b'a', b'u']),
                (ATTR_REALM, b"example.org".to_vec()),
                (ATTR_NONCE, b"nonce1".to_vec()),
            ],
        );
        assert!(c.handle_packet(&refus).expect("401 traité").is_none());

        let signee = c.poll_transmit().expect("allocation signée attendue");
        let message = is::stun::StunMessage::parse(&signee).expect("relue");
        assert_eq!(message.realm(), Some("example.org"));
        assert_eq!(message.nonce(), Some("nonce1"));
        assert!(message.verify(
            &cle_longue_duree("u", "example.org", "p"),
            sha1_hmac
        ));
    }

    #[test]
    fn un_succes_rend_l_adresse_relayee_et_l_adresse_reflexive() {
        let mut c = allouee();
        let a = c.allocation().expect("allocation obtenue");
        assert_eq!(a.relayee, "192.0.2.15:50000".parse::<SocketAddr>().unwrap());
        assert_eq!(a.reflexive, Some("203.0.113.4:41234".parse().unwrap()));
    }

    /// Amène un client jusqu'à l'état alloué, pour les tests qui partent de là.
    fn allouee() -> TurnClient {
        let mut c = TurnClient::new(serveur(), "u".into(), "p".into(), t0());
        let nue = c.poll_transmit().unwrap();
        let refus = reponse(
            METHODE_ALLOCATE,
            false,
            trans_id_de(&nue),
            &[
                (ATTR_ERROR_CODE, vec![0, 0, 4, 1, b'x']),
                (ATTR_REALM, b"r".to_vec()),
                (ATTR_NONCE, b"n1".to_vec()),
            ],
        );
        c.handle_packet(&refus).unwrap();
        let signee = c.poll_transmit().unwrap();
        let succes = reponse(
            METHODE_ALLOCATE,
            true,
            trans_id_de(&signee),
            &[
                (
                    ATTR_XOR_RELAYED_ADDRESS,
                    xor_adresse("192.0.2.15:50000".parse().unwrap(), &trans_id_de(&signee)),
                ),
                (
                    ATTR_XOR_MAPPED_ADDRESS,
                    xor_adresse("203.0.113.4:41234".parse().unwrap(), &trans_id_de(&signee)),
                ),
                (ATTR_LIFETIME, 600u32.to_be_bytes().to_vec()),
            ],
        );
        c.handle_packet(&succes).unwrap();
        c
    }

    #[test]
    fn un_nonce_perime_est_rejoue_et_non_abandonne() {
        // 438 « Stale Nonce » est le cas d'erreur RÉELLEMENT rencontré :
        // coturn fait tourner ses nonces. Le traiter comme un échec
        // terminerait l'allocation au bout de quelques minutes.
        let mut c = allouee();
        c.avancer(t0() + Duration::from_secs(300));
        let refresh = c.poll_transmit().expect("rafraîchissement attendu");

        let perime = reponse(
            METHODE_REFRESH,
            false,
            trans_id_de(&refresh),
            &[
                (ATTR_ERROR_CODE, vec![0, 0, 4, 38, b'x']),
                (ATTR_REALM, b"r".to_vec()),
                (ATTR_NONCE, b"n2".to_vec()),
            ],
        );
        c.handle_packet(&perime).expect("438 traité");

        let rejoue = c.poll_transmit().expect("la requête doit repartir");
        let message = is::stun::StunMessage::parse(&rejoue).expect("relue");
        assert_eq!(message.nonce(), Some("n2"), "le nouveau nonce doit être employé");
        assert!(c.allocation().is_some(), "l'allocation ne doit pas être perdue");
    }

    #[test]
    fn le_rafraichissement_tombe_a_la_moitie_du_bail() {
        let mut c = allouee();
        // Bail de 600 s : rien avant 300 s.
        c.avancer(t0() + Duration::from_secs(299));
        assert!(c.poll_transmit().is_none(), "rafraîchissement trop précoce");
        c.avancer(t0() + Duration::from_secs(300));
        let paquet = c.poll_transmit().expect("rafraîchissement attendu");
        assert_eq!(&paquet[0..2], &[0x00, 0x04], "Refresh attendu");
    }
```

- [ ] **Étape 2 : Lancer les tests pour vérifier qu'ils échouent**

```bash
cargo test -p agent turn
```

Attendu : ÉCHEC de compilation — `cannot find type 'TurnClient'`.

- [ ] **Étape 3 : Écrire l'implémentation**

Ajouter les constantes d'attributs manquantes, à côté des autres :

```rust
const ATTR_ERROR_CODE: u16 = 0x0009;
const ATTR_XOR_RELAYED_ADDRESS: u16 = 0x0016;
const ATTR_XOR_MAPPED_ADDRESS: u16 = 0x0020;
```

puis, après `xor_adresse` :

```rust
use std::time::Instant;

/// Ce qu'une allocation réussie procure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Allocation {
    /// Adresse que le serveur relaie vers nous. C'est elle qui devient un
    /// candidat ICE relayé.
    pub relayee: SocketAddr,
    /// Adresse réflexive vue par le serveur. Offerte par la MÊME réponse —
    /// d'où l'absence de serveur STUN séparé (voir la spec §3.3).
    pub reflexive: Option<SocketAddr>,
}

/// Nombre de tentatives d'allocation avant abandon. Au-delà, la session
/// continue sans relais : c'est une dégradation, pas une panne.
const TENTATIVES_MAX: u8 = 5;

enum Etat {
    /// Rien n'est encore parti.
    Repos,
    /// Allocation nue émise, on attend le 401.
    AttenteRefus { trans_id: [u8; 12] },
    /// Allocation signée émise, on attend le succès.
    AttenteAllocation { trans_id: [u8; 12] },
    Allouee { echeance_refresh: Instant },
    /// Rafraîchissement émis, on attend sa confirmation.
    AttenteRefresh { trans_id: [u8; 12], echeance_refresh: Instant },
    /// Abandon définitif. La session continue sans relais.
    Abandonnee,
}

pub struct TurnClient {
    serveur: SocketAddr,
    username: String,
    password: String,
    identifiants: Option<Identifiants>,
    cle: Option<Vec<u8>>,
    etat: Etat,
    allocation: Option<Allocation>,
    /// Requêtes prêtes à partir, dans l'ordre.
    sortantes: std::collections::VecDeque<Vec<u8>>,
    maintenant: Instant,
    tentatives: u8,
    /// Compteur d'identifiants de transaction. Un identifiant STUN doit être
    /// imprévisible en usage réel ; ici il doit surtout être UNIQUE, et un
    /// compteur le garantit de façon reproductible en test.
    compteur_trans: u64,
}

impl TurnClient {
    pub fn new(serveur: SocketAddr, username: String, password: String, now: Instant) -> Self {
        let mut client = Self {
            serveur,
            username,
            password,
            identifiants: None,
            cle: None,
            etat: Etat::Repos,
            allocation: None,
            sortantes: std::collections::VecDeque::new(),
            maintenant: now,
            tentatives: 0,
            compteur_trans: 0,
        };
        client.emettre_allocation_nue();
        client
    }

    pub fn serveur(&self) -> SocketAddr {
        self.serveur
    }

    pub fn allocation(&self) -> Option<Allocation> {
        self.allocation
    }

    /// Prochain paquet à envoyer au serveur TURN, s'il y en a un.
    pub fn poll_transmit(&mut self) -> Option<Vec<u8>> {
        self.sortantes.pop_front()
    }

    /// Instant du prochain réveil utile, pour que l'appelant ne dorme pas
    /// au-delà.
    pub fn poll_timeout(&self) -> Option<Instant> {
        match self.etat {
            Etat::Allouee { echeance_refresh }
            | Etat::AttenteRefresh { echeance_refresh, .. } => Some(echeance_refresh),
            _ => None,
        }
    }

    /// Fait avancer l'horloge interne, et émet le rafraîchissement s'il est dû.
    pub fn avancer(&mut self, now: Instant) {
        self.maintenant = now;
        if let Etat::Allouee { echeance_refresh } = self.etat {
            if now >= echeance_refresh {
                self.emettre_refresh(echeance_refresh);
            }
        }
    }

    fn prochain_trans_id(&mut self) -> [u8; 12] {
        self.compteur_trans += 1;
        let mut id = [0u8; 12];
        id[..8].copy_from_slice(&self.compteur_trans.to_be_bytes());
        id
    }

    fn emettre_allocation_nue(&mut self) {
        let trans_id = self.prochain_trans_id();
        self.sortantes
            .push_back(encoder_requete(&Requete::AllocateNu, trans_id, None));
        self.etat = Etat::AttenteRefus { trans_id };
    }

    fn emettre_allocation_signee(&mut self) {
        let (Some(ids), Some(cle)) = (self.identifiants.clone(), self.cle.clone()) else {
            return;
        };
        let trans_id = self.prochain_trans_id();
        self.sortantes.push_back(encoder_requete(
            &Requete::AllocateSigne,
            trans_id,
            Some((&ids, &cle)),
        ));
        self.etat = Etat::AttenteAllocation { trans_id };
    }

    fn emettre_refresh(&mut self, echeance_refresh: Instant) {
        let (Some(ids), Some(cle)) = (self.identifiants.clone(), self.cle.clone()) else {
            return;
        };
        let trans_id = self.prochain_trans_id();
        self.sortantes.push_back(encoder_requete(
            &Requete::Refresh { duree_s: BAIL_DEMANDE_S },
            trans_id,
            Some((&ids, &cle)),
        ));
        self.etat = Etat::AttenteRefresh { trans_id, echeance_refresh };
    }

    /// Traite un paquet venant du serveur TURN.
    ///
    /// Rend `Ok(None)` pour tout message de service (réponse d'allocation,
    /// de rafraîchissement, erreur) : il est absorbé par la machine à états.
    /// Les données relayées sont traitées par `desencapsuler` (tâche 5).
    pub fn handle_packet(&mut self, data: &[u8]) -> anyhow::Result<Option<()>> {
        let message = is::stun::StunMessage::parse(data)
            .map_err(|e| anyhow::anyhow!("message TURN illisible : {e}"))?;

        if let Some((code, _raison)) = message.error_code() {
            self.traiter_erreur(code, &message);
            return Ok(None);
        }

        // Réponse de succès : allocation ou rafraîchissement.
        if let Some(relayee) = message.xor_relayed_address() {
            self.allocation = Some(Allocation { relayee, reflexive: message.mapped_address() });
            self.tentatives = 0;
        }
        let bail = message.lifetime().unwrap_or(BAIL_DEMANDE_S);
        // Rafraîchir à la MOITIÉ du bail : une seule perte de paquet ne doit
        // pas suffire à perdre l'allocation.
        let echeance = self.maintenant + std::time::Duration::from_secs((bail / 2).max(1) as u64);
        self.etat = Etat::Allouee { echeance_refresh: echeance };
        Ok(None)
    }

    fn traiter_erreur(&mut self, code: u16, message: &is::stun::StunMessage<'_>) {
        match code {
            // 401 : premier refus, porteur du realm et du nonce. Étape
            // normale du protocole, pas un échec.
            // 438 : nonce périmé — coturn les fait tourner. Même conduite :
            // adopter le nouveau nonce et rejouer.
            401 | 438 => {
                let (Some(realm), Some(nonce)) = (message.realm(), message.nonce()) else {
                    self.abandonner("401/438 sans realm ni nonce");
                    return;
                };
                self.identifiants = Some(Identifiants {
                    username: self.username.clone(),
                    password: self.password.clone(),
                    realm: realm.to_string(),
                    nonce: nonce.to_string(),
                });
                self.cle = Some(cle_longue_duree(&self.username, realm, &self.password));

                self.tentatives += 1;
                if self.tentatives > TENTATIVES_MAX {
                    self.abandonner("trop de refus d'authentification");
                    return;
                }

                // Un 438 sur un rafraîchissement ne doit PAS réallouer :
                // l'allocation existe toujours côté serveur, il faut rejouer
                // le rafraîchissement avec le nouveau nonce.
                match self.etat {
                    Etat::AttenteRefresh { echeance_refresh, .. } => {
                        self.emettre_refresh(echeance_refresh)
                    }
                    _ => self.emettre_allocation_signee(),
                }
            }
            autre => {
                self.abandonner(&format!("erreur TURN {autre}"));
            }
        }
    }

    fn abandonner(&mut self, raison: &str) {
        tracing::warn!(raison, "allocation TURN abandonnée : la session continuera sans relais");
        self.etat = Etat::Abandonnee;
        self.allocation = None;
    }
}
```

- [ ] **Étape 4 : Lancer les tests pour vérifier qu'ils passent**

```bash
cargo test -p agent turn
```

Attendu : SUCCÈS, 10 tests.

> Si `un_succes_rend_l_adresse_relayee_et_l_adresse_reflexive` échoue sur
> `reflexive`, vérifier le nom du lecteur d'adresse réflexive dans `is::stun` :
> ```bash
> grep -n "pub fn mapped_address\|pub fn xor_mapped" \
>   /root/.cargo/registry/src/index.crates.io-*/is-0.10.0/src/stun.rs
> ```
> et employer celui que rend cette commande.

- [ ] **Étape 5 : Commit**

```bash
git add agent/src/turn.rs
git commit -m "feat(nat): machine à états d'allocation TURN, nonce périmé rejoué"
```

---

## Tâche 5 : ChannelData et démultiplexage

**Fichiers :**
- Modifier : `agent/src/turn.rs`

**Interfaces :**
- Consomme : `TurnClient` (tâche 4).
- Produit : `TurnClient::{lier_canal, encapsuler, desencapsuler}`,
  `turn::est_channel_data(&[u8]) -> bool`. Consommé par la tâche 6.

- [ ] **Étape 1 : Écrire les tests qui échouent**

Ajouter dans le module `tests` :

```rust
    #[test]
    fn le_premier_octet_departage_stun_de_channel_data() {
        // Les deux bits de poids fort : 00 = STUN, 01 = ChannelData. C'est le
        // seul démultiplexage dont on dispose sur un socket UDP partagé.
        assert!(!est_channel_data(&[0x00, 0x03, 0, 0]), "Allocate est du STUN");
        assert!(!est_channel_data(&[0x01, 0x01, 0, 0]), "réponse STUN");
        assert!(est_channel_data(&[0x40, 0x00, 0, 0]), "premier canal valide");
        assert!(est_channel_data(&[0x7F, 0xFF, 0, 0]), "dernier canal valide");
        assert!(!est_channel_data(&[]), "un paquet vide n'est rien");
    }

    #[test]
    fn encapsuler_prefixe_le_numero_de_canal_et_la_longueur() {
        let mut c = allouee();
        let pair: SocketAddr = "203.0.113.9:6000".parse().unwrap();
        let canal = c.lier_canal(pair).expect("canal attribué");

        let encapsule = c.encapsuler(pair, &[1, 2, 3]).expect("pair connu");
        assert_eq!(&encapsule[0..2], &canal.to_be_bytes());
        assert_eq!(&encapsule[2..4], &3u16.to_be_bytes());
        assert_eq!(&encapsule[4..7], &[1, 2, 3]);
        // Sur UDP, aucun remplissage n'est requis sur le dernier paquet.
        assert_eq!(encapsule.len(), 7);
    }

    #[test]
    fn encapsuler_refuse_un_pair_sans_canal() {
        let mut c = allouee();
        let inconnu: SocketAddr = "198.51.100.1:1".parse().unwrap();
        assert!(c.encapsuler(inconnu, &[1]).is_none(), "aucun canal lié pour ce pair");
    }

    #[test]
    fn lier_un_canal_emet_permission_puis_channel_bind() {
        let mut c = allouee();
        // Vider ce qui restait en attente.
        while c.poll_transmit().is_some() {}

        let pair: SocketAddr = "203.0.113.9:6000".parse().unwrap();
        c.lier_canal(pair).expect("canal attribué");

        let premier = c.poll_transmit().expect("CreatePermission attendu");
        assert_eq!(&premier[0..2], &[0x00, 0x08], "CreatePermission");
        let second = c.poll_transmit().expect("ChannelBind attendu");
        assert_eq!(&second[0..2], &[0x00, 0x09], "ChannelBind");
    }

    #[test]
    fn les_numeros_de_canal_restent_dans_la_plage_normative() {
        let mut c = allouee();
        for i in 0..8u16 {
            let pair: SocketAddr = format!("203.0.113.{}:6000", i + 1).parse().unwrap();
            let canal = c.lier_canal(pair).expect("canal attribué");
            assert!(
                (0x4000..=0x7FFF).contains(&canal),
                "canal {canal:#x} hors de la plage 0x4000-0x7FFF"
            );
        }
    }

    #[test]
    fn desencapsuler_rend_le_pair_et_la_charge_utile() {
        let mut c = allouee();
        let pair: SocketAddr = "203.0.113.9:6000".parse().unwrap();
        let canal = c.lier_canal(pair).expect("canal attribué");

        let mut trame = Vec::new();
        trame.extend_from_slice(&canal.to_be_bytes());
        trame.extend_from_slice(&4u16.to_be_bytes());
        trame.extend_from_slice(&[9, 8, 7, 6]);

        let (source, charge) = c.desencapsuler(&trame).expect("trame reconnue");
        assert_eq!(source, pair);
        assert_eq!(charge, &[9, 8, 7, 6]);
    }

    #[test]
    fn desencapsuler_refuse_une_trame_tronquee_ou_inconnue() {
        let mut c = allouee();
        let pair: SocketAddr = "203.0.113.9:6000".parse().unwrap();
        let canal = c.lier_canal(pair).expect("canal");

        // Longueur annoncée plus grande que ce qui suit : une lecture naïve
        // paniquerait sur un découpage hors bornes.
        let mut tronquee = Vec::new();
        tronquee.extend_from_slice(&canal.to_be_bytes());
        tronquee.extend_from_slice(&99u16.to_be_bytes());
        tronquee.extend_from_slice(&[1, 2]);
        assert!(c.desencapsuler(&tronquee).is_none());

        // Canal jamais lié.
        let mut inconnue = Vec::new();
        inconnue.extend_from_slice(&0x7FFFu16.to_be_bytes());
        inconnue.extend_from_slice(&1u16.to_be_bytes());
        inconnue.push(0);
        assert!(c.desencapsuler(&inconnue).is_none());
    }
```

- [ ] **Étape 2 : Lancer les tests pour vérifier qu'ils échouent**

```bash
cargo test -p agent turn
```

Attendu : ÉCHEC de compilation — `cannot find function 'est_channel_data'`.

- [ ] **Étape 3 : Écrire l'implémentation**

Ajouter à `agent/src/turn.rs` :

```rust
/// Premier numéro de canal de la plage normative (RFC 5766 §2.5).
const CANAL_MIN: u16 = 0x4000;
const CANAL_MAX: u16 = 0x7FFF;

/// Vrai si ce datagramme est une trame ChannelData plutôt qu'un message STUN.
///
/// Les deux bits de poids fort du premier octet suffisent : STUN vaut `00`,
/// ChannelData vaut `01` (par construction, les numéros de canal valides
/// commencent tous par ces deux bits). C'est le démultiplexage prévu par la
/// RFC, et le seul possible sur un socket partagé.
pub fn est_channel_data(data: &[u8]) -> bool {
    matches!(data.first(), Some(premier) if premier >> 6 == 0b01)
}
```

et, dans `impl TurnClient`, les trois méthodes plus les deux champs :

```rust
    /// Attribue un canal à un pair et émet les requêtes nécessaires.
    ///
    /// `CreatePermission` d'abord, `ChannelBind` ensuite : la seconde
    /// implique la première côté serveur, mais les émettre toutes deux évite
    /// une fenêtre pendant laquelle le serveur jetterait nos paquets si le
    /// `ChannelBind` se perdait.
    ///
    /// Rend `None` si aucune allocation n'est en place, ou si la plage de
    /// canaux est épuisée.
    pub fn lier_canal(&mut self, pair: SocketAddr) -> Option<u16> {
        if self.allocation.is_none() {
            return None;
        }
        if let Some(canal) = self.canaux.iter().find(|(_, p)| **p == pair).map(|(c, _)| *c) {
            return Some(canal);
        }
        if self.prochain_canal > CANAL_MAX {
            tracing::warn!("plage de canaux TURN épuisée");
            return None;
        }
        let canal = self.prochain_canal;
        self.prochain_canal += 1;

        let (ids, cle) = (self.identifiants.clone()?, self.cle.clone()?);
        let trans_permission = self.prochain_trans_id();
        self.sortantes.push_back(encoder_requete(
            &Requete::CreatePermission { pair },
            trans_permission,
            Some((&ids, &cle)),
        ));
        let trans_bind = self.prochain_trans_id();
        self.sortantes.push_back(encoder_requete(
            &Requete::ChannelBind { canal, pair },
            trans_bind,
            Some((&ids, &cle)),
        ));

        self.canaux.insert(canal, pair);
        Some(canal)
    }

    /// Enveloppe une charge utile pour le pair donné.
    ///
    /// Rend `None` si aucun canal n'est lié à ce pair : l'appelant doit alors
    /// envoyer en direct, pas fabriquer une trame que le serveur jetterait.
    ///
    /// Aucun remplissage : la RFC 5766 §11.5 ne l'exige pas sur UDP.
    pub fn encapsuler(&self, pair: SocketAddr, charge: &[u8]) -> Option<Vec<u8>> {
        let canal = self.canaux.iter().find(|(_, p)| **p == pair).map(|(c, _)| *c)?;
        let mut trame = Vec::with_capacity(4 + charge.len());
        trame.extend_from_slice(&canal.to_be_bytes());
        trame.extend_from_slice(&(charge.len() as u16).to_be_bytes());
        trame.extend_from_slice(charge);
        Some(trame)
    }

    /// Extrait le pair d'origine et la charge utile d'une trame ChannelData.
    ///
    /// Rend `None` sur une trame tronquée ou dont le canal n'est pas lié :
    /// un datagramme malformé venu du réseau ne doit jamais faire paniquer
    /// l'agent — c'est le même principe que le `DatagramRecv::try_from` du
    /// transport.
    pub fn desencapsuler<'a>(&self, trame: &'a [u8]) -> Option<(SocketAddr, &'a [u8])> {
        if trame.len() < 4 {
            return None;
        }
        let canal = u16::from_be_bytes([trame[0], trame[1]]);
        let longueur = u16::from_be_bytes([trame[2], trame[3]]) as usize;
        if trame.len() < 4 + longueur {
            return None;
        }
        let pair = *self.canaux.get(&canal)?;
        Some((pair, &trame[4..4 + longueur]))
    }
```

Ajouter les deux champs à `struct TurnClient` :

```rust
    /// Canaux liés, du numéro vers le pair.
    canaux: std::collections::HashMap<u16, SocketAddr>,
    /// Prochain numéro à attribuer, dans la plage normative.
    prochain_canal: u16,
```

et à leur initialisation dans `new` :

```rust
            canaux: std::collections::HashMap::new(),
            prochain_canal: CANAL_MIN,
```

- [ ] **Étape 4 : Lancer les tests pour vérifier qu'ils passent**

```bash
cargo test -p agent turn
```

Attendu : SUCCÈS, 17 tests.

- [ ] **Étape 5 : Commit**

```bash
git add agent/src/turn.rs
git commit -m "feat(nat): encapsulation ChannelData et démultiplexage STUN/données"
```

---

## Tâche 6 : Router les paquets du transport

**Fichiers :**
- Modifier : `agent/src/transport.rs`

**Interfaces :**
- Consomme : `TurnClient` (tâches 4, 5).
- Produit : `Session::envoyer(&mut self, &Transmit)` et le traitement des
  paquets venus du serveur TURN. Consommé par la tâche 7.

- [ ] **Étape 1 : Constater ce que `Transmit.source` vaut réellement**

La documentation de str0m dit seulement que la source « peut venir d'un socket
local ou d'un relais TURN ». Router sur une supposition serait le meilleur moyen
de passer une journée à déboguer un silence. On constate d'abord.

Ajouter temporairement, dans la branche `Output::Transmit` de `run`
(`transport.rs:557`) :

```rust
                    tracing::info!(
                        source = %transmit.source,
                        destination = %transmit.destination,
                        "transmit"
                    );
```

Puis, une fois la tâche 7 câblée (candidat relayé ajouté), relancer une session
et relever :

```bash
grep "transmit" /chemin/vers/agent.log | sort -u | head
```

**Noter la valeur exacte de `source` pour les paquets destinés au relais.** Deux
possibilités : l'adresse relayée elle-même, ou l'adresse locale du socket depuis
lequel l'allocation a été faite. Le prédicat de l'étape 3 s'écrit à partir de ce
constat, pas avant.

> Cet ordre est volontairement circulaire : la tâche 7 ajoute le candidat, cette
> étape l'observe. Dérouler la tâche 7 d'abord, revenir ici, puis finir la tâche
> 6. C'est le seul endroit du plan où deux tâches s'entrelacent, et c'est parce
> que le fait à constater n'existe qu'une fois le candidat en place.

- [ ] **Étape 2 : Extraire un point d'envoi unique**

Trois endroits envoient aujourd'hui sur le socket : `transport.rs:561`, `:1116`,
et la réception à `:836`. Les deux premiers deviennent un appel unique. Ajouter
à `impl Session` :

```rust
    /// Point d'émission unique. Route vers le socket direct ou vers le relais
    /// TURN selon ce que str0m indique comme source.
    ///
    /// Une erreur d'envoi transitoire est journalisée et ignorée, jamais
    /// remontée : le pair qui ferme son port ne doit pas terminer la session
    /// (I2 de la revue du jalon 1).
    fn envoyer(&mut self, transmit: &str0m::net::Transmit) {
        let (donnees, destination) = match self.route_relayee(transmit) {
            Some(trame) => (trame, self.turn.as_ref().expect("relais présent").serveur()),
            None => (transmit.contents.to_vec(), transmit.destination),
        };
        if let Err(e) = self.socket.send_to(&donnees, destination) {
            tracing::warn!(erreur = %e, "échec d'envoi UDP, ignoré");
        }
    }

    /// Trame ChannelData à envoyer au serveur TURN, ou `None` si ce paquet
    /// part en direct.
    fn route_relayee(&mut self, transmit: &str0m::net::Transmit) -> Option<Vec<u8>> {
        let turn = self.turn.as_mut()?;
        let allocation = turn.allocation()?;
        // Hypothèse de départ : str0m nomme comme source l'adresse du
        // candidat local employé, donc l'adresse relayée pour un paquet qui
        // doit passer par le relais. **À confirmer par le constat de l'étape
        // 1** — si le journal montre plutôt l'adresse locale du socket, c'est
        // cette comparaison-ci qu'il faut inverser, et rien d'autre.
        if transmit.source != allocation.relayee {
            return None;
        }
        // Le pair peut n'avoir pas encore de canal : le lier à la volée. Le
        // tout premier paquet vers un pair neuf part alors en direct et sera
        // probablement perdu — ICE réémet ses contrôles de connectivité, donc
        // ce n'est pas un trou, seulement un aller-retour de retard.
        if turn.encapsuler(transmit.destination, &transmit.contents).is_none() {
            turn.lier_canal(transmit.destination);
        }
        turn.encapsuler(transmit.destination, &transmit.contents)
    }
```

Remplacer les deux sites d'envoi par `self.envoyer(&transmit);`.

- [ ] **Étape 3 : Traiter les paquets venus du serveur TURN**

Dans la boucle de réception (`transport.rs:836`), juste après
`self.consecutive_recv_errors = 0;` et avant `DatagramRecv::try_from` :

```rust
                    // Paquet venu du serveur TURN : soit un message de
                    // service (réponse d'allocation, 401, 438), soit des
                    // données relayées à désencapsuler avant de les présenter
                    // à str0m comme venant du pair.
                    if self.turn.as_ref().is_some_and(|t| t.serveur() == source_addr) {
                        let recu = &buffer[..n];
                        if crate::turn::est_channel_data(recu) {
                            let turn = self.turn.as_ref().expect("présent, testé juste au-dessus");
                            let Some((pair, charge)) = turn.desencapsuler(recu) else {
                                tracing::debug!("trame ChannelData illisible, ignorée");
                                return Ok(Tick::Continue);
                            };
                            // La charge utile est recopiée : `charge`
                            // emprunte `self.turn`, et `handle_input` a
                            // besoin de `&mut self.rtc`.
                            let charge = charge.to_vec();
                            let allocation_relayee =
                                self.turn.as_ref().and_then(|t| t.allocation()).map(|a| a.relayee);
                            match DatagramRecv::try_from(&charge[..]) {
                                Ok(contents) => {
                                    let receive = Receive {
                                        proto: Protocol::Udp,
                                        source: pair,
                                        // La destination est l'adresse
                                        // RELAYÉE, pas celle du socket local :
                                        // c'est le candidat auquel le pair a
                                        // écrit, et str0m apparie ses paires
                                        // là-dessus.
                                        destination: allocation_relayee
                                            .unwrap_or(self.socket.local_addr()?),
                                        contents,
                                    };
                                    self.rtc
                                        .handle_input(Input::Receive(Instant::now(), receive))
                                        .map_err(|e| anyhow!("handle_input relayé : {e}"))?;
                                }
                                Err(e) => {
                                    tracing::debug!(erreur = %e, "charge relayée non reconnue");
                                }
                            }
                        } else if let Some(turn) = self.turn.as_mut() {
                            if let Err(e) = turn.handle_packet(recu) {
                                tracing::warn!(erreur = %e, "message TURN illisible, ignoré");
                            }
                        }
                        return Ok(Tick::Continue);
                    }
```

- [ ] **Étape 4 : Vider la file sortante du client TURN**

Le client TURN produit des paquets (allocation, rafraîchissement, permissions)
qu'il faut envoyer. Ajouter une branche dans `act_on_timeout`, **avant** la
branche de sondage du socket :

```rust
        // b0) Requête TURN en attente d'émission. Ne mute jamais `Rtc` :
        // c'est un échange avec le serveur de relais, invisible de str0m.
        if let Some(turn) = self.turn.as_mut() {
            turn.avancer(Instant::now());
            if let Some(paquet) = turn.poll_transmit() {
                let serveur = turn.serveur();
                if let Err(e) = self.socket.send_to(&paquet, serveur) {
                    tracing::warn!(erreur = %e, "échec d'envoi vers le serveur TURN, ignoré");
                }
                return Ok(Tick::Continue);
            }
        }
```

- [ ] **Étape 5 : Compiler et vérifier la non-régression locale**

```bash
cargo test -p agent
scripts/build-agent.sh
```

Puis, **sans** configuration TURN (`self.turn` valant `None`), une session
locale ordinaire :

```bash
scripts/run-agent.sh
cd client && npm run verify
```

Attendu : la session s'établit exactement comme avant. Tout ce qui précède est
inerte quand aucun relais n'est configuré — c'est ce que ce contrôle vérifie.

- [ ] **Étape 6 : Retirer l'instrumentation de l'étape 1 et committer**

```bash
git add agent/src/transport.rs
git commit -m "feat(nat): router chaque paquet vers le socket direct ou le relais"
```

---

## Tâche 7 : Candidat relayé et séquence de démarrage

**Fichiers :**
- Modifier : `agent/src/signaling.rs`
- Modifier : `agent/src/main.rs`
- Modifier : `agent/src/transport.rs`

**Interfaces :**
- Consomme : le message `ice-config` du signaling (tâche 2), `TurnClient`
  (tâche 4).
- Produit : un candidat relayé présent dans la réponse SDP.

- [ ] **Étape 1 : Remonter la configuration ICE depuis le signaling**

Dans `agent/src/signaling.rs`, ajouter au `SignalingHandle` :

```rust
    /// Configuration ICE délivrée par le serveur juste après la déclaration
    /// de rôle. `watch` plutôt que `mpsc` : c'est un ÉTAT, dont seule la
    /// dernière valeur compte, et l'appelant doit pouvoir le lire même s'il
    /// arrive après l'émission.
    pub ice_config: watch::Receiver<Option<ConfigIce>>,
```

et le type, dans le même fichier :

```rust
/// Ce que le signaling nous dit du relais à employer.
#[derive(Debug, Clone)]
pub struct ConfigIce {
    /// Adresse du serveur TURN, extraite de l'URL `turn:hôte:port`.
    pub serveur: std::net::SocketAddr,
    pub username: String,
    pub credential: String,
}
```

Dans la boucle de réception, ajouter un bras au `match parsed["type"].as_str()` :

```rust
                Some("ice-config") => {
                    match analyser_config_ice(&parsed) {
                        Some(config) => {
                            tracing::info!(serveur = %config.serveur, "configuration TURN reçue");
                            let _ = ice_tx.send(Some(config));
                        }
                        None => tracing::warn!(
                            "configuration ICE reçue mais inexploitable : session sans relais"
                        ),
                    }
                }
```

et la fonction d'analyse, en bas du fichier :

```rust
/// Extrait la première entrée TURN exploitable d'un message `ice-config`.
///
/// Résout le nom d'hôte : `Candidate::relayed` et le socket UDP veulent une
/// `SocketAddr`, pas une URL. Une résolution qui échoue rend `None` — session
/// sans relais plutôt que session sans démarrage.
fn analyser_config_ice(message: &serde_json::Value) -> Option<ConfigIce> {
    use std::net::ToSocketAddrs;

    let serveurs = message["iceServers"].as_array()?;
    for entree in serveurs {
        let urls = entree["urls"].as_str()?;
        // Forme attendue : `turn:hôte:port`. On ignore les entrées `stun:` —
        // l'adresse réflexive nous vient de la réponse Allocate elle-même.
        let Some(reste) = urls.strip_prefix("turn:") else { continue };
        let Ok(mut adresses) = reste.to_socket_addrs() else { continue };
        let serveur = adresses.next()?;
        return Some(ConfigIce {
            serveur,
            username: entree["username"].as_str()?.to_string(),
            credential: entree["credential"].as_str()?.to_string(),
        });
    }
    None
}
```

Créer le canal dans `run_signaling`, à côté des autres :

```rust
    let (ice_tx, ice_config) = watch::channel::<Option<ConfigIce>>(None);
```

et l'ajouter au `SignalingHandle` rendu.

- [ ] **Étape 2 : Allouer avant de répondre à l'offre**

Dans `agent/src/main.rs`, localiser l'endroit où l'offre est consommée et
`accept_offer` appelé :

```bash
grep -n "accept_offer\|offers.recv" agent/src/main.rs
```

Avant cet appel, construire le client TURN et attendre son allocation :

```rust
    // Le client web n'a pas de trickle ICE : il envoie UNE offre après
    // collecte complète et attend UNE réponse. Le candidat relayé doit donc
    // exister AVANT que la réponse ne soit produite — après, il n'y a plus
    // aucun moyen de le transmettre.
    //
    // Borné à 2 s : très en deçà des 15 s au bout desquelles le client
    // abandonne (`ANSWER_TIMEOUT_MS`, webrtc.ts:27), et suffisant pour les
    // deux aller-retours d'une allocation authentifiée (Allocate nu → 401 →
    // Allocate signé).
    const DELAI_ALLOCATION: Duration = Duration::from_secs(2);

    if let Some(config) = signaling.ice_config.borrow().clone() {
        match session.allouer_relais(config, DELAI_ALLOCATION) {
            Ok(()) => tracing::info!("relais TURN alloué avant la réponse SDP"),
            Err(e) => tracing::warn!(
                erreur = %e,
                "allocation TURN impossible : la session continue sans relais"
            ),
        }
    }
```

- [ ] **Étape 3 : Implémenter l'allocation bloquante**

Dans `impl Session` (`transport.rs`) :

```rust
    /// Alloue un relais TURN et ajoute le candidat correspondant, en bloquant
    /// jusqu'au succès ou jusqu'au délai.
    ///
    /// Bloquer est ici volontaire et borné : sans trickle ICE, le candidat
    /// doit exister avant la réponse SDP (voir l'appelant). Un échec n'est
    /// pas fatal — la session continue avec les candidats hôtes.
    pub fn allouer_relais(
        &mut self,
        config: crate::signaling::ConfigIce,
        delai: Duration,
    ) -> Result<()> {
        let local = self.socket.local_addr()?;
        let mut turn = crate::turn::TurnClient::new(
            config.serveur,
            config.username,
            config.credential,
            Instant::now(),
        );

        let echeance = Instant::now() + delai;
        let mut buffer = vec![0u8; 2000];
        let allocation = loop {
            while let Some(paquet) = turn.poll_transmit() {
                self.socket.send_to(&paquet, config.serveur)?;
            }
            if let Some(a) = turn.allocation() {
                break a;
            }
            if Instant::now() >= echeance {
                anyhow::bail!("aucune allocation TURN obtenue en {:?}", delai);
            }
            match self.socket.recv_from(&mut buffer) {
                Ok((n, source)) if source == config.serveur => {
                    if let Err(e) = turn.handle_packet(&buffer[..n]) {
                        tracing::debug!(erreur = %e, "paquet TURN ignoré pendant l'allocation");
                    }
                }
                // Un datagramme venu d'ailleurs pendant l'allocation est du
                // bruit : le socket n'est pas encore connu du pair.
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(e) => return Err(e).context("réception pendant l'allocation TURN"),
            }
        };

        self.rtc.add_local_candidate(
            Candidate::relayed(allocation.relayee, local, "udp")
                .map_err(|e| anyhow!("candidat relayé invalide : {e}"))?,
        );
        if let Some(reflexive) = allocation.reflexive {
            // La même réponse Allocate porte l'adresse réflexive : un
            // candidat de plus, sans échange supplémentaire.
            match Candidate::server_reflexive(reflexive, local, "udp") {
                Ok(c) => {
                    self.rtc.add_local_candidate(c);
                }
                Err(e) => tracing::warn!(erreur = %e, "candidat réflexif invalide, ignoré"),
            }
        }
        self.turn = Some(turn);
        // `add_local_candidate` mute `Rtc` : drainer avant de rendre la main,
        // comme le fait déjà `Session::new`.
        self.drain_quietly()?;
        Ok(())
    }
```

Ajouter le champ à `struct Session` :

```rust
    /// Client TURN, absent tant qu'aucun relais n'est configuré ou alloué.
    /// Son absence rend tout le chemin relayé inerte.
    turn: Option<crate::turn::TurnClient>,
```

initialisé à `turn: None,`.

> **Vérifier la signature exacte de `Candidate::server_reflexive`** avant
> d'écrire l'appel :
> ```bash
> grep -n -A8 "pub fn server_reflexive" \
>   /root/.cargo/registry/src/index.crates.io-*/is-0.10.0/src/candidate.rs
> ```
> Elle prend peut-être une adresse de base supplémentaire. Employer la forme
> que rend cette commande.

- [ ] **Étape 4 : Vérifier que le candidat relayé apparaît dans la réponse**

```bash
docker compose up -d coturn
scripts/build-agent.sh
scripts/run-agent.sh
```

Connecter le client, puis :

```bash
grep -i "candidate" /chemin/vers/agent.log | grep -i "relay"
```

Attendu : au moins un candidat de type `relay` dans la réponse SDP. S'il n'y en
a pas, relever d'abord si le journal contient « relais TURN alloué avant la
réponse SDP » : sans cette ligne, le problème est l'allocation, pas le candidat.

- [ ] **Étape 5 : Revenir finir la tâche 6**

C'est maintenant que l'étape 1 de la tâche 6 peut être menée : le candidat
relayé existe, donc str0m produit des `Transmit` qui en partent. Relever la
valeur de `source`, écrire le prédicat de `route_relayee`, puis revenir ici.

- [ ] **Étape 6 : Commit**

```bash
git add agent/src/signaling.rs agent/src/main.rs agent/src/transport.rs
git commit -m "feat(nat): allouer le relais avant la réponse SDP, faute de trickle ICE"
```

---

## Tâche 8 : Le client consomme les `iceServers`

**Fichiers :**
- Modifier : `client/src/webrtc.ts`

**Interfaces :**
- Consomme : le message `ice-config` (tâche 2).

- [ ] **Étape 1 : Écrire le test qui échoue**

Dans `client/src/webrtc.test.ts`, ajouter au bloc de tests de
`parseSignalingMessage` :

```typescript
    it('reconnaît la configuration ICE', () => {
        const message = parseSignalingMessage(
            JSON.stringify({
                type: 'ice-config',
                iceServers: [{ urls: 'turn:x:3478', username: 'u', credential: 'c' }],
            }),
        );
        expect(message).toEqual({
            type: 'ice-config',
            iceServers: [{ urls: 'turn:x:3478', username: 'u', credential: 'c' }],
        });
    });
```

- [ ] **Étape 2 : Lancer le test pour vérifier qu'il échoue**

```bash
cd client && npm test -- webrtc
```

Attendu : ÉCHEC — `parseSignalingMessage` rend `undefined` pour ce type inconnu.

- [ ] **Étape 3 : Étendre le type et le parseur**

Dans `client/src/webrtc.ts` :

```typescript
type SignalingMessage =
    | { type: 'answer'; sdp: string }
    | { type: 'error'; reason?: string }
    | { type: 'peer-gone' }
    | { type: 'ice-config'; iceServers: RTCIceServer[] };
```

et dans `parseSignalingMessage`, étendre la garde :

```typescript
    if (type === 'answer' || type === 'error' || type === 'peer-gone' || type === 'ice-config') {
        return parsed as SignalingMessage;
    }
```

- [ ] **Étape 4 : Attendre la configuration avant de créer la connexion**

La `RTCPeerConnection` est construite **avant** l'ouverture du socket
(`webrtc.ts:161`), donc avant que la configuration n'arrive. L'ordre doit
s'inverser : ouvrir le socket, se déclarer, attendre brièvement la
configuration, puis construire la connexion.

Remplacer le début de `connectSession` :

```typescript
export async function connectSession(options: SessionOptions): Promise<SessionHandle> {
    const status = options.onStatus ?? (() => {});

    const socket = new WebSocket(options.signalingUrl);
    await new Promise<void>((resolve, reject) => {
        socket.addEventListener('open', () => resolve(), { once: true });
        socket.addEventListener('error', () => reject(new Error('signaling injoignable')), {
            once: true,
        });
    });
    socket.send(JSON.stringify({ role: 'client', session: options.sessionId }));

    // La configuration ICE arrive juste après la déclaration de rôle, ou
    // jamais si aucun relais n'est déployé. On l'attend brièvement plutôt que
    // de bloquer : une session en réseau local doit continuer à s'établir
    // sans relais, exactement comme avant ce chantier.
    const iceServers = await attendreConfigIce(socket, 2000);
    const pc = new RTCPeerConnection({ iceServers });
```

et supprimer, plus bas, le bloc qui ouvrait le socket et envoyait `role` — il
est désormais en tête. Ajouter la fonction d'attente, à côté de `waitForAnswer` :

```typescript
/// Attend la configuration ICE du signaling, au plus `delaiMs`.
///
/// Rend un tableau VIDE en cas d'absence : c'est le cas normal d'un
/// déploiement sans relais, pas une erreur. Le message est retiré du flux
/// pour ne pas être confondu plus tard avec une réponse SDP.
function attendreConfigIce(socket: WebSocket, delaiMs: number): Promise<RTCIceServer[]> {
    return new Promise((resolve) => {
        const finir = (serveurs: RTCIceServer[]) => {
            socket.removeEventListener('message', onMessage);
            clearTimeout(timer);
            resolve(serveurs);
        };
        const onMessage = (event: MessageEvent) => {
            const message = parseSignalingMessage(String(event.data));
            if (message?.type === 'ice-config') finir(message.iceServers);
        };
        const timer = setTimeout(() => finir([]), delaiMs);
        socket.addEventListener('message', onMessage);
    });
}
```

- [ ] **Étape 5 : Vérifier types et tests**

```bash
cd client && npm run typecheck && npm test
```

Attendu : SUCCÈS des deux. Si un test existant échoue parce qu'il n'émet jamais
d'`ice-config`, c'est attendu : il devra patienter 2 s. Lui faire émettre un
`ice-config` vide plutôt que d'allonger les délais du test.

- [ ] **Étape 6 : Commit**

```bash
git add client/src/webrtc.ts client/src/webrtc.test.ts
git commit -m "feat(client): employer les serveurs ICE délivrés par le signaling"
```

---

## Tâche 9 : Recette de traversée

**Fichiers :**
- Créer : `docs/superpowers/plans/2026-07-29-traversee-nat-resultats.md`

- [ ] **Étape 1 : Non-régression locale, sans relais**

```bash
docker compose stop coturn
scripts/run-agent.sh
cd client && npm run verify
```

Attendu : la session s'établit comme avant le chantier. Tout le chemin TURN doit
être inerte quand aucun relais n'est configuré. **Si cette étape échoue, ne pas
continuer** : la traversée ne doit pas coûter le cas nominal.

- [ ] **Étape 2 : Session relayée de bout en bout**

```bash
docker compose up -d coturn
scripts/run-agent.sh
```

Connecter le client, puis vérifier dans `chrome://webrtc-internals` (ou dans
`getStats()`) la paire de candidats **nominée**. Relever son type des deux
côtés.

Pour **forcer** le passage par le relais et prouver que le chemin fonctionne
vraiment — sans quoi ICE choisira la paire hôte directe et la mesure ne dirait
rien — configurer temporairement le client en `iceTransportPolicy: 'relay'` :

```typescript
    const pc = new RTCPeerConnection({ iceServers, iceTransportPolicy: 'relay' });
```

Attendu : la session s'établit, l'image arrive, la paire nominée est
`relay`/`relay`. **Retirer ce réglage après la mesure** — il n'a pas sa place en
fonctionnement normal, où ICE doit préférer le chemin direct.

- [ ] **Étape 3 : Vérifier que le bail se rafraîchit**

Laisser une session relayée ouverte **plus de 10 minutes** (le bail demandé vaut
600 s, le rafraîchissement tombe à 300 s), puis :

```bash
docker compose logs coturn | grep -i refresh | tail
grep -i "turn" /chemin/vers/agent.log | tail -20
```

Attendu : au moins un rafraîchissement accepté, et la session toujours vivante.
C'est le seul moyen d'exercer réellement le chemin du 438 nonce périmé.

- [ ] **Étape 4 : Passe sur un vrai réseau contraint**

Une session depuis l'extérieur, sur un lien qui n'a pas de route directe vers
l'agent — partage de connexion mobile, ou réseau d'entreprise. Relever le type
de la paire nominée.

Attendu : `relay` au moins d'un côté. C'est la preuve que le chantier remplit
son objet : sans lui, cette session ne s'établissait pas du tout.

- [ ] **Étape 5 : Écrire les résultats**

Créer `docs/superpowers/plans/2026-07-29-traversee-nat-resultats.md` :

- §1 non-régression locale sans relais ;
- §2 session relayée forcée : types de candidats, débit, latence, comparés à la
  session directe — un relais ajoute un aller-retour, le chiffrer plutôt que de
  le supposer ;
- §3 rafraîchissement du bail : durée de la session, nombre de
  rafraîchissements, 438 rencontrés ou non ;
- §4 passe sur réseau réel contraint ;
- §5 limites constatées, dont l'absence de TURN/TCP (spec §11) et ce qu'elle a
  coûté si un réseau testé bloquait l'UDP ;
- §6 ce que la mesure a coûté : outils jetables, faux départs, mesures refaites.

- [ ] **Étape 6 : Commit**

```bash
git add docs/superpowers/plans/2026-07-29-traversee-nat-resultats.md
git commit -m "docs(nat): résultats de la recette de traversée"
```

---

## Fin du volet 2

Avec le volet 1, le chantier C est complet : le pipeline joint le pair depuis un
réseau quelconque, et s'asservit à ce que ce réseau porte.
