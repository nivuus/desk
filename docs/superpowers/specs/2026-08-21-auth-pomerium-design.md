# L'authentification derrière Pomerium — conception

**21 août 2026.** Décidé avec le propriétaire du dépôt, par quatre arbitrages
successifs consignés au § 2.

---

## 1. Le constat qui gouverne

**Le système doit vivre derrière Pomerium** (`/opt/nivuus/Pomerium`), qui
authentifie déjà les humains contre Google et sait poser leur identité sur la
requête relayée. L'authentification par mot de passe de la plateforme fait donc
une seconde fois, moins bien, un travail déjà fait.

**Deux relevés du 21 août 2026 gouvernent la conception, et ils ont été MESURÉS,
non supposés :**

| Relevé | Commande | Ce qu'il établit |
| --- | --- | --- |
| **Rien n'écoute sur 3445** | `ss -ltn \| grep 3445` → vide | La route `app.allanic.me → 127.0.0.1:3445` de Pomerium pointe un backend **mort**. Elle n'est pas prise : elle attend ce service |
| **La plateforme écoute sur `0.0.0.0:8080`** | `ss -ltnp \| grep ':8080 '` → pid 3832468, `tsx src/index.ts` | La garde du § 5.2 refuserait le montage **réel d'aujourd'hui**. Sa rouge est jouable sans rien fabriquer |

⚠️ **La politique de cette route whiteliste déjà `manifest.json`, `.ico` et
`.png` en accès non authentifié** — c'est-à-dire exactement ce que le legs ④
« un `<img src>` ne porte pas d'`Authorization`, le hub n'est pas installable »
réclame. Ce document ne solde PAS ce legs, mais il n'y met aucun obstacle.

🔴 **LE FAIT D'ARCHITECTURE QUI CONTRAINT TOUT LE RESTE : le relais de signaling
sert DEUX pairs de natures différentes.** Le navigateur, que Pomerium sait
authentifier ; et l'agent Windows, qui n'a ni navigateur, ni cookie, ni session
Google, et que Pomerium ne saura **jamais** authentifier. Toute conception qui
remplacerait le jeton interne par l'identité Pomerium coupe l'agent.

---

## 2. Les quatre arbitrages, et ce qu'ils ferment

| # | Question | Décision | Ce qu'elle écarte |
| --- | --- | --- | --- |
| ① | Quelle identité les routes voient-elles ? | **Le courriel lu chez Pomerium**, utilisateur créé au vol | L'utilisateur unique implicite, qui aurait réduit le système à **une seule VM attribuable** (index `vm_un_utilisateur`) |
| ② | À quel point croire l'en-tête ? | **En clair**, adossé à une garde d'écoute | La vérification de signature ES256 du `X-Pomerium-Jwt-Assertion`, et sa dépendance |
| ③ | Comment l'agent atteint-il le relais ? | **Le relais quitte `/` pour `/signal`** | Le contournement de Pomerium par le LAN, et la racine publique |
| ④ | Que devient la machinerie de mots de passe ? | **Dormante derrière un interrupteur**, pas retirée | La suppression franche |

---

## 3. Le pivot : le jeton interne reste, seule sa délivrance change

🔴 **`identite/jeton.ts`, `http/porteur.ts`, `identite/garde.ts`, les cinq
routeurs porteurs et la poignée de main WebSocket NE CHANGENT PAS.** C'est le
cœur de cette conception, et ce n'est pas une économie : le jeton interne est ce
qui authentifie le relais que Pomerium ne peut pas garder (§ 1). Le retirer
couperait l'agent.

Ce qui change est **l'endroit d'où le jeton vient** :

```
AVANT   POST /auth/connexion {email, motdepasse}  ──scrypt──▶  {acces, rafraichissement}
APRÈS   GET  /auth/moi  + X-Pomerium-Claim-Email  ──upsert──▶  {acces}
```

Puis, identiquement dans les deux cas : `Authorization: Bearer <acces>` sur les
routes HTTP, et le champ `jeton` du premier message de la poignée de main
WebSocket (`client/src/webrtc.ts`).

**Le rafraîchissement devient gratuit** : à l'expiration, le client rappelle
`/auth/moi`. Le cookie Pomerium vit `8640h` (`config.yaml`), donc l'appel
réussit sans interaction. C'est pourquoi le mode `pomerium` ne délivre **aucun**
jeton de rafraîchissement — en délivrer un serait tenir une chaîne rotative
anti-rejeu dont plus personne n'a besoin.

---

## 4. Les interfaces

### 4.1 `GET /auth/moi` — la route neuve

**Elle naît dans un fichier neuf, `plateforme/src/http/routes-identite.ts`, et
NON dans `routes-auth.ts`.** Ce dernier pèse **397 lignes** au 21 août 2026
(`wc -l`), et la règle des 500 lignes veut que toute addition substantielle
s'accompagne d'une extraction plutôt que d'une croissance.

En mode `pomerium` :

| Cas | Réponse |
| --- | --- |
| En-tête absent ou vide | `401 {refus:'identite-absente'}` |
| En-tête **répété** (Node rend un tableau) | `401 {refus:'identite-absente'}` |
| Courriel présent | `200 {acces}` — l'utilisateur est créé s'il n'existe pas |

🔴 **AUCUN REPLI SUR UN UTILISATEUR PAR DÉFAUT, dans aucun cas.** Un repli
transformerait « Pomerium n'a pas posé son en-tête » — c'est-à-dire une
mauvaise configuration du proxy — en « tout le monde est administrateur ». La
panne doit être bruyante et refusante.

⚠️ **L'en-tête répété est REFUSÉ, jamais désambiguïsé** : c'est le précédent
littéral de `http/porteur.ts`, dont l'en-tête écrit qu'« en choisir un serait
prendre une décision qu'un attaquant exploite dès que deux couches n'en
prennent pas la même ».

⚠️ **`empreinte_mdp` est `NOT NULL`** (`0001-socle.sql`). L'utilisateur créé au
vol y reçoit un **marqueur inutilisable et reconnaissable** — jamais une chaîne
vide, qui pourrait un jour croiser un vérificateur permissif. Le marqueur
retenu : `pomerium$aucun-mot-de-passe`. Il ne peut correspondre à aucun format
que `identite/mot-de-passe.ts` sait vérifier (`scrypt$N$r$p$sel$empreinte`).

En mode `motdepasse` : **`404`**. La route n'existe pas.

### 4.2 Ce que le mode `pomerium` ferme

`POST /auth/connexion` et `POST /auth/rafraichir` rendent **`404`** en mode
`pomerium` — pas `403`, pas `405`. Un `404` dit « cette route n'existe pas dans
ce montage », ce qui est vrai ; un `403` dirait « elle existe, tu n'y as pas
droit », ce qui inviterait à réessayer.

🔴 **C'EST CE 404 QUI PORTE LE MODE JUSQU'AU CLIENT.** La page est bâtie
statiquement par Vite : elle ne peut lire aucune variable du serveur. Elle
**demande** donc, et n'a aucun mode à connaître. Aucune route de découverte
supplémentaire n'est ajoutée — celle-ci suffit, et une route qui répond déjà à
la question est meilleure qu'une route qui la pose.

---

## 5. La configuration

### 5.1 `PLATEFORME_AUTH`

| | |
| --- | --- |
| Valeurs | `pomerium` (défaut) ou `motdepasse` |
| Valeur inconnue | **LÈVE**, au démarrage |
| Convention | celle de `PLATEFORME_BASE`, **pas** celle de `PLEIN_ECRAN` |

⚠️ **CE N'EST PAS UN ARMEMENT, C'EST UN CHOIX DE MODE**, et c'est pourquoi la
convention `=0 désarme` ne s'applique pas. Le dépôt tient déjà les deux
conventions séparées ; `PLATEFORME_BASE=sqlite|postgres` est le précédent
exact, dans le fichier même où la variable neuve sera lue.

🔴 **UNE VALEUR INCONNUE LÈVE, elle ne retombe pas sur le défaut.** Une coquille
(`pomerium ` avec une espace, `Pomerium`) ferait tourner le mode mot de passe
sous le nom du mode Pomerium, ou l'inverse — et le second sens est une
**ouverture**. `config.ts` lève déjà pour `PLATEFORME_BASE`, pour cette raison
écrite là-bas.

### 5.2 La garde d'écoute — et ce que son nom ne promet pas

En mode `pomerium`, `lireConfig` **refuse de démarrer si `PLATEFORME_HOTE` vaut
`0.0.0.0` ou `::`**.

🔴 **CETTE GARDE EST PLUS ÉTROITE QUE « L'ÉCOUTE EST BORNÉE », ET LE DIRE FAIT
PARTIE DE LA LIVRAISON.** Le contrôle qu'on aimerait — « ce doit être une
adresse de bouclage » — **casserait le déploiement livré** : `docker-compose.
plateforme.yml` pose `PLATEFORME_HOTE: plateforme`, un nom de service Docker
sur un réseau interne sans port publié, qui n'est pas une adresse de bouclage
et qui est pourtant le montage le plus sûr des trois.

Ce qui est décidable est donc : **refuser l'écoute universelle**. Cela attrape
la mauvaise configuration réellement dangereuse, laisse passer les deux
montages légitimes (bouclage, réseau interne Docker), et **ne garantit pas** que
seul Pomerium atteint le port. Cette dernière propriété reste à la charge de
l'exploitant, et elle est nommée au § 8.

🔵 **LA ROUGE EST ACQUISE D'AVANCE** : le service qui tourne aujourd'hui écoute
sur `0.0.0.0:8080` (§ 1). La garde le refusera au premier démarrage, sur le
montage réel, sans rien fabriquer.

---

## 6. Le relais quitte `/` pour `/signal`

**Quatre fichiers, et une recompilation.**

| Fichier | Ce qui change |
| --- | --- |
| `plateforme/src/http/serveur.ts` | la comparaison `chemin === '/'` devient `chemin === '/signal'` |
| `client/src/webrtc.ts` | l'URL de montée |
| `agent/src/signaling.rs` | l'URL de montée — **recompilation sur la VM** |
| `deploiement/nginx.conf` | la table `map $http_upgrade $vers_plateforme` **disparaît** |

🔴 **RUPTURE ASSUMÉE : TOUT AGENT NON RECOMPILÉ CESSE DE SE CONNECTER.** Il n'y
a qu'une VM, ce qui rend le coût acceptable — mais il faut la démarrer
(`virsh start Windows`) et rejouer `scripts/build-agent.sh` dans la même ronde
que le déploiement du service. Un service déplacé sans agent recompilé donne un
navigateur qui se connecte et un bureau qui n'arrive jamais.

✅ **CE DÉPLACEMENT SOLDE UN LEGS DÉCLARÉ.** `deploiement/nginx.conf` l'écrit
lui-même : « L'ALTERNATIVE PROPRE SERAIT DE DÉPLACER LE CHEMIN DU RELAIS (de
`/` vers `/signal`), et elle est écartée pour une raison de PÉRIMÈTRE, pas de
goût : `agent/src/signaling.rs` vise la racine, le sous-bloc P5 n'a pas le droit
de toucher `agent/`. **Legs déclaré** : le jour où `agent/` sera rouvert,
déplacer ce chemin simplifierait ce fichier. » `agent/` est rouvert ici.

⚠️ **`/agent` ne bouge pas.** Il est déjà distinct de la racine, et son
déplacement n'apporterait rien.

---

## 7. Pomerium

**Le nom d'hôte est `app.allanic.me`**, dont la route existe déjà et pointe un
backend mort (§ 1). Ce qui change : la cible, et deux routes ajoutées.

### 7.1 L'adresse d'écoute, et pourquoi ce n'est ni `127.0.0.1` ni `0.0.0.0`

🔴 **CETTE SOUS-SECTION EXISTE PARCE QUE LA PREMIÈRE RÉDACTION DE CE DOCUMENT
SE CONTREDISAIT**, et la contradiction est instructive : elle faisait router
Pomerium vers `127.0.0.1:8080` **et** l'agent vers `192.168.3.1:8080`, ce qu'un
processus à un seul socket d'écoute ne peut pas servir — sauf à écouter
`0.0.0.0`, que le § 5.2 refuse. Trois contraintes, dont deux seulement étaient
regardées à la fois.

**Les trois contraintes, ensemble :**

| Qui | D'où il parle | Ce qu'il exige |
| --- | --- | --- |
| Pomerium | l'hôte, en `network_mode: host` | atteint **n'importe quelle** adresse de l'hôte |
| L'agent | la VM, `192.168.3.2` | atteint `192.168.3.1`, **jamais** `127.0.0.1` |
| La garde du § 5.2 | — | refuse `0.0.0.0` et `::` |

**La seule valeur qui les satisfait toutes les trois : `PLATEFORME_HOTE=192.168.3.1`**,
une adresse du pont libvirt de l'hôte (relevé le 21 août 2026 par `ip -4 addr`).
Pomerium l'atteint parce qu'il partage la pile réseau de l'hôte ; l'agent
l'atteint parce que c'est la passerelle de son propre réseau ; et la garde
l'accepte parce que ce n'est pas une écoute universelle.

⚠️ **CE N'EST PAS `127.0.0.1`, ET LA DIFFÉRENCE A UN COÛT NOMMÉ** : le port est
joignable depuis le réseau `192.168.3.0/24`, donc depuis la VM Windows. C'est
exactement le périmètre que le § 9 déclare, et il n'y a pas de configuration à
un seul socket qui le réduise davantage tout en gardant l'agent connecté.

⚠️ **Le déploiement Docker ne change pas** : il pose `PLATEFORME_HOTE: plateforme`
sans port publié, et la garde l'accepte tout autant. Cette sous-section décrit
le montage **hôte**, celui qui tourne aujourd'hui.

### 7.2 Le bloc de routes

```yaml
  # La page et l'API : AUTHENTIFIÉES, avec l'identité relayée au service.
  - from: https://app.allanic.me
    to: http://192.168.3.1:8080
    pass_identity_headers: true
    policy:
      - allow:
          or:
            - email:
                is: maxime.g.allanic@gmail.com
            - http_path:
                ends_with: manifest.json
            - http_path:
                ends_with: .ico
            - http_path:
                ends_with: .png

  # Le relais de signaling : PUBLIC pour Pomerium, gardé par le jeton interne.
  - from: https://app.allanic.me
    to: http://192.168.3.1:8080
    prefix: /signal
    allow_websockets: true
    allow_public_unauthenticated_access: true

  # Le canal d'enrôlement des agents : idem, gardé par le secret d'enrôlement.
  - from: https://app.allanic.me
    to: http://192.168.3.1:8080
    prefix: /agent
    allow_websockets: true
    allow_public_unauthenticated_access: true
```

🔴 **`allow_public_unauthenticated_access` SUR LES DEUX WEBSOCKETS N'OUVRE
RIEN**, et c'est le point le plus facile à mal lire de tout ce document. Ces
deux chemins gardent **intégralement** l'authentification interne qu'ils ont
aujourd'hui : le relais exige un jeton signé de type `utilisateur` ou `agent`
(`identite/garde.ts`), le canal exige le secret d'enrôlement. Pomerium n'y
ajoutait rien hier — il n'y retire rien demain. Ce qui est « public » est
l'accès au **port**, jamais l'accès à une session.

⚠️ **L'agent emprunte le chemin DIRECT**, `ws://192.168.3.1:8080/signal` : c'est
la valeur que `SIGNALING_URL` porte déjà à un chemin près, et elle évite le
repli en épingle qu'exigerait `wss://app.allanic.me/signal` depuis le LAN. La
route `/signal` de Pomerium n'est donc **pas** pour l'agent : elle est pour le
**navigateur**, dont la page est servie par Pomerium et qui doit ouvrir son
WebSocket sur la même origine.

🔴 **`config.yaml` EST APPLIQUÉ PAR NOUS, SUR AUTORISATION EXPLICITE DU
PROPRIÉTAIRE (21 août 2026), ET IL FRONTE SEPT SERVICES** — Home Assistant,
Grocy, MediaManager, Sunshine, deux services `personas`, le site racine. La
séquence est donc : **copie datée d'abord** (`config.yaml.bak-<date>`, geste
que ce répertoire pratique déjà trois fois), édition, puis contrôle que les
sept autres routes répondent encore. Une régression sur Home Assistant serait
un dommage causé à un système tiers par un chantier qui ne le concerne pas.

---

## 8. Les critères, et la rouge de chacun

🔴 **UN CONTRÔLE QU'ON N'A JAMAIS VU ROUGE N'EST PAS UN CONTRÔLE.** Chaque
critère porte donc l'état qu'il doit dénoncer, et cet état doit être **provoqué
puis observé**, jamais raisonné.

| # | Le critère | Sa ROUGE |
| --- | --- | --- |
| ① | En mode `pomerium`, une requête portant `X-Pomerium-Claim-Email` reçoit un jeton et atteint `/session` | **La même requête SANS l'en-tête** rend `401 identite-absente` — et non un jeton d'un utilisateur par défaut |
| ② | En mode `motdepasse`, le comportement d'aujourd'hui est **strictement** inchangé | **Une requête portant un en-tête forgé** est ignorée : le formulaire reste seul chemin |
| ③ | `GET /auth/moi` rend `404` en mode `motdepasse`, et `/auth/connexion` rend `404` en mode `pomerium` | **Les deux sens**, jamais un seul : un seul sens laisserait l'autre route vivante dans le mauvais mode |
| ④ | Une valeur inconnue de `PLATEFORME_AUTH` **empêche le démarrage** | `PLATEFORME_AUTH=Pomerium` (majuscule) doit lever, **pas** retomber sur le défaut |
| ⑤ | `PLATEFORME_HOTE=0.0.0.0` empêche le démarrage en mode `pomerium` | Elle le **permet** en mode `motdepasse` : la garde est liée au mode, pas universelle |
| ⑥ | Le relais répond sur `/signal` | **Il ne répond plus sur `/`** — sans quoi le déplacement serait une addition, pas un déplacement, et l'ancienne porte resterait ouverte |
| ⑦ | Dans un **navigateur réel** derrière Pomerium : la page ne montre jamais le formulaire, et le bureau s'affiche | L'agent **non recompilé** doit produire un échec **visible**, jamais un silence |

⚠️ **LE CRITÈRE ⑦ EST LE SEUL QUI JUGE LE PRODUIT**, et aucun test Node ne peut
le voir. `client/src/adresse-plateforme.ts` porte déjà la démonstration de cette
classe : le déploiement de P5 aurait été « livré non fonctionnel ET VERT ».

---

## 9. Ce que ce document N'ÉTABLIT PAS

- 🔴 **L'en-tête reste falsifiable par quiconque atteint le port du service.**
  La garde du § 5.2 refuse l'écoute universelle ; elle ne prouve pas que seul
  Pomerium parle au port. Sur le montage direct de l'agent (§ 7), **la VM
  Windows est dans ce périmètre** : un agent compromis pourrait se déclarer
  n'importe quel courriel sur les routes HTTP. C'est le coût de l'arbitrage ②,
  et il est nommé plutôt que découvert.
- ⚠️ **Aucune mesure de latence, ni avant ni après.** Le legs « la latence de
  bout en bout n'a jamais été mesurée, par aucun sous-bloc, depuis D1 » reste
  entier.
- ⚠️ **Le mode `motdepasse` ne sera plus jamais exercé en production.** C'est le
  coût déclaré de l'arbitrage ④ : deux chemins vivants, dont un que personne ne
  parcourt. Seuls ses tests le tiennent en vie.
- ⚠️ **Le legs ④ « le hub n'est pas installable » n'est pas soldé.** La
  politique Pomerium whiteliste les trois extensions nécessaires, ce qui lève
  l'obstacle côté proxy ; l'`<img src>` sans `Authorization` reste entier côté
  service.

---

## 10. Ce que ce document lègue

- **La vérification de signature du `X-Pomerium-Jwt-Assertion`** — écartée par
  l'arbitrage ②. Condition de réouverture : le jour où le port du service
  cesse d'être joignable seulement par des pairs de confiance.
- **Le retrait franc de la machinerie de mots de passe** — écarté par
  l'arbitrage ④. Condition de réouverture : le jour où le mode `motdepasse`
  coûte plus à tenir qu'il ne rapporte.
- **`SEUIL_INJOIGNABLE_MS`, `PERIODE_BATTEMENT` et les paramètres `scrypt`**
  restent non calibrés, comme avant ce document.
