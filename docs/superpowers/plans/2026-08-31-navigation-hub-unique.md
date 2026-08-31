# Navigation : le hub devient la SEULE surface — plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Le hub, servi à `/`, devient la seule surface que l'utilisateur atteint : il tient la session de contrôle, montre ses fenêtres et son pont fichiers, et valide la fraîcheur de son jeton avant chaque usage ; `shell.html` devient une redirection permanente.

**Architecture:** Un onglet est élu **porteur** par Web Locks et lui seul ouvre le WebSocket en rôle `client` (ce rôle est exclusif côté plateforme) ; les autres onglets reçoivent l'état par `BroadcastChannel` et n'ouvrent aucun socket. Le balisage et la logique du bureau migrent de `shell.html`/`shell-page.ts` vers `hub.html` et un nouveau répertoire `client/src/bureau/`, la règle métier `shell.ts` étant réutilisée **telle quelle**. La fraîcheur du jeton devient un point d'entrée unique dans `jeton.ts`, appelé avant chaque usage.

**Tech Stack:** TypeScript, Vite, Vitest (client) ; Node + TypeScript (plateforme) ; Web Locks API, BroadcastChannel.

**Spec:** [`docs/superpowers/specs/2026-08-31-navigation-hub-unique-design.md`](../specs/2026-08-31-navigation-hub-unique-design.md)

## Global Constraints

- **Français partout** : noms de symboles, commentaires, messages d'interface, messages de commit (ces derniers **sans accents**, convention du dépôt).
- **500 lignes maximum par fichier de code source.** Vérifier avec la commande de `CLAUDE.md`, **relancée**, jamais un chiffre recopié. **Extraire, jamais comprimer.**
- **Deux commandes de test, jamais une** : `cd client && npx vitest run` **puis** `cd client && npx vitest run --dir ../proto`. La racine Vitest est `client/`.
- **`tsc --noEmit` est une étape distincte et obligatoire** : Vitest transpile sans vérifier les types.
- **Chaque contrôle neuf est vu ROUGE avant d'être vu VERT**, par mutation ciblée du produit, restaurée depuis une **copie nommée** (`cp fichier /tmp/…`), **jamais** par `git checkout --` qui restaure HEAD.
- **Ne jamais `git add -A`** : nommer les fichiers. Un pathspec de répertoire ne prend pas le module homonyme (`src/bureau` laisse `src/bureau.ts`).
- **`expect(x).toBe(y, 'message')` est silencieusement ignoré** par Vitest ; ne jamais l'écrire.
- **Trois assertions dans un test ne prouvent que la première** : `expect` s'arrête au premier échec. Une assertion qui compte doit être seule ou première.
- Messages de commit terminés par `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`.

## Structure des fichiers

| Fichier | État | Responsabilité |
| --- | --- | --- |
| `client/src/jeton.ts` | modifié | + `assurerAccesFrais` ; `assurerAcces` retiré |
| `client/src/hub/cartes.ts` | **créé** | le balisage d'une carte d'application (extrait de `page.ts`) |
| `plateforme/src/signaling/relais.ts` | modifié | + `motif: 'role-occupe'` sur le refus d'appariement |
| `client/src/bureau/porteur.ts` | **créé** | élection Web Locks, classement du refus, validation de l'état diffusé — **pur, testé** |
| `client/src/bureau/porteur-dom.ts` | **créé** | câblage : socket, `BroadcastChannel`, DOM |
| `client/src/bureau/fenetres-dom.ts` | **créé** | la liste « Mes fenêtres » (extrait de `shell-page.ts`) |
| `client/src/bureau/fichiers-dom.ts` | **créé** | le pont ProjFS et ses quatre boutons (extrait de `shell-page.ts`) |
| `client/src/bureau/bureau.css` | **déplacé** depuis `client/src/shell.css` | la peinture des sections du bureau |
| `client/hub.html` | modifié | + les deux sections et le `<template>` |
| `client/shell.html` | **vidé** | redirection vers `/`, aucune UI |
| `client/src/shell-page.ts` | **vidé** | la redirection, ~30 lignes |
| `client/src/shell.ts` | **INCHANGÉ** | la règle du bureau, déjà pure et testée |
| `client/src/hub/manifeste.ts` | modifié | `start_url` → racine, `id` figé |
| `client/src/connexion.ts` | modifié | défaut de `suite` |
| `client/outils/classes-employees.mjs` | modifié | `SURFACES_PRODUIT` |

**Ordre et invariant d'exécution :** à la fin de **chaque** tâche, le produit fonctionne. `shell.html` reste pleinement fonctionnel jusqu'à la tâche 9 ; les modules des tâches 6 et 7 sont extraits **depuis** `shell-page.ts`, qui les emploie aussitôt, de sorte qu'aucun code n'est dupliqué entre les deux surfaces.

---

### Task 1: La fraîcheur du jeton — `assurerAccesFrais`

**Files:**
- Modify: `client/src/jeton.ts` (retirer `assurerAcces`, ajouter `assurerAccesFrais`)
- Modify: `client/src/hub/page.ts:357` (le seul appelant)
- Test: `client/src/jeton.test.ts`

**Interfaces:**
- Consumes: `jetonAcces`, `expireAvant`, `poserAcces`, `vider`, `accesParPomerium`, `rafraichirSiNecessaire`, `Coffre`, `AppelAuthMoi` (tous déjà dans `jeton.ts`)
- Produces:
  ```ts
  export const MARGE_FRAICHEUR_MS = 30_000;
  export async function assurerAccesFrais(
      coffre: Coffre,
      base: string,
      appelAuthMoi: AppelAuthMoi,
      maintenant: number,
      appelRafraichissement: (corps: unknown) => Promise<{ acces: string; rafraichissement: string } | undefined>,
      margeMs?: number,
  ): Promise<string | undefined>;
  ```

- [ ] **Step 1: Écrire les tests qui échouent**

Ajouter à la fin de `client/src/jeton.test.ts` (le fichier a déjà un `coffreFactice` — le réutiliser s'il existe, sinon écrire celui-ci) :

```ts
describe('assurerAccesFrais', () => {
    /// Un jeton dont `exp` vaut `expMs`. La signature n'est pas vérifiée par
    /// le navigateur (voir `expireAvant`), donc un en-tête et une signature
    /// factices suffisent — c'est ce que font déjà les tests d'`expireAvant`.
    function jetonExpirantA(expMs: number): string {
        const charge = btoa(JSON.stringify({ exp: expMs })).replace(/=+$/, '');
        return `x.${charge}.y`;
    }

    function coffreAvec(entrees: Record<string, string>): Coffre {
        const carte = new Map(Object.entries(entrees));
        return {
            getItem: (c) => carte.get(c) ?? null,
            setItem: (c, v) => void carte.set(c, v),
            removeItem: (c) => void carte.delete(c),
        };
    }

    it('un jeton frais est rendu SANS aucun appel reseau', async () => {
        const coffre = coffreAvec({ [CLE_ACCES]: jetonExpirantA(100_000) });
        let appels = 0;
        const acces = await assurerAccesFrais(
            coffre,
            'https://h',
            async () => { appels += 1; return { ok: false, json: async () => ({}) }; },
            0,
            async () => { appels += 1; return undefined; },
        );
        // 🔴 LE ZERO D'APPELS EST LE SUJET DU TEST, ET IL EST SEUL :
        // `expect` s'arrete au premier echec, donc une assertion qui compte
        // ne se place jamais en seconde position.
        expect(appels).toBe(0);
        expect(acces).toBe(jetonExpirantA(100_000));
    });

    it('un jeton qui expire DANS LA MARGE est traite comme perime', async () => {
        // `exp` = 20 s, marge = 30 s, maintenant = 0 : encore valide a
        // l'instant meme, deja perime au sens de la marge.
        const coffre = coffreAvec({ [CLE_ACCES]: jetonExpirantA(20_000) });
        const acces = await assurerAccesFrais(
            coffre,
            'https://h',
            async () => ({ ok: true, json: async () => ({ acces: 'FRAIS' }) }),
            0,
            async () => undefined,
        );
        expect(acces).toBe('FRAIS');
    });

    it('un jeton perime avec rafraichissement passe par le rafraichissement, PAS par Pomerium', async () => {
        const coffre = coffreAvec({
            [CLE_ACCES]: jetonExpirantA(0),
            [CLE_RAFRAICHISSEMENT]: 'R',
        });
        let pomerium = 0;
        const acces = await assurerAccesFrais(
            coffre,
            'https://h',
            async () => { pomerium += 1; return { ok: true, json: async () => ({ acces: 'PAR-POMERIUM' }) }; },
            10_000,
            async () => ({ acces: jetonExpirantA(999_000), rafraichissement: 'R2' }),
        );
        expect(pomerium).toBe(0);
        expect(acces).toBe(jetonExpirantA(999_000));
    });

    it('sans jeton de rafraichissement, Pomerium prend le relais et le jeton est POSE', async () => {
        const coffre = coffreAvec({ [CLE_ACCES]: jetonExpirantA(0) });
        const acces = await assurerAccesFrais(
            coffre,
            'https://h',
            async () => ({ ok: true, json: async () => ({ acces: 'FRAIS' }) }),
            10_000,
            async () => undefined,
        );
        expect(acces).toBe('FRAIS');
        // Pose, sinon le rechargement suivant repaierait l'aller-retour.
        expect(coffre.getItem(CLE_ACCES)).toBe('FRAIS');
    });

    it('les deux voies echouent : rend undefined ET vide le coffre', async () => {
        const coffre = coffreAvec({ [CLE_ACCES]: jetonExpirantA(0) });
        const acces = await assurerAccesFrais(
            coffre,
            'https://h',
            async () => ({ ok: false, json: async () => ({}) }),
            10_000,
            async () => undefined,
        );
        expect(acces).toBeUndefined();
        // 🔴 LE COFFRE EST VIDE, ET C'EST LE POINT : un acces perime laisse
        // en place ferait echouer la poignee de main plus tard, ailleurs, sur
        // un refus que rien ne relierait a ici.
        expect(coffre.getItem(CLE_ACCES)).toBeNull();
    });

    it('un coffre VIDE va directement a Pomerium', async () => {
        const coffre = coffreAvec({});
        const acces = await assurerAccesFrais(
            coffre,
            'https://h',
            async () => ({ ok: true, json: async () => ({ acces: 'FRAIS' }) }),
            0,
            async () => undefined,
        );
        expect(acces).toBe('FRAIS');
    });
});
```

Ajouter `CLE_RAFRAICHISSEMENT` et `assurerAccesFrais` aux imports du fichier de test si absents.

- [ ] **Step 2: Lancer les tests pour vérifier qu'ils échouent**

```bash
cd client && npx vitest run src/jeton.test.ts
```
Attendu : ÉCHEC — `assurerAccesFrais is not a function` (ou une erreur d'import).

- [ ] **Step 3: Écrire l'implémentation**

Dans `client/src/jeton.ts`, **remplacer** la fonction `assurerAcces` (et son commentaire d'en-tête) par :

```ts
/// La marge de fraîcheur : un jeton qui expire dans moins que cela est
/// traité comme périmé.
///
/// ⚠️ **CONSTANTE NON CALIBRÉE, ET DÉCLARÉE COMME TELLE** — comme les
/// quarante autres de ce dépôt (`CLAUDE.md`, « aucune constante n'est
/// calibrée »). Elle vaut assez pour qu'un appel parti avec un jeton valide
/// n'arrive pas expiré, sans forcer un aller-retour à chaque geste.
export const MARGE_FRAICHEUR_MS = 30_000;

/// Assure qu'un jeton d'accès **UTILISABLE** est disponible, en l'obtenant
/// si besoin.
///
/// 🔴 **CE QUI LA DISTINGUE D'`assurerAcces`, QU'ELLE REMPLACE : elle regarde
/// si le jeton du coffre est PÉRIMÉ.** `assurerAcces` rendait le contenu du
/// coffre dès qu'il n'était pas vide — un jeton expiré était donc rendu tel
/// quel, et chaque appel échouait ensuite sans que rien ne relie l'échec à
/// l'expiration. C'est la seconde moitié de la demande du 31 août 2026
/// (« si je vais sur hub.html, ça valide et rafraîchit ma connexion »).
///
/// Quatre étapes, dans cet ordre, chacune tentée seulement si la précédente
/// échoue :
///   ① le coffre porte un jeton encore frais à `margeMs` près → le rendre,
///      **sans aucun réseau** : un aller-retour à chaque geste serait un coût
///      pour un cas qui n'en a pas besoin ;
///   ② `rafraichirSiNecessaire` — le chemin du mode `motdepasse` ;
///   ③ `accesParPomerium` → `GET /auth/moi` — le mode `pomerium`, celui de
///      la production ;
///   ④ `undefined`, **coffre vidé** : à l'appelant de renvoyer vers l'écran
///      de connexion.
///
/// 🔴 **ELLE NE VÉRIFIE AUCUNE SIGNATURE**, et `expireAvant` le dit déjà : le
/// navigateur n'a pas le secret. Ce qu'on évite ici est un aller-retour
/// inutile et un échec inexpliqué, jamais une décision d'autorisation —
/// celle-ci reste au service, sur chaque poignée de main.
export async function assurerAccesFrais(
    coffre: Coffre,
    base: string,
    appelAuthMoi: AppelAuthMoi,
    maintenant: number,
    appelRafraichissement: (
        corps: unknown,
    ) => Promise<{ acces: string; rafraichissement: string } | undefined>,
    margeMs: number = MARGE_FRAICHEUR_MS,
): Promise<string | undefined> {
    // ① et ② : `rafraichirSiNecessaire` porte DÉJÀ les deux, et il est testé.
    // Le réécrire ici en produirait une seconde version à tenir d'accord.
    if (await rafraichirSiNecessaire(coffre, maintenant, margeMs, appelRafraichissement)) {
        return jetonAcces(coffre);
    }
    // ⚠️ `rafraichirSiNecessaire` a VIDÉ le coffre en rendant `false` : il n'y
    // a plus rien à présenter, et c'est bien l'état voulu si ③ échoue aussi.
    const frais = await accesParPomerium(base, appelAuthMoi);
    if (frais === undefined) return undefined;
    poserAcces(coffre, frais);
    return frais;
}
```

- [ ] **Step 4: Lancer les tests pour vérifier qu'ils passent**

```bash
cd client && npx vitest run src/jeton.test.ts
```
Attendu : PASS. Les tests existants d'`assurerAcces` échoueront à l'import — **les supprimer** (trois `it`, autour de `jeton.test.ts:287-308`) : la fonction n'existe plus.

- [ ] **Step 5: Voir la rouge du contrôle qui compte**

Le test qui ne doit jamais pouvoir passer par accident est « un jeton frais est rendu SANS aucun appel réseau ». L'éprouver :

```bash
cp client/src/jeton.ts /tmp/jeton.ts.copie
# Muter : rendre la marge enorme, ce qui perime tout jeton
sed -i 's/export const MARGE_FRAICHEUR_MS = 30_000;/export const MARGE_FRAICHEUR_MS = 99_000_000;/' client/src/jeton.ts
cd client && npx vitest run src/jeton.test.ts   # ATTENDU : ROUGE sur `appels` = 1
cd .. && cp /tmp/jeton.ts.copie client/src/jeton.ts
cd client && npx vitest run src/jeton.test.ts   # ATTENDU : VERT
```
**Lire QUELLE assertion a rougi** : ce doit être `expected 1 to be 0` sur `appels`, pas une autre. Une rouge qui rougit pour la mauvaise raison ne prouve rien.

- [ ] **Step 6: Brancher le seul appelant**

Dans `client/src/hub/page.ts`, remplacer l'import `assurerAcces` par `assurerAccesFrais`, et le corps de `demarrer()` :

```ts
async function demarrer(): Promise<void> {
    dire('neutre', 'identification…');
    const acces = await jetonFrais();
    if (acces === undefined) {
        const suite = `/${window.location.search}`;
        window.location.href = `connexion.html?suite=${encodeURIComponent(suite)}`;
        return;
    }
    deps = { base, jeton: acces, fetch: window.fetch.bind(window) };
    dire('neutre', '');
    await peupler();
}

/// 🔴 **APPELÉE AVANT CHAQUE USAGE, ET C'EST LE POINT DE LA DÉCISION DU
/// 31 AOÛT 2026.** Un test local d'expiration, un appel réseau seulement s'il
/// est périmé : le chargement, chaque lancement et chaque lecture d'icône
/// passent par ici, sans aucune minuterie à calibrer.
async function jetonFrais(): Promise<string | undefined> {
    const acces = await assurerAccesFrais(
        window.localStorage,
        base,
        window.fetch.bind(window),
        Date.now(),
        async (corps) => {
            const reponse = await fetch(`${base}/auth/rafraichir`, {
                method: 'POST',
                headers: { 'content-type': 'application/json' },
                body: JSON.stringify(corps),
            });
            if (!reponse.ok) return undefined;
            return (await reponse.json()) as { acces: string; rafraichissement: string };
        },
    );
    if (acces !== undefined) deps = { ...deps, jeton: acces };
    return acces;
}
```

Puis, dans le gestionnaire de clic du bouton « Lancer » (`hub/page.ts`, autour de la l. 215), rafraîchir **avant** l'appel réseau — mais **jamais avant** `ouvrirLeBureau`, qui doit rester dans l'activation du clic :

```ts
        const bureau = ouvrirLeBureau({ ouvrir: (url, nom) => window.open(url, nom) });
        dire('neutre', `Lancement de ${application.nom}…`);
        void jetonFrais().then((frais) => {
            if (frais === undefined) {
                dire('danger', 'Votre session a expiré. Rechargez la page pour vous reconnecter.');
                return;
            }
            return lancerApplication(application.id, deps).then((issue) => {
                // Le corps existant, INCHANGÉ : le bandeau de succès, et
                // le bandeau de danger quand le lancement est refusé. Le
                // relire dans `hub/page.ts` plutôt que de le retaper — il
                // porte deux commentaires 🔴 qui expliquent pourquoi le
                // lancement a lieu même si l'ouverture a échoué.
                //
                // ⚠️ SEULE LA MENTION « Employez « Mon bureau » en haut de
                // page » devra partir, en tâche 8 : le lien disparaît, et une
                // consigne qui désigne un bouton absent est pire qu'aucune.
            });
        });
```

⚠️ **`ouvrirLeBureau` reste la PREMIÈRE ligne du gestionnaire** : un `await` intercalé consommerait l'activation transitoire du clic et l'ouverture redeviendrait une pop-up bloquable. Le fichier le dit déjà ; ce plan ne le déplace pas.

- [ ] **Step 7: Vérifier les types et la suite entière**

```bash
cd client && npx tsc --noEmit && npx vitest run && npx vitest run --dir ../proto
```
Attendu : aucune erreur, tous les tests verts.

- [ ] **Step 8: Commit**

```bash
git add client/src/jeton.ts client/src/jeton.test.ts client/src/hub/page.ts
git commit -m "correction(navigation) : le hub verifie que son jeton n est pas PERIME

assurerAcces rendait le contenu du coffre des qu il n etait pas vide : un
jeton expire etait donc rendu tel quel, et chaque appel echouait ensuite sans
que rien ne relie l echec a l expiration. assurerAccesFrais le remplace et
teste la fraicheur (expireAvant, deja ecrit et teste), puis rafraichit par le
chemin du mode motdepasse (rafraichirSiNecessaire, ecrit, teste, et jusqu ici
appele par PERSONNE), puis par Pomerium.

Appele avant chaque usage : chargement et lancement. Aucune minuterie a
calibrer -- ce depot n a calibre aucune de ses constantes, et MARGE_FRAICHEUR_MS
est declaree non calibree.

Le controle qui compte -- zero appel reseau sur un jeton frais -- a ete VU
ROUGE par mutation de la marge, et l assertion qui a rougi est bien celle-la.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 2: Extraction préalable — `hub/cartes.ts`

**Files:**
- Create: `client/src/hub/cartes.ts`
- Modify: `client/hub.html` (le `<template>` d'une carte)
- Modify: `client/src/hub/page.ts` (retirer la fonction `entree`)

**Interfaces:**
- Consumes: `ApplicationListee` (`hub/catalogue.ts`)
- Produces:
  ```ts
  export interface DepsCarte {
      modele: HTMLTemplateElement;
      /// L'URL absolue de l'icône, ou `null` : le calcul de la base reste
      /// chez l'appelant, qui seul la connaît.
      urlIcone: string | null;
      lancer(): void;
      installer(): void;
  }
  export function batirCarte(application: ApplicationListee, deps: DepsCarte): DocumentFragment;
  ```

🔴 **AUCUN TEST NEUF, ET AUCUNE DÉPENDANCE `jsdom` — ruling R2 du pré-vol.**
`client/` n'a **ni jsdom ni happy-dom**, et ce n'est pas un manque : c'est une
convention écrite (`accent-dom.test.ts` : « les dépendances sont injectées, et
c'est ce qui rend ce module éprouvable là où `main.ts` ne l'est pas »). Or
cloner un `<template>` et y poser du texte ne **décide** rien — au critère du
dépôt, « une condition est une RÈGLE si la changer change ce que le produit
DÉCIDE ; elle est du CÂBLAGE si elle ne fait que router une décision déjà prise
ailleurs ». `cartes.ts` est du câblage, et il le **déclare**, comme
`hub/page.ts` et `shell-page.ts` le déclarent déjà pour eux-mêmes.

**Ce qui le garde, à défaut d'un test :** `tsc --noEmit`, la suite existante,
`npm run design:verifier` (le contrôle §7.9, que le passage au `<template>`
rend *plus* strict) et `npm run build`.

**Pourquoi cette tâche est AVANT :** `hub/page.ts` est à **373** lignes
(relevé du 31 août 2026 — le revérifier, ne pas le croire) et va absorber deux
sections. La règle du dépôt est « extraire, jamais comprimer », **dans une
tâche dédiée jouée avant celle qui ajoute** : la marge regagnée par une
extraction se reperd si on la traite comme acquise, ce qui a été payé six fois.

- [ ] **Step 1: Relever la taille AVANT**

```bash
cd /home/mallanic/Projects/Nivuus/packages/desk
wc -l client/src/hub/page.ts
```
Noter le nombre. Il servira au commit.

- [ ] **Step 2: Poser le `<template>` d'une carte dans `hub.html`**

Les classes ci-dessous sont celles que `hub/page.ts::entree` pose
**aujourd'hui**, relevées le 31 août 2026 — ne rien inventer : une classe
inédite serait employée sans être déclarée, et `design:verifier` la
dénoncerait.

Ajouter avant `<script type="module" …>` dans `client/hub.html` :

```html
        <!--
          LE BALISAGE D'UNE CARTE D'APPLICATION VIT ICI, PAS DANS LE TYPESCRIPT.
          `hub/cartes.ts` clone ce modèle et le remplit.

          Deux raisons, la première décide : ① toutes les classes restent dans
          le HTML, donc dans le périmètre le plus simple du contrôle §7.9 —
          celui qui ne dépend d'aucune analyse de TypeScript ; ② le balisage
          d'une carte se lit à l'endroit où l'on regarde une page. C'est
          exactement la convention que `shell.html` applique déjà à
          `#modele-fenetre`.

          🔴 `hub__icone--absente` EST DÉCLARÉE ICI, SUR UN ÉLÉMENT QUI NE LA
          PORTE PAS : le script l'AJOUTE quand l'application n'a pas d'icône,
          par `classList.add` d'un LITTÉRAL. Avant cette extraction elle vivait
          dans un ternaire (`className = application.icone === null ? … : …`),
          donc dans une valeur CALCULÉE — invisible au contrôle §7.9, qui ne
          voit que les littéraux. L'extraction est l'occasion de ne pas
          reconduire ce trou (ruling R3).

          ⚠️ LES CROCHETS SONT DES ATTRIBUTS `data-*`, PAS DES CLASSES. Une
          classe sert à peindre ; s'en servir aussi comme point d'accroche du
          script rendrait tout renommage visuel capable de casser le câblage
          en silence.
        -->
        <template id="modele-application">
            <li class="carte hub__entree">
                <img class="hub__icone" data-icone alt="" />
                <div class="hub__corps">
                    <h2 class="hub__nom" data-nom></h2>
                    <p class="hub__boutons">
                        <button type="button" class="bouton bouton--principal" data-lancer>
                            Lancer
                        </button>
                        <button type="button" class="bouton bouton--discret" data-installer>
                            Installer
                        </button>
                    </p>
                </div>
            </li>
        </template>
```

⚠️ **Vérifier la liste des classes contre le code AVANT d'écrire**, plutôt que
contre ce plan :
```bash
grep -n "className = \|classList.add" client/src/hub/page.ts
```
Attendu : `carte hub__entree`, `hub__icone` / `hub__icone--absente`,
`hub__corps`, `hub__nom`, `hub__boutons`, `bouton bouton--principal`,
`bouton bouton--discret`. Si le relevé diffère, **c'est le relevé qui a
raison**.

- [ ] **Step 3: Créer `hub/cartes.ts` en DÉPLAÇANT le code**

Créer `client/src/hub/cartes.ts` :

```ts
// LE BALISAGE D'UNE CARTE D'APPLICATION — cloné depuis le `<template>` du
// hub, jamais construit élément par élément.
//
// 🔴 EXTRAIT DE `hub/page.ts::entree` LE 31 AOÛT 2026, DANS UNE TÂCHE DÉDIÉE
// ET AVANT L'ADDITION QU'ELLE PRÉPARE (le hub absorbe les deux sections du
// bureau). C'est la forme forte que `CLAUDE.md` exige : extraire, jamais
// comprimer, et jamais dans le commit qui ajoute.
//
// ⚠️ CE FICHIER N'EST PAS TESTÉ UNITAIREMENT, ET C'EST DÉCLARÉ PLUTÔT QUE
// SUBI — la convention de `hub/page.ts`, `shell-page.ts` et `main.ts`. Ce qui
// la rend tenable est la clause qui l'accompagne : **une condition est une
// RÈGLE si la changer change ce que le produit DÉCIDE ; elle est du CÂBLAGE si
// elle ne fait que router une décision déjà prise ailleurs.** Cloner un
// gabarit et y poser un nom ne décide rien. Et `client/` n'a **ni jsdom ni
// happy-dom** : les modules qui veulent être éprouvés injectent leurs
// dépendances (`accent-dom.ts`, `presse-papier-dom.ts`) — ce qui n'a pas de
// sens ici, où tout le travail EST la manipulation du gabarit.
//
// ⚠️ UNE EXTRACTION N'EST JAMAIS RIGOUREUSEMENT VERBATIM : elle laisse ses
// imports derrière elle (un `TS6133` est un ÉCHEC de `tsc`, pas un
// avertissement) et casse les déictiques. Relire `hub/page.ts` APRÈS ce
// déplacement, pas seulement avant.

import type { ApplicationListee } from './catalogue';

export interface DepsCarte {
    /// Le `<template id="modele-application">` du hub.
    modele: HTMLTemplateElement;
    /// L'URL absolue de l'icône, ou `null`. 🔴 LE CALCUL DE LA BASE RESTE CHEZ
    /// L'APPELANT : ce module n'a aucune raison de connaître l'adresse de la
    /// plateforme, et la lui donner en ferait un second endroit à tenir
    /// d'accord avec `adresse-plateforme.ts`.
    urlIcone: string | null;
    lancer(): void;
    installer(): void;
}

export function batirCarte(application: ApplicationListee, deps: DepsCarte): DocumentFragment {
    const fragment = deps.modele.content.cloneNode(true) as DocumentFragment;

    const icone = fragment.querySelector<HTMLImageElement>('[data-icone]')!;
    // 🔴 UN LITTÉRAL, PAS UN TERNAIRE SUR `className`. Une classe calculée est
    // invisible au contrôle §7.9, qui ne voit que les littéraux passés à
    // `classList.add('…')` et `className = '…'`. C'est la forme que
    // `fenetres-dom` emploie déjà pour ses pastilles.
    if (application.icone === null) icone.classList.add('hub__icone--absente');
    if (deps.urlIcone !== null) icone.src = deps.urlIcone;

    fragment.querySelector('[data-nom]')!.textContent = application.nom;
    fragment
        .querySelector<HTMLButtonElement>('[data-lancer]')!
        .addEventListener('click', () => deps.lancer());
    fragment
        .querySelector<HTMLButtonElement>('[data-installer]')!
        .addEventListener('click', () => deps.installer());
    return fragment;
}
```

⚠️ **Reprendre le commentaire 🔴 de l'icône qui vit dans `hub/page.ts`**
(« l'icône ne peut pas être posée par `src` … PLUS VRAI DEPUIS LE 30 AOÛT
2026 ») : il documente un renversement de décision, et le laisser derrière le
rendrait orphelin de la ligne qu'il explique.

- [ ] **Step 4: Employer le module dans `hub/page.ts`**

Remplacer la fonction `entree` par un appel, en gardant **verbatim** les corps
des deux gestionnaires de clic (celui de « Lancer » porte deux commentaires 🔴
sur l'activation transitoire ; celui d'« Installer » appelle
`publierLeManifeste`) :

```ts
function entree(application: ApplicationListee): DocumentFragment {
    return batirCarte(application, {
        modele: document.querySelector<HTMLTemplateElement>('#modele-application')!,
        urlIcone: application.icone_url === null ? null : `${base}${application.icone_url}`,
        lancer: () => { /* … le corps existant du clic « Lancer », VERBATIM … */ },
        installer: () => { /* … le corps existant du clic « Installer », VERBATIM … */ },
    });
}
```

⚠️ **Le type de retour change de `HTMLLIElement` à `DocumentFragment`.**
Adapter l'appelant dans `peupler()` — `elListe.append(entree(a))` accepte les
deux, mais `tsc` dira si une annotation explicite traîne.

- [ ] **Step 5: Vérifier**

```bash
cd client && npx tsc --noEmit && npx vitest run && npm run design:verifier && npm run build
```
Attendu : aucune erreur de type, tous les tests verts, le contrôle des classes
vert, le build passe. ⚠️ `tsc` est ce qui attrapera les imports laissés
derrière — un `TS6133` est un **échec**.

⚠️ **Si `design:verifier` rougit**, lire le nom de classe exact qu'il rend et
le corriger dans le HTML ou la feuille — **jamais en assouplissant le
contrôle**. `hub.html` n'entre dans `SURFACES_PRODUIT` qu'en tâche 5 : un
défaut introduit ici s'y révélerait, et il vaut mieux le trouver maintenant.

- [ ] **Step 6: Relever la taille APRÈS, et le contrôle du dépôt**

```bash
cd /home/mallanic/Projects/Nivuus/packages/desk
wc -l client/src/hub/page.ts client/src/hub/cartes.ts
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
```
Attendu : `page.ts` sensiblement sous son chiffre d'avant ; la dernière
commande ne rend que les fichiers déjà connus de la dette
(`agent/src/windows_source.rs`, et `client/src/shell-page.ts` à 500 — qui sera
vidé en tâche 9).

- [ ] **Step 7: Commit**

```bash
git add client/src/hub/cartes.ts client/src/hub/page.ts client/hub.html
git commit -m "extraction(navigation) : hub/cartes.ts, AVANT l addition qu elle prepare

hub/page.ts va absorber les deux sections du bureau. La regle du depot est
extraire, jamais comprimer, dans une tache DEDIEE jouee AVANT celle qui
ajoute -- la marge regagnee par une extraction se reperd si on la traite
comme acquise, ce qui a ete paye six fois.

Le balisage d une carte descend dans un <template> de hub.html, comme
shell.html le fait deja pour #modele-fenetre : les classes restent dans le
HTML, donc dans le perimetre le plus simple du controle 7.9.

AUCUN TEST NEUF, ET C EST DELIBERE : client/ n a ni jsdom ni happy-dom, ce qui
est une convention ecrite et non un manque -- accent-dom.test.ts dit que les
dependances sont injectees, et c est ce qui rend un module eprouvable. Cloner
un gabarit et y poser un nom ne DECIDE rien : c est du cablage, et le fichier
le declare comme hub/page.ts et shell-page.ts le declarent deja.

Effet de bord recherche : hub__icone--absente cesse d etre posee par un
ternaire sur className -- donc par une valeur CALCULEE, invisible au controle
7.9 -- et devient un classList.add d un LITTERAL.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 3: Un motif TYPÉ pour le refus « la place est prise »

**Files:**
- Modify: `plateforme/src/signaling/relais.ts` (le `send` du refus d'appariement, ~l. 356)
- Test: `plateforme/src/signaling/server.test.ts` (le test existant « rejette un second agent sur la même session », ~l. 174-182)

**Interfaces:**
- Produces: le message `{ type: 'error', reason: string, motif: 'role-occupe' }` sur le refus d'appariement.

**Pourquoi :** le refus est aujourd'hui `{ type: 'error', reason: "un client est déjà connecté à la session …" }`, **sans motif**. Le repli du porteur (spec §3, quand `navigator.locks` n'existe pas) doit distinguer « la place est prise » — qu'il faut avaler en silence — d'un refus d'une autre cause, qu'il faut afficher. Le faire sur une **phrase française** serait le piège de F1, payé neuf minutes sur deux messages qui partageaient une sous-chaîne.

⚠️ **Strictement ADDITIF** : le champ est ajouté, aucun n'est retiré. C'est la règle §10.2 que `webrtc.ts` et `shell-page.ts` appliquent déjà, et elle existe pour qu'un client d'hier continue de fonctionner.

🔴 **IL N'Y A PAS DE `relais.test.ts` — le relais est éprouvé par `server.test.ts`**, qui monte un vrai serveur et porte déjà `connect(role, session)` et `nextMessage(ws)`. Ne pas créer un fichier neuf : le cas est **déjà couvert**, et c'est son assertion qu'on étend.

- [ ] **Step 1: Étendre le test EXISTANT, qui va rougir**

Dans `plateforme/src/signaling/server.test.ts`, le test « rejette un second agent sur la même session » (~l. 174) assert par `toEqual`, qui est **strict sur l'ensemble des champs** : ajouter `motif` au produit le ferait échouer. C'est ce test-là qu'on met à jour, et non un nouveau à côté.

Remplacer son assertion par :

```ts
    it('rejette un second agent sur la même session', async () => {
        const first = await connect('agent', 's4');
        const second = await connect('agent', 's4');
        // 🔴 `motif` EST TYPÉ, `reason` EST UNE PHRASE. `toEqual` est STRICT :
        // c'est lui qui garantit qu'aucun champ n'est parti en douce, et c'est
        // pourquoi cette assertion est étendue plutôt que doublée.
        expect(await nextMessage(second)).toEqual({
            type: 'error',
            reason: 'un agent est déjà connecté à la session s4',
            motif: 'role-occupe',
        });
        first.close();
        second.close();
    });
```

Et ajouter, juste après, le cas qui compte pour le client :

```ts
    it('le refus porte un motif que le client peut trancher SANS lire la phrase', async () => {
        // 🔴 C'EST LA RAISON D'ÊTRE DU CHAMP. Le hub élit un onglet porteur
        // par Web Locks ; son REPLI (navigateur sans cette API) doit
        // distinguer « la place est prise » — à avaler en silence — d'un refus
        // d'une autre cause, qu'il faut afficher. Trancher sur `reason`
        // obligerait le client à comparer une phrase FRANÇAISE, qui se
        // reformule : le piège de F1.
        const premier = await connect('client', 's-motif');
        const second = await connect('client', 's-motif');
        const message = await nextMessage(second);
        expect(message.motif).toBe('role-occupe');
        premier.close();
        second.close();
    });
```

- [ ] **Step 2: Lancer les tests pour les voir rougir**

```bash
cd plateforme && npm run test:sqlite -- server
```
Attendu : **DEUX** échecs — le test étendu (`motif` absent de l'objet reçu) et le test neuf (`expected undefined to be 'role-occupe'`). **Lire les deux messages** : un seul échec voudrait dire que le montage ne fait pas ce qu'on croit.

- [ ] **Step 3: Ajouter le motif**

Dans `plateforme/src/signaling/relais.ts`, remplacer :

```ts
                const refus = sessions.declarer(declaredSession, declaredRole, socket);
                if (refus) {
                    send(socket, { type: 'error', reason: refus });
                    return;
                }
```
par :
```ts
                const refus = sessions.declarer(declaredSession, declaredRole, socket);
                if (refus) {
                    // 🔴 `motif` EST TYPÉ, `reason` EST UNE PHRASE. Ajouté le
                    // 31 août 2026 : le hub élit un onglet porteur par Web
                    // Locks, et son REPLI (navigateur sans cette API) doit
                    // distinguer « la place est prise » — à avaler en silence,
                    // un second onglet n'étant pas une faute de l'utilisateur —
                    // d'un refus d'une autre cause, qu'il faut afficher.
                    // Trancher sur `reason` obligerait le client à comparer une
                    // phrase FRANÇAISE, qui se reformule : c'est le piège de F1,
                    // payé neuf minutes sur deux messages qui partageaient une
                    // sous-chaîne.
                    //
                    // ⚠️ STRICTEMENT ADDITIF : le champ est ajouté, aucun n'est
                    // retiré (règle §10.2). Un client d'hier ne lit pas `motif`
                    // et continue de lire `reason`.
                    send(socket, { type: 'error', reason: refus, motif: 'role-occupe' });
                    return;
                }
```

- [ ] **Step 4: Lancer les tests, les deux bases**

```bash
cd plateforme && npm run test:sqlite && npm run test:postgres
```
Attendu : tous verts. ⚠️ **La double passe est la convention du dépôt** ; n'en jouer qu'une laisserait la moitié non éprouvée.

⚠️ **Chercher les AUTRES assertions strictes sur ce message** avant de conclure :
```bash
grep -rn "type: 'error'" plateforme/src --include='*.test.ts'
```
Toute autre `toEqual` portant sur le refus d'appariement doit être mise à jour dans ce même commit — un `toEqual` voisin qui rougirait plus tard serait attribué au mauvais lot.

- [ ] **Step 5: Commit**

```bash
git add plateforme/src/signaling/relais.ts plateforme/src/signaling/server.test.ts
git commit -m "ajout(signaling) : le refus d appariement porte un motif TYPE

Le refus rendait { type: error, reason: <phrase francaise> } et rien d autre.
Le hub va elire un onglet porteur par Web Locks, et son REPLI doit distinguer
la place est prise -- a avaler en silence, un second onglet n etant pas une
faute de l utilisateur -- d un refus d une autre cause, qu il faut afficher.

Trancher sur reason obligerait le client a comparer une phrase francaise, qui
se reformule : c est le piege de F1, paye neuf minutes sur deux messages qui
partageaient une sous-chaine.

Strictement additif : le champ est ajoute, aucun n est retire (regle 10.2). Le
test existant de server.test.ts assertait par toEqual, qui est STRICT : c est
son assertion qui est ETENDUE, pas un test neuf pose a cote -- sinon le champ
neuf aurait fait rougir un test que ce lot n aurait pas nomme.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 4: `bureau/porteur.ts` — l'élection, pure et testée

**Files:**
- Create: `client/src/bureau/porteur.ts`
- Test: `client/src/bureau/porteur.test.ts`

**Interfaces:**
- Consumes: `FenetreConnue` (`client/src/shell.ts`)
- Produces:
  ```ts
  export const NOM_VERROU = 'nivuus-bureau';
  export type Role = 'porteur' | 'suiveur';
  export interface DepsElection {
      /// `undefined` quand `navigator.locks` n'existe pas.
      verrou?: (nom: string, pendant: () => Promise<never>) => void;
      devenirPorteur(): void;
      devenirSuiveur(): void;
  }
  export function elire(nomVerrou: string, deps: DepsElection): void;
  export function estPlacePrise(message: unknown): boolean;
  export interface EtatDiffuse { type: 'etat-bureau'; fenetres: FenetreConnue[]; }
  export function batirEtat(fenetres: FenetreConnue[]): EtatDiffuse;
  export function lireEtat(donnees: unknown): FenetreConnue[] | undefined;
  ```

- [ ] **Step 1: Écrire les tests qui échouent**

Créer `client/src/bureau/porteur.test.ts` :

```ts
import { describe, expect, it } from 'vitest';
import { batirEtat, elire, estPlacePrise, lireEtat } from './porteur';

describe('elire', () => {
    it('sans API de verrou, l onglet devient porteur : le repli est OPTIMISTE', () => {
        // ⚠️ Optimiste et non pessimiste : sans verrou, se declarer suiveur
        // ferait qu AUCUN onglet n ouvrirait jamais la session. La plateforme
        // tranchera, et `estPlacePrise` rattrapera le perdant.
        let role = '';
        elire('v', {
            devenirPorteur: () => { role = 'porteur'; },
            devenirSuiveur: () => { role = 'suiveur'; },
        });
        expect(role).toBe('porteur');
    });

    it('avec l API, le porteur n est proclame QUE lorsque le verrou est obtenu', () => {
        let role = '';
        let relacher: (() => void) | undefined;
        elire('v', {
            // Un verrou qui n appelle JAMAIS `pendant` : le verrou n est pas
            // obtenu, donc cet onglet n est pas porteur.
            verrou: (_nom, pendant) => { relacher = () => void pendant(); },
            devenirPorteur: () => { role = 'porteur'; },
            devenirSuiveur: () => { role = 'suiveur'; },
        });
        expect(role).toBe('suiveur');
        // Puis le verrou se libere : l onglet en attente est promu.
        relacher!();
        expect(role).toBe('porteur');
    });
});

describe('estPlacePrise', () => {
    it('reconnait le motif TYPE', () => {
        expect(estPlacePrise({ type: 'error', reason: 'peu importe', motif: 'role-occupe' })).toBe(true);
    });

    it('ne reconnait PAS un refus d une autre cause', () => {
        // 🔴 CE CAS EST LE POINT : un frein de volume doit rester VISIBLE.
        // L avaler ferait de ce lot la panne muette qu il pretend eviter.
        expect(estPlacePrise({ type: 'error', reason: 'trop de requetes', motif: 'trop-de-requetes' })).toBe(false);
    });

    it('ne reconnait PAS un refus sans motif, meme si sa phrase le dit', () => {
        // ⚠️ Le piege de F1 : une phrase francaise se reformule. On ne
        // devine pas, on lit le motif -- ou on affiche.
        expect(estPlacePrise({ type: 'error', reason: 'un client est deja connecte a la session s' })).toBe(false);
    });

    it('ne leve pas sur une entree qui n est pas un objet', () => {
        expect(estPlacePrise(undefined)).toBe(false);
        expect(estPlacePrise('role-occupe')).toBe(false);
    });
});

describe('lireEtat', () => {
    it('rend la liste d un etat bien forme', () => {
        const etat = batirEtat([{ session: 's', titre: 'Bloc-notes', ouverte: true }]);
        expect(lireEtat(etat)?.[0]?.titre).toBe('Bloc-notes');
    });

    it('rend undefined sur un message d un AUTRE emetteur', () => {
        // Un `BroadcastChannel` est partage par origine : tout ce qui y passe
        // n est pas forcement de nous.
        expect(lireEtat({ type: 'autre-chose', fenetres: [] })).toBeUndefined();
    });

    it('rend undefined quand `fenetres` n est pas un tableau', () => {
        expect(lireEtat({ type: 'etat-bureau', fenetres: 'trois' })).toBeUndefined();
    });

    it('ECARTE une entree mal formee au lieu de la laisser passer', () => {
        const lu = lireEtat({
            type: 'etat-bureau',
            fenetres: [{ session: 's', titre: 'bon', ouverte: false }, { session: 42 }],
        });
        expect(lu?.length).toBe(1);
    });
});
```

- [ ] **Step 2: Lancer les tests pour vérifier qu'ils échouent**

```bash
cd client && npx vitest run src/bureau/porteur.test.ts
```
Attendu : ÉCHEC — le module `./porteur` n'existe pas.

- [ ] **Step 3: Écrire `client/src/bureau/porteur.ts`**

```ts
// L'ÉLECTION DE L'ONGLET QUI TIENT LA SESSION DE CONTRÔLE — la règle, pure
// et testée. Le câblage (socket, `BroadcastChannel`, DOM) vit dans
// `porteur-dom.ts`.
//
// 🔴 POURQUOI UNE ÉLECTION EXISTE. Le rôle `client` est EXCLUSIF par session
// (`plateforme/src/signaling/appariement.ts::declarer` — « un client est déjà
// connecté à la session … »). Tant que le bureau vivait dans une fenêtre
// NOMMÉE (`window.open(url, 'nivuus-bureau')`), il ne pouvait pas y en avoir
// deux. Depuis que le hub — atteint par l'URL racine, donc ouvrable en autant
// d'onglets qu'on veut — porte cette session, la question se pose vraiment.
//
// 🔵 LA PRIMITIVE EST **Web Locks**, PRISE TELLE QUELLE PLUTÔT QUE
// RECONSTRUITE. Un onglet qui obtient le verrou le tient jusqu'à sa mort, et
// le navigateur le libère lui-même : c'est exactement « le premier garde la
// session, et sa fermeture promeut un autre ». Une élection écrite à la main
// sur `BroadcastChannel` devrait DÉTECTER LA MORT D'UN PAIR, ce qu'aucun
// événement ne signale — c'est le trou que `setInterval(redessiner, 1000)`
// bouche déjà ailleurs, faute de mieux.

import type { FenetreConnue } from '../shell';

/// Le nom du verrou. ⚠️ **IL SE COMPOSE AVEC LE PRÉFIXE DE VM** avant usage
/// (`prefixe.ts::composer`), exactement comme le nom de session : sans lui,
/// deux VMs différentes ouvertes dans deux onglets s'excluraient l'une
/// l'autre — le défaut que P3 a corrigé sur le nom de session, réintroduit
/// par la porte de derrière.
export const NOM_VERROU = 'nivuus-bureau';

export interface DepsElection {
    /// Demande le verrou exclusif. `pendant` est appelée QUAND il est obtenu,
    /// et le verrou est tenu tant que la promesse qu'elle rend n'est pas
    /// résolue — d'où son type `Promise<never>` : on ne le rend jamais.
    ///
    /// `undefined` quand `navigator.locks` n'existe pas.
    verrou?: (nom: string, pendant: () => Promise<never>) => void;
    /// Cet onglet tient la session : il ouvre le socket.
    devenirPorteur(): void;
    /// Cet onglet suit : il n'ouvre AUCUN socket et affiche l'état diffusé.
    devenirSuiveur(): void;
}

/// Élit cet onglet, ou l'installe en suiveur en attendant son tour.
///
/// ⚠️ **LE REPLI SANS `navigator.locks` EST OPTIMISTE, ET C'EST DÉLIBÉRÉ** :
/// se déclarer suiveur ferait qu'AUCUN onglet n'ouvrirait jamais la session,
/// et le produit serait mort sur ce navigateur-là. On tente, la plateforme
/// tranche, et `estPlacePrise` rattrape le perdant en silence.
export function elire(nomVerrou: string, deps: DepsElection): void {
    if (deps.verrou === undefined) {
        deps.devenirPorteur();
        return;
    }
    deps.devenirSuiveur();
    deps.verrou(nomVerrou, () => {
        deps.devenirPorteur();
        // 🔴 UNE PROMESSE QUI NE SE RÉSOUT JAMAIS : c'est l'idiome des Web
        // Locks pour tenir un verrou jusqu'à la mort du contexte. Le
        // navigateur le libère à la fermeture de l'onglet, sans qu'aucun code
        // n'ait à l'orchestrer — y compris sur un plantage, où aucun
        // `beforeunload` ne courrait.
        return new Promise<never>(() => {});
    });
}

/// Ce refus est-il « la place est déjà prise » ?
///
/// 🔴 **SUR LE MOTIF TYPÉ, JAMAIS SUR LA PHRASE.** `reason` est du français
/// destiné à un humain, et il se reformule ; un client qui le comparerait
/// casserait en silence le jour où quelqu'un l'améliore. C'est le piège de
/// F1, payé neuf minutes sur deux messages qui partageaient une sous-chaîne.
///
/// ⚠️ **TOUT AUTRE REFUS REND `false`, ET C'EST LE POINT** : le frein de
/// volume (`trop-de-requetes`, avec son `retryApresS`) doit rester VISIBLE.
/// L'avaler ferait de cette élection la panne muette qu'elle prétend éviter.
export function estPlacePrise(message: unknown): boolean {
    if (typeof message !== 'object' || message === null) return false;
    return (message as { motif?: unknown }).motif === 'role-occupe';
}

/// L'état que le porteur diffuse aux autres onglets.
///
/// 🔴 **IL NE PORTE QUE DE L'ÉTAT, JAMAIS UN ORDRE.** `window.open` exige une
/// activation utilisateur **dans l'onglet qui a le geste** : relayer un clic
/// vers le porteur le ferait ouvrir hors activation, donc bloqué. Ce serait
/// déplacer le mur d'un cran — ce que `hub/bureau.ts` a explicitement refusé
/// de faire. Chaque onglet ouvre ses propres fenêtres depuis ses propres
/// clics.
export interface EtatDiffuse {
    type: 'etat-bureau';
    fenetres: FenetreConnue[];
}

export function batirEtat(fenetres: FenetreConnue[]): EtatDiffuse {
    return { type: 'etat-bureau', fenetres };
}

/// Lit un message reçu sur le canal, ou rend `undefined` si ce n'en est pas
/// un des nôtres.
///
/// ⚠️ **UN `BroadcastChannel` EST PARTAGÉ PAR ORIGINE** : tout ce qui y passe
/// ne vient pas forcément de nous, et une entrée mal formée est ÉCARTÉE plutôt
/// que laissée passer — une liste à moitié valide vaut mieux qu'un `undefined`
/// sur le champ `titre` au moment de peindre.
export function lireEtat(donnees: unknown): FenetreConnue[] | undefined {
    if (typeof donnees !== 'object' || donnees === null) return undefined;
    const message = donnees as { type?: unknown; fenetres?: unknown };
    if (message.type !== 'etat-bureau') return undefined;
    if (!Array.isArray(message.fenetres)) return undefined;
    return message.fenetres.filter(
        (f: unknown): f is FenetreConnue =>
            typeof f === 'object' &&
            f !== null &&
            typeof (f as FenetreConnue).session === 'string' &&
            typeof (f as FenetreConnue).titre === 'string' &&
            typeof (f as FenetreConnue).ouverte === 'boolean',
    );
}
```

- [ ] **Step 4: Lancer les tests pour vérifier qu'ils passent**

```bash
cd client && npx vitest run src/bureau/porteur.test.ts && npx tsc --noEmit
```
Attendu : PASS, aucune erreur de type.

- [ ] **Step 5: Voir la rouge du contrôle qui compte**

Le test qui ne doit pas pouvoir passer par accident est « ne reconnaît PAS un refus d'une autre cause » : c'est lui qui garde le frein de volume visible.

```bash
cp client/src/bureau/porteur.ts /tmp/porteur.ts.copie
# Muter : avaler TOUT refus de type error
sed -i "s/return (message as { motif?: unknown }).motif === 'role-occupe';/return (message as { type?: unknown }).type === 'error';/" client/src/bureau/porteur.ts
cd client && npx vitest run src/bureau/porteur.test.ts   # ATTENDU : ROUGE sur `trop-de-requetes`
cd .. && cp /tmp/porteur.ts.copie client/src/bureau/porteur.ts
cd client && npx vitest run src/bureau/porteur.test.ts   # ATTENDU : VERT
```
**Lire QUELLE assertion a rougi** : ce doit être celle du frein de volume, pas une autre.

- [ ] **Step 6: Commit**

```bash
git add client/src/bureau/porteur.ts client/src/bureau/porteur.test.ts
git commit -m "ajout(navigation) : l election de l onglet porteur, pure et testee

Le role client est EXCLUSIF par session. Tant que le bureau vivait dans une
fenetre NOMMEE il ne pouvait pas y en avoir deux ; depuis que le hub -- atteint
par l URL racine -- porte cette session, la question se pose vraiment.

La primitive est Web Locks, prise telle quelle : un onglet qui obtient le
verrou le tient jusqu a sa mort, et le navigateur le libere lui-meme, y compris
sur un plantage ou aucun beforeunload ne courrait. Une election ecrite a la
main devrait DETECTER LA MORT D UN PAIR, ce qu aucun evenement ne signale.

Le repli sans navigator.locks est OPTIMISTE : se declarer suiveur ferait qu
AUCUN onglet n ouvrirait jamais la session. On tente, la plateforme tranche, et
estPlacePrise rattrape le perdant en silence -- sur le MOTIF TYPE, jamais sur
la phrase francaise, qui se reformule.

Tout autre refus reste VISIBLE, et c est le point : avaler le frein de volume
ferait de cette election la panne muette qu elle pretend eviter. Ce controle-la
a ete VU ROUGE par mutation.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 5: `hub.html` absorbe le balisage du bureau

**Files:**
- Modify: `client/hub.html`
- Modify: `client/src/hub/hub.css` (ou lien vers `shell.css`)
- Modify: `client/outils/classes-employees.mjs` (`SURFACES_PRODUIT`)

**Interfaces:**
- Produces: les identifiants DOM que les tâches 6, 7 et 8 câblent — `#statut`, `#fenetres`, `#choisir-dossier`, `#etat-fichiers`, `#ecritures-dues`, `#actions-fichiers`, `#rafraichir`, `#reprendre-enregistrement`, `#modele-fenetre`, et les deux conteneurs `#section-fenetres`, `#section-fichiers`.

**À la fin de cette tâche, le balisage est en place et INERTE** : les deux sections sont masquées, aucune n'est câblée. `shell.html` continue de fonctionner à l'identique. C'est voulu — la révélation progressive fait qu'une section non peuplée est absente, donc l'inertie ne se voit pas.

- [ ] **Step 1: Ajouter les deux sections à `client/hub.html`**

Après `<div id="message" …>` et **avant** `<ul id="applications" …>` — non : après la liste, pour que le catalogue reste le sujet de la page (§4.1 de la spec) :

```html
            <ul id="applications" class="hub__grille"></ul>

            <!-- ═══ CE QUI VIENT DE `shell.html`, LE 31 AOÛT 2026 ═══════════
                 La page-shell est devenue une redirection : le hub est
                 désormais la SEULE surface. Ces deux sections viennent d'elle
                 telles quelles — mêmes identifiants, mêmes crochets `data-*`,
                 même `<template>` — pour que `shell.ts`, qui est pur et testé,
                 soit RÉUTILISÉ et non réécrit.

                 🔴 LES CROCHETS D'INSTRUMENT SURVIVENT AU DÉPLACEMENT :
                 `data-dues`, `data-vues` et `data-retenues` sont lus par les
                 pilotes de recette, et JAMAIS le texte — piège de F1, payé
                 neuf minutes sur deux messages qui partageaient une
                 sous-chaîne. -->

            <div id="statut" class="message hub__message" role="status"></div>

            <!-- ⚠️ `hidden` PAR DÉFAUT, ET RETIRÉ PAR LE SCRIPT DÈS QU'IL Y A
                 UNE FENÊTRE. Une section montrant en permanence « aucune
                 fenêtre ouverte » serait du bruit sur l'état NOMINAL d'un hub
                 qu'on vient d'ouvrir (décision du propriétaire, spec §4.1). -->
            <section id="section-fenetres" class="hub__section" hidden>
                <h2 class="hub__section-titre">Mes fenêtres</h2>
                <ul id="fenetres" class="bureau__fenetres"></ul>
            </section>

            <!-- ⚠️ REPLIÉE, JAMAIS RETIRÉE : « Choisir mon dossier » doit
                 rester atteignable en un geste. `<details>` porte le pli
                 nativement — un élément replié reste dans le DOM, donc reste
                 lisible par un pilote de recette ; un élément RETIRÉ ne l'est
                 pas, et c'est pourquoi c'est un pli et non un rendu
                 conditionnel. Le script l'ouvre (`open = true`) dès qu'un pont
                 est monté, pour que le compteur d'écritures dues et le bouton
                 « Reprendre » ne soient jamais cachés au moment où ils
                 comptent. -->
            <details id="section-fichiers" class="hub__section">
                <summary class="hub__section-titre">Mes fichiers</summary>
                <div class="bureau__actions">
                    <button id="choisir-dossier" type="button" class="bouton bouton--secondaire">
                        Choisir mon dossier
                    </button>
                </div>
                <div id="etat-fichiers" class="message bureau__bandeau" role="status"></div>
                <div
                    id="ecritures-dues"
                    class="message bureau__bandeau"
                    role="status"
                    data-dues="0"
                    data-vues="0"
                ></div>
                <p class="bureau__actions" data-retenues="false" id="actions-fichiers">
                    <button id="rafraichir" type="button" class="bouton bouton--discret">
                        Rafraîchir
                    </button>
                    <button
                        id="reprendre-enregistrement"
                        type="button"
                        class="bouton bouton--secondaire"
                        hidden
                    >
                        Reprendre l’enregistrement
                    </button>
                </p>
            </details>
```

Et, juste avant `<script type="module" src="/src/hub/page.ts">`, le `<template>` **copié verbatim depuis `shell.html`** (le bloc `<template id="modele-fenetre">` et son commentaire).

⚠️ **Copier, pas retaper** : `sed -n '/<template id="modele-fenetre">/,/<\/template>/p' client/shell.html`.

- [ ] **Step 2: Lier la feuille du bureau**

Dans `<head>` de `client/hub.html`, après `hub.css` :

```html
        <!-- ⚠️ APRÈS `hub.css`, ET L'ORDRE PORTE LA CASCADE. Cette feuille
             vient de `shell.html` avec les sections ci-dessous ; elle sera
             renommée `bureau/bureau.css` quand la page-shell disparaîtra
             (tâche 9), le nom `shell` mentant dès lors sur ce qu'elle peint. -->
        <link rel="stylesheet" href="/src/shell.css" />
```

- [ ] **Step 3: Mettre `hub.html` sous contrôle des classes**

Dans `client/outils/classes-employees.mjs`, remplacer :
```js
const SURFACES_PRODUIT = ['client/index.html', 'client/shell.html', 'client/connexion.html'];
```
par :
```js
/** Les surfaces du PRODUIT — celles qu'un utilisateur voit.
 *
 * 🔴 `client/hub.html` Y ENTRE LE 31 AOÛT 2026, ET IL N'Y ÉTAIT PAS : le hub
 * est servi à la racine depuis le lot 14, donc c'est LA surface que
 * l'utilisateur atteint, et elle était pourtant hors de tout contrôle §7.9.
 * `client/shell.html` y reste jusqu'à ce qu'elle devienne une redirection
 * (tâche 9) — les deux pages emploient les mêmes classes pendant la
 * transition, ce qui est licite et voulu.
 */
const SURFACES_PRODUIT = [
    'client/index.html',
    'client/hub.html',
    'client/shell.html',
    'client/connexion.html',
];
```

- [ ] **Step 4: Lancer le contrôle des classes et le voir passer**

```bash
cd client && npm run design:verifier
```
Attendu : vert. **S'il rougit**, c'est qu'une classe employée dans le nouveau balisage n'est pas déclarée (ou l'inverse) : lire le nom exact qu'il rend et le corriger dans le HTML ou la feuille — **jamais en assouplissant le contrôle**.

- [ ] **Step 5: Vérifier que la page se bâtit**

```bash
cd client && npm run build && ls dist/
```
Attendu : `hub.html` et `shell.html` présents. ⚠️ Une page absente de `rollupOptions.input` ne sort pas du build **et rien ne le dit** — les deux doivent être là.

- [ ] **Step 6: Commit**

```bash
git add client/hub.html client/outils/classes-employees.mjs
git commit -m "balisage(navigation) : le hub accueille les deux sections du bureau

Memes identifiants, memes crochets data-*, meme template que shell.html : c est
ce qui permettra de REUTILISER shell.ts, qui est pur et teste, au lieu de le
reecrire. Les sections sont inertes a ce stade, et shell.html fonctionne
toujours a l identique.

Mes fenetres est hidden par defaut (une section montrant en permanence aucune
fenetre ouverte serait du bruit sur l etat NOMINAL) ; Mes fichiers est un
<details> repli, jamais retire -- un element replie reste dans le DOM donc
reste lisible par un pilote, un element retire ne l est pas.

Et hub.html entre dans SURFACES_PRODUIT, ou il n etait PAS : le hub est servi
a la racine depuis le lot 14, donc c est LA surface que l utilisateur atteint,
et elle etait hors de tout controle des classes.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 6: La liste des fenêtres — une règle PURE, et son câblage

**Files:**
- Create: `client/src/bureau/fenetres.ts` (**pur, testé**)
- Create: `client/src/bureau/fenetres-dom.ts` (câblage, non testé)
- Modify: `client/src/shell-page.ts` (remplacer `redessiner`)
- Test: `client/src/bureau/fenetres.test.ts`

**Interfaces:**
- Consumes: `FenetreConnue` (`../shell`)
- Produces:
  ```ts
  // fenetres.ts — PUR
  export interface LigneFenetre {
      session: string;
      titre: string;
      /// Le mot affiché dans la pastille.
      etat: 'ouverte' | 'fermée';
      /// 🔴 DEUX LITTÉRAUX, jamais une classe composée : une classe calculée
      /// est invisible au contrôle §7.9.
      classePastille: 'bureau__pastille--ouverte' | 'bureau__pastille--fermee';
      /// Faut-il un bouton « Rouvrir » ?
      rouvrable: boolean;
  }
  export function lignes(fenetres: FenetreConnue[]): LigneFenetre[];
  export function sectionVisible(fenetres: FenetreConnue[]): boolean;

  // fenetres-dom.ts — CÂBLAGE
  export interface DepsFenetres {
      liste: HTMLUListElement;
      modele: HTMLTemplateElement;
      /// La section à révéler ; `undefined` quand la page n'en a pas
      /// (`shell.html`, qui affiche la liste sans pli).
      section?: HTMLElement;
      rouvrir(session: string): void;
  }
  export function dessinerFenetres(fenetres: FenetreConnue[], deps: DepsFenetres): void;
  ```

🔴 **LA SCISSION EST LE POINT DE LA TÂCHE — ruling R2 du pré-vol.** `client/`
n'a **ni jsdom ni happy-dom**, par convention écrite. Ce qui **décide** —
quel mot, quelle classe, un bouton ou pas, la section visible ou non — descend
donc dans un module PUR qu'on éprouve sans DOM ; ce qui ne fait que **poser**
ces valeurs dans un gabarit reste du câblage déclaré non testé.

**Pourquoi l'extraction se fait DEPUIS `shell-page.ts` :** le module est
aussitôt employé par la page qui existe, donc aucun code n'est dupliqué entre
les deux surfaces, et `shell.html` reste vert à chaque étape. Le hub s'en
servira en tâche 8.

- [ ] **Step 1: Écrire les tests qui échouent**

Créer `client/src/bureau/fenetres.test.ts` :

```ts
import { describe, expect, it } from 'vitest';
import { lignes, sectionVisible } from './fenetres';

describe('lignes', () => {
    it('une fenetre OUVERTE n est pas rouvrable : il n y a rien a suggerer', () => {
        expect(lignes([{ session: 's', titre: 'Bloc-notes', ouverte: true }])[0].rouvrable).toBe(false);
    });

    it('une fenetre FERMEE est rouvrable', () => {
        expect(lignes([{ session: 's', titre: 'Paint', ouverte: false }])[0].rouvrable).toBe(true);
    });

    it('la pastille porte DEUX litteraux distincts, jamais une classe composee', () => {
        // 🔴 UNE CLASSE CALCULEE EST INVISIBLE AU CONTROLE 7.9, qui ne voit que
        // les litteraux. Les deux valeurs doivent donc etre des constantes du
        // type, pas une concatenation.
        const ouverte = lignes([{ session: 'a', titre: 'x', ouverte: true }])[0];
        const fermee = lignes([{ session: 'b', titre: 'x', ouverte: false }])[0];
        expect([ouverte.classePastille, fermee.classePastille]).toEqual([
            'bureau__pastille--ouverte',
            'bureau__pastille--fermee',
        ]);
    });

    it('le mot de la pastille est accentue du cote FERME', () => {
        // `fermée`, pas `fermee` : c est du texte montre a un humain.
        expect(lignes([{ session: 's', titre: 'x', ouverte: false }])[0].etat).toBe('fermée');
    });

    it('la session est reconduite telle quelle : c est elle qui rouvre', () => {
        expect(lignes([{ session: 's-7', titre: 'x', ouverte: false }])[0].session).toBe('s-7');
    });

    it('l ordre des fenetres est PRESERVE', () => {
        const rendu = lignes([
            { session: 'a', titre: 'Un', ouverte: true },
            { session: 'b', titre: 'Deux', ouverte: false },
        ]);
        expect(rendu.map((l) => l.titre)).toEqual(['Un', 'Deux']);
    });
});

describe('sectionVisible', () => {
    it('aucune fenetre : la section est ABSENTE', () => {
        // 🔴 ABSENTE, PAS VIDE. Une section montrant en permanence « aucune
        // fenetre ouverte » serait du bruit sur l etat NOMINAL d un hub qu on
        // vient d ouvrir (spec 4.1).
        expect(sectionVisible([])).toBe(false);
    });

    it('une fenetre, meme FERMEE : la section est visible', () => {
        // Une fenetre fermee a quelque chose a offrir -- son bouton Rouvrir --
        // donc la cacher priverait l utilisateur du seul geste qui la ramene.
        expect(sectionVisible([{ session: 's', titre: 'x', ouverte: false }])).toBe(true);
    });
});
```

- [ ] **Step 2: Lancer les tests pour vérifier qu'ils échouent**

```bash
cd client && npx vitest run src/bureau/fenetres.test.ts
```
Attendu : ÉCHEC — le module `./fenetres` n'existe pas.

- [ ] **Step 3: Écrire `client/src/bureau/fenetres.ts`**

```ts
// CE QU'UNE FENÊTRE CONNUE DEVIENT À L'ÉCRAN — la règle, pure et testée.
// La pose dans le gabarit vit dans `fenetres-dom.ts`, qui ne décide rien.
//
// 🔴 POURQUOI CETTE SCISSION PLUTÔT QU'UN SEUL MODULE DOM. `client/` n'a **ni
// jsdom ni happy-dom**, et ce n'est pas un manque : `accent-dom.test.ts` le
// déclare comme la convention du dépôt — « les dépendances sont injectées, et
// c'est ce qui rend ce module éprouvable là où `main.ts` ne l'est pas ». Ce
// fichier-ci prend l'autre voie, également admise : sortir la DÉCISION du
// câblage, et ne laisser en face du DOM que ce qui pose des valeurs déjà
// calculées.

import type { FenetreConnue } from '../shell';

export interface LigneFenetre {
    session: string;
    titre: string;
    /// Le mot montré dans la pastille — du texte pour un humain, donc accentué.
    etat: 'ouverte' | 'fermée';
    /// 🔴 DEUX LITTÉRAUX, ET NON UNE CLASSE COMPOSÉE : une classe calculée est
    /// invisible au contrôle §7.9, qui ne voit que les littéraux passés à
    /// `classList.add('…')` et à `className = '…'`. Le type les énumère, ce
    /// qui les rend aussi vérifiables par `tsc`.
    classePastille: 'bureau__pastille--ouverte' | 'bureau__pastille--fermee';
    /// Une fenêtre ouverte n'a rien à rouvrir : son bouton PART plutôt que
    /// d'être désactivé — il n'y a pas d'action à suggérer.
    rouvrable: boolean;
}

export function lignes(fenetres: FenetreConnue[]): LigneFenetre[] {
    return fenetres.map((f) => ({
        session: f.session,
        titre: f.titre,
        etat: f.ouverte ? 'ouverte' : 'fermée',
        classePastille: f.ouverte ? 'bureau__pastille--ouverte' : 'bureau__pastille--fermee',
        rouvrable: !f.ouverte,
    }));
}

/// La section « Mes fenêtres » doit-elle paraître ?
///
/// 🔴 ABSENTE, PAS VIDE (spec §4.1) : une section montrant en permanence
/// « aucune fenêtre ouverte » serait du bruit sur l'état NOMINAL d'un hub
/// qu'on vient d'ouvrir. ⚠️ Une fenêtre FERMÉE compte : elle porte son bouton
/// « Rouvrir », donc la cacher priverait l'utilisateur du seul geste qui la
/// ramène.
export function sectionVisible(fenetres: FenetreConnue[]): boolean {
    return fenetres.length > 0;
}
```

- [ ] **Step 4: Lancer les tests pour vérifier qu'ils passent**

```bash
cd client && npx vitest run src/bureau/fenetres.test.ts && npx tsc --noEmit
```
Attendu : PASS, aucune erreur de type.

- [ ] **Step 5: Voir la rouge du contrôle qui compte**

Le contrôle qui ne doit pas pouvoir passer par accident est « aucune fenêtre :
la section est ABSENTE ».

```bash
cp client/src/bureau/fenetres.ts /tmp/fenetres.ts.copie
sed -i 's/return fenetres.length > 0;/return true;/' client/src/bureau/fenetres.ts
cd client && npx vitest run src/bureau/fenetres.test.ts   # ATTENDU : ROUGE sur « ABSENTE »
cd .. && cp /tmp/fenetres.ts.copie client/src/bureau/fenetres.ts
cd client && npx vitest run src/bureau/fenetres.test.ts   # ATTENDU : VERT
```
**Lire QUELLE assertion a rougi** : ce doit être `expected true to be false`
sur `sectionVisible([])`, pas une autre. Une rouge qui rougit pour la mauvaise
raison ne prouve rien.

- [ ] **Step 6: Écrire le câblage `client/src/bureau/fenetres-dom.ts`**

```ts
// LA POSE DE LA LISTE « MES FENÊTRES » DANS LE GABARIT — du câblage, et rien
// d'autre. Les décisions (quel mot, quelle classe, un bouton ou pas, la
// section visible ou non) vivent dans `fenetres.ts`, qui est pur et testé.
//
// 🔴 EXTRAIT DE `shell-page.ts` LE 31 AOÛT 2026. Ce n'est pas un rangement :
// `shell-page.ts` déclare lui-même, dans son en-tête, que son câblage n'est
// éprouvable par RIEN. Cette extraction ne rend pas CE fichier-ci éprouvable
// non plus — elle sort de lui tout ce qui pouvait l'être.
//
// ⚠️ LE BALISAGE VIENT D'UN `<template>` DE LA PAGE, PAS D'ICI : les classes
// restent dans le HTML, où le contrôle §7.9 les lit sans avoir à analyser du
// TypeScript.

import type { FenetreConnue } from '../shell';
import { lignes, sectionVisible } from './fenetres';

export interface DepsFenetres {
    liste: HTMLUListElement;
    modele: HTMLTemplateElement;
    /// La section à révéler. `undefined` quand la page affiche la liste sans
    /// pli — c'est le cas de `shell.html`, qui n'a pas de section masquable.
    section?: HTMLElement;
    rouvrir(session: string): void;
}

export function dessinerFenetres(fenetres: FenetreConnue[], deps: DepsFenetres): void {
    deps.liste.replaceChildren();
    for (const ligne of lignes(fenetres)) {
        const item = deps.modele.content.cloneNode(true) as DocumentFragment;
        item.querySelector('[data-titre]')!.textContent = ligne.titre;

        const pastille = item.querySelector<HTMLElement>('[data-etat]')!;
        pastille.textContent = ligne.etat;
        pastille.classList.add(ligne.classePastille);

        const bouton = item.querySelector<HTMLButtonElement>('[data-rouvrir]')!;
        if (ligne.rouvrable) bouton.addEventListener('click', () => deps.rouvrir(ligne.session));
        else bouton.remove();

        deps.liste.append(item);
    }
    // ⚠️ RÉVÉLER **ET** RECACHER. Ne faire que le premier laisserait une
    // section vide sur un hub qui n'a plus rien à montrer.
    if (deps.section !== undefined) deps.section.hidden = !sectionVisible(fenetres);
}
```

- [ ] **Step 7: Employer le module dans `shell-page.ts`**

Remplacer la fonction `redessiner` de `shell-page.ts` par :

```ts
function redessiner(): void {
    dessinerFenetres(bureau.liste(), {
        liste,
        modele: modeleFenetre,
        // `shell.html` affiche la liste sans pli : aucune section à révéler.
        rouvrir: (session) => { bureau.rouvrir(session); redessiner(); },
    });
}
```
et ajouter `import { dessinerFenetres } from './bureau/fenetres-dom';`.

- [ ] **Step 8: Lancer tout**

```bash
cd client && npx vitest run && npx vitest run --dir ../proto && npx tsc --noEmit
```
Attendu : tous verts, aucune erreur de type.

- [ ] **Step 9: Commit**

```bash
git add client/src/bureau/fenetres.ts client/src/bureau/fenetres.test.ts client/src/bureau/fenetres-dom.ts client/src/shell-page.ts
git commit -m "extraction(navigation) : la liste des fenetres, sa REGLE separee de sa pose

Extraite de shell-page.ts, et employee par lui immediatement : aucun code n est
duplique entre les deux surfaces, et shell.html reste vert.

La scission est le point. client/ n a ni jsdom ni happy-dom, et ce n est pas un
manque : accent-dom.test.ts le declare comme la convention du depot. Ce qui
DECIDE -- quel mot, quelle classe, un bouton ou pas, la section visible ou non
-- descend donc dans fenetres.ts, pur et teste sans DOM ; ce qui ne fait que
poser ces valeurs dans un gabarit reste du cablage declare non teste.

Reveler ET recacher : ne faire que le premier laisserait une section vide sur
un hub qui n a plus rien a montrer. Ce controle-la a ete VU ROUGE, et l
assertion qui a rougi est bien celle-la.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 7: `bureau/fichiers-dom.ts` — le pont, extrait VERBATIM

**Files:**
- Create: `client/src/bureau/fichiers-dom.ts`
- Modify: `client/src/shell-page.ts`

**Interfaces:**
- Consumes: `Bureau` (`../shell`), `CanalFichiers`, `choisirDossier`, `connecterCanalFichiers`, `sessionDuPont` (`../fichiers/canal`), `creerServeur`, `trameBonjour`, `trameRafraichir` (`../fichiers/protocole`), `creerAdaptateur` (`../fichiers/adaptateur`), `creerEcrivain`, `type RacineInscriptible` (`../fichiers/ecriture`), `creerMutateur` (`../fichiers/mutation-service`), `type RacineMutable` (`../fichiers/mutation`)
- Produces:
  ```ts
  export interface DepsFichiers {
      bureau: Bureau;
      signalingUrl: string;
      fautesArmees: boolean;
      boutonDossier: HTMLButtonElement;
      boutonRafraichir: HTMLButtonElement;
      boutonReprendre: HTMLButtonElement;
      /// Le `<details>` à déplier au clic ; `undefined` si la page n'a pas de
      /// pli (`shell.html`).
      section?: HTMLDetailsElement;
  }
  export function installerLePont(deps: DepsFichiers): void;
  ```

🔴 **AUCUN TEST NEUF, ET C'EST UNE CONSÉQUENCE, PAS UN RENONCEMENT — ruling R2
du pré-vol.** Ce bloc est du câblage de bout en bout : il ouvre un
`RTCPeerConnection`, un `WebSocket` et un sélecteur de répertoire, dont aucun
n'existe sous Node, et `client/` n'a **ni jsdom ni happy-dom** par convention.
Tout ce qu'il porte de décidable est **déjà** testé ailleurs — `shell.ts`,
`fichiers/protocole.ts` et `fichiers/adaptateur.ts` sont purs et couverts.
**Cette tâche est une extraction, et son contrôle est que rien ne change** :
`tsc --noEmit`, la suite existante entière, et le build.

⚠️ **C'EST L'EXTRACTION LA PLUS DÉLICATE DU PLAN.** `monterLeLecteur` fait
~130 lignes et porte cinq blocs 🔴 qui documentent chacun un défaut **mesuré** :
l'ordre du `Bonjour`, l'attente de l'ouverture du canal (« le `Bonjour` partait
dans le vide, **et donc AUCUNE écriture due n'aurait jamais été poussée** »),
la garde `if (pont !== ce)` (un écouteur qui survit à son objet), la fermeture
des flux au `close`, et le transtypage qui **n'est PAS un contrôle**. **Les
déplacer à la lettre.** Aucun n'est décoratif ; chacun décrit une panne qui a
coûté une mesure.

- [ ] **Step 1: Relever ce qui doit être déplacé, AVANT de déplacer**

```bash
cd /home/mallanic/Projects/Nivuus/packages/desk
grep -n "let pont\|boutonDossier.addEventListener\|async function monterLeLecteur\|^}" client/src/shell-page.ts | head -20
grep -c "🔴" client/src/shell-page.ts
```
Noter le nombre de blocs 🔴 **avant**, et le revérifier **après** : la somme
des deux fichiers doit être au moins égale. Un commentaire perdu en chemin est
une preuve perdue.

- [ ] **Step 2: Créer le module en DÉPLAÇANT le bloc**

Créer `client/src/bureau/fichiers-dom.ts` avec cet en-tête, puis la variable
`let pont`, le gestionnaire de `boutonDossier`, et la fonction
`monterLeLecteur` **entière avec tous ses commentaires** :

```ts
// LE LECTEUR « MES FICHIERS » — le câblage du pont ProjFS, à dépendances
// injectées.
//
// 🔴 EXTRAIT DE `shell-page.ts` LE 31 AOÛT 2026, VERBATIM. Les cinq blocs 🔴
// qu'il porte documentent chacun un défaut MESURÉ — l'ordre du `Bonjour`,
// l'attente de l'ouverture du canal (le `Bonjour` partait dans le vide, et
// AUCUNE écriture due n'aurait jamais été poussée), la garde `pont !== ce`
// (un écouteur qui survit à son objet), la fermeture des flux, et le
// transtypage qui n'est PAS un contrôle. Aucun n'est décoratif : ils ont été
// déplacés à la lettre, jamais résumés.
//
// ⚠️ CE FICHIER N'EST PAS TESTÉ, ET NE PEUT PAS L'ÊTRE ICI : il ouvre un
// `RTCPeerConnection`, un `WebSocket` et un sélecteur de répertoire, dont
// aucun n'existe sous Node — et `client/` n'a ni jsdom ni happy-dom, par
// convention (`accent-dom.test.ts`). Tout ce qu'il porte de DÉCIDABLE est
// testé ailleurs : `shell.ts`, `fichiers/protocole.ts` et
// `fichiers/adaptateur.ts` sont purs et couverts.

import { creerAdaptateur } from '../fichiers/adaptateur';
import {
    choisirDossier,
    connecterCanalFichiers,
    sessionDuPont,
    type CanalFichiers,
} from '../fichiers/canal';
import { creerEcrivain, type RacineInscriptible } from '../fichiers/ecriture';
import type { RacineMutable } from '../fichiers/mutation';
import { creerMutateur } from '../fichiers/mutation-service';
import { creerServeur, trameBonjour, trameRafraichir } from '../fichiers/protocole';
import type { Bureau } from '../shell';

export interface DepsFichiers {
    bureau: Bureau;
    signalingUrl: string;
    /// `?faute-fichiers=1` — variable de BANC, jamais une configuration
    /// livrée. Lue UNE fois par la page et passée ici, jamais relue : c'est la
    /// convention de `PLEIN_ECRAN` et de `PART_SONDAGE` côté agent — le
    /// mécanisme lit un drapeau qu'on lui donne. Et un utilisateur qui
    /// créerait un dossier au nom réservé ne casserait pas son propre pont.
    fautesArmees: boolean;
    boutonDossier: HTMLButtonElement;
    boutonRafraichir: HTMLButtonElement;
    boutonReprendre: HTMLButtonElement;
    /// Le `<details>` à déplier au clic ; `undefined` quand la page n'a pas de
    /// pli — c'est le cas de `shell.html`.
    section?: HTMLDetailsElement;
}

export function installerLePont(deps: DepsFichiers): void {
    let pont: CanalFichiers | null = null;

    deps.boutonDossier.addEventListener('click', () => {
        // ⚠️ LE DÉPLIAGE EST AU CLIC, ET AVANT TOUT `await`. Le déplier au
        // montage RÉUSSI donnerait l'impression, sur une annulation du
        // sélecteur, que le clic n'a rien fait.
        if (deps.section !== undefined) deps.section.open = true;
        // 🔴 `showDirectoryPicker()` EXIGE UNE ACTIVATION UTILISATEUR
        // TRANSITOIRE, et c'est pourquoi il est appelé depuis ce gestionnaire.
        // Celui-ci n'est pas `async` : un `await` avant l'appel consommerait
        // l'activation, et le sélecteur serait refusé sans que rien ne le dise.
        void monterLeLecteur();
    });

    async function monterLeLecteur(): Promise<void> {
        /* … LE CORPS EXISTANT DE `shell-page.ts::monterLeLecteur`, DÉPLACÉ
           VERBATIM, avec ses cinq blocs 🔴 et ses ⚠️. Les seules substitutions
           mécaniques sont :
             `bureau.`            → `deps.bureau.`
             `signalingUrl`       → `deps.signalingUrl`
             `fautesFichiersArmees` → `deps.fautesArmees`
             `boutonRafraichir`   → `deps.boutonRafraichir`
             `boutonReprendre`    → `deps.boutonReprendre`
           Aucune autre ligne ne change, et aucun commentaire n'est résumé. */
    }
}
```

🔴 **NE PAS RETAPER LE CORPS DE MÉMOIRE.** Le copier depuis le fichier :
```bash
sed -n '/^async function monterLeLecteur/,/^}$/p' client/src/shell-page.ts
```
puis appliquer les cinq substitutions ci-dessus, **et relire le résultat contre
la source**.

- [ ] **Step 3: Employer le module dans `shell-page.ts`**

Remplacer tout le bloc du lecteur (de `let pont` à la fin de
`monterLeLecteur`) par :

```ts
installerLePont({
    bureau,
    signalingUrl,
    fautesArmees: fautesFichiersArmees,
    boutonDossier,
    boutonRafraichir,
    boutonReprendre,
    // `shell.html` n'a aucun pli : sa section est toujours dépliée.
});
```
et ajouter `import { installerLePont } from './bureau/fichiers-dom';`.

- [ ] **Step 4: Vérifier que RIEN n'a changé**

```bash
cd client && npx vitest run && npx vitest run --dir ../proto && npx tsc --noEmit && npm run build
```
Attendu : tous verts, aucune erreur, le build passe. ⚠️ `tsc` attrapera les
imports restés dans `shell-page.ts` — un `TS6133` est un **échec**, pas un
avertissement, et c'est le contrôle principal de cette tâche.

- [ ] **Step 5: Vérifier qu'aucune preuve n'a été perdue**

```bash
cd /home/mallanic/Projects/Nivuus/packages/desk
grep -c "🔴" client/src/shell-page.ts client/src/bureau/fichiers-dom.ts
wc -l client/src/shell-page.ts client/src/bureau/fichiers-dom.ts
```
La somme des 🔴 doit être **au moins** celle relevée à l'étape 1. Si elle a
baissé, un commentaire a été résumé ou perdu : le retrouver dans
`git show HEAD:client/src/shell-page.ts` avant de continuer.

- [ ] **Step 6: Commit**

```bash
git add client/src/bureau/fichiers-dom.ts client/src/shell-page.ts
git commit -m "extraction(navigation) : le pont fichiers devient un module a dependances injectees

Deplace VERBATIM depuis shell-page.ts, ses cinq blocs de commentaire compris :
chacun documente un defaut MESURE -- l ordre du Bonjour, l attente de l
ouverture du canal (le Bonjour partait dans le vide, et AUCUNE ecriture due n
aurait jamais ete poussee), la garde pont !== ce, la fermeture des flux, et le
transtypage qui n est PAS un controle. Aucun n est decoratif, et le compte des
marqueurs a ete verifie de part et d autre.

AUCUN TEST NEUF, et c est une consequence, pas un renoncement : ce bloc ouvre
un RTCPeerConnection, un WebSocket et un selecteur de repertoire, dont aucun n
existe sous Node, et client/ n a ni jsdom ni happy-dom par convention. Tout ce
qu il porte de DECIDABLE est deja teste ailleurs. Le controle de cette tache
est que rien ne change : tsc, la suite entiere, le build.

Le depliage de la section est au CLIC et avant tout await : le deplier au
montage REUSSI donnerait l impression, sur une annulation du selecteur, que le
clic n a rien fait -- et showDirectoryPicker exige une activation transitoire
qu un await intercale consommerait.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 8: Le hub devient porteur — `bureau/porteur-dom.ts`

**Files:**
- Create: `client/src/bureau/porteur-dom.ts`
- Modify: `client/src/hub/page.ts` (brancher)
- Test: `client/src/bureau/porteur-dom.test.ts`

**Interfaces:**
- Consumes: `elire`, `estPlacePrise`, `batirEtat`, `lireEtat`, `NOM_VERROU` (`./porteur`) ; `dessinerFenetres` (`./fenetres-dom`, qui applique lui-même la règle pure de `./fenetres`) ; `creerBureau`, `type Bureau`, `type Ton` (`../shell`) ; `dessinerFenetres` (`./fenetres-dom`) ; `installerLePont` (`./fichiers-dom`) ; `composer`, `lirePrefixe` (`../prefixe`) ; `adresseSignaling` (`../adresse-plateforme`)
- Produces:
  ```ts
  export interface DepsBureauPage {
      signalingUrl: string;
      jeton: string;
      prefixe: string;
      fautesArmees: boolean;
      elements: {
          statut: HTMLDivElement;
          liste: HTMLUListElement;
          modele: HTMLTemplateElement;
          sectionFenetres: HTMLElement;
          sectionFichiers: HTMLDetailsElement;
          boutonDossier: HTMLButtonElement;
          etatFichiers: HTMLDivElement;
          ecrituresDues: HTMLDivElement;
          actionsFichiers: HTMLParagraphElement;
          boutonRafraichir: HTMLButtonElement;
          boutonReprendre: HTMLButtonElement;
      };
  }
  export function installerLeBureau(deps: DepsBureauPage): void;
  ```

- [ ] **Step 1: Écrire le test qui échoue**

Créer `client/src/bureau/porteur-dom.test.ts`. Ce qui est éprouvable sans réseau : que le **suiveur n'ouvre aucun socket**, et que la diffusion peint la liste.

```ts
// ⚠️ AUCUNE directive `@vitest-environment` : ces deux fonctions sont PURES,
// et `client/` n'a ni jsdom ni happy-dom — par convention, pas par oubli
// (`accent-dom.test.ts`). C'est pour cela qu'elles sont exportées séparément
// du reste du module, qui, lui, touche le DOM et n'est pas testé.
import { describe, expect, it } from 'vitest';
import { diffuserSiChange, nomDuVerrou } from './porteur-dom';

describe('nomDuVerrou', () => {
    it('porte le PREFIXE de VM', () => {
        // 🔴 SANS LUI, DEUX VMs OUVERTES DANS DEUX ONGLETS S EXCLURAIENT L UNE
        // L AUTRE -- le defaut que P3 a corrige sur le nom de session,
        // reintroduit par la porte de derriere.
        expect(nomDuVerrou('vm-7')).toBe('vm-7:nivuus-bureau');
    });

    it('sans prefixe connu, rend le nom nu', () => {
        expect(nomDuVerrou('')).toBe('nivuus-bureau');
    });
});

describe('diffuserSiChange', () => {
    it('ne diffuse RIEN quand l etat est identique', () => {
        // ⚠️ Le porteur redessine a 1 Hz : diffuser a chaque tour reveillerait
        // tous les onglets une fois par seconde pour rien.
        const envoyes: unknown[] = [];
        const canal = { postMessage: (m: unknown) => void envoyes.push(m) };
        const liste = [{ session: 's', titre: 'x', ouverte: true }];
        let dernier = '';
        dernier = diffuserSiChange(canal, liste, dernier);
        dernier = diffuserSiChange(canal, liste, dernier);
        expect(envoyes.length).toBe(1);
    });

    it('diffuse quand une fenetre change d etat', () => {
        const envoyes: unknown[] = [];
        const canal = { postMessage: (m: unknown) => void envoyes.push(m) };
        let dernier = diffuserSiChange(canal, [{ session: 's', titre: 'x', ouverte: true }], '');
        dernier = diffuserSiChange(canal, [{ session: 's', titre: 'x', ouverte: false }], dernier);
        expect(envoyes.length).toBe(2);
    });
});
```

- [ ] **Step 2: Lancer le test pour vérifier qu'il échoue**

```bash
cd client && npx vitest run src/bureau/porteur-dom.test.ts
```
Attendu : ÉCHEC — module introuvable.

- [ ] **Step 3: Écrire `client/src/bureau/porteur-dom.ts`**

```ts
// LE CÂBLAGE DU BUREAU DANS LE HUB : élection, socket de la session de
// contrôle, canal entre onglets, DOM.
//
// 🔴 CE FICHIER EST LE SUCCESSEUR DE `shell-page.ts`, ET IL NE REPREND PAS SON
// DÉFAUT : ses dépendances sont INJECTÉES, et les trois règles qu'il emploie
// (`porteur.ts`, `fenetres-dom.ts`, `shell.ts`) sont testées ailleurs.
//
// ⚠️ AUCUNE RÈGLE ICI. Une condition qui déciderait quelque chose du produit
// doit descendre dans `porteur.ts` ou `shell.ts`.

// ⚠️ AUCUN import d'`adresseSignaling` ICI : l'URL arrive par `deps`, calculée
// par `hub/page.ts`. L'importer sans l'employer serait un `TS6133`, c'est-à-dire
// un ÉCHEC de `tsc --noEmit`, pas un avertissement.
import { composer } from '../prefixe';
import { creerBureau, type Bureau, type FenetreConnue, type Ton } from '../shell';
import { dessinerFenetres } from './fenetres-dom';
import { installerLePont } from './fichiers-dom';
import { NOM_VERROU, batirEtat, elire, estPlacePrise, lireEtat } from './porteur';

/// Le nom du verrou d'élection, PRÉFIXÉ par la VM.
///
/// 🔴 SANS LE PRÉFIXE, deux VMs différentes ouvertes dans deux onglets
/// s'excluraient l'une l'autre : le défaut que P3 a corrigé sur le nom de
/// session, réintroduit par la porte de derrière. `composer` rend le nom nu
/// quand aucun préfixe n'est connu — exactement le comportement d'avant P3.
export function nomDuVerrou(prefixe: string): string {
    return composer(prefixe, NOM_VERROU);
}

export interface CanalDiffusion {
    postMessage(message: unknown): void;
}

/// Diffuse l'état aux autres onglets **seulement s'il a changé**, et rend la
/// nouvelle empreinte.
///
/// ⚠️ LE PORTEUR REDESSINE À 1 Hz (la fermeture d'une fenêtre par
/// l'utilisateur ne prévient personne : on relit l'état plutôt que d'attendre
/// un événement qui n'existe pas). Diffuser à chaque tour réveillerait tous
/// les onglets une fois par seconde pour rien.
export function diffuserSiChange(
    canal: CanalDiffusion,
    fenetres: FenetreConnue[],
    empreintePrecedente: string,
): string {
    const empreinte = JSON.stringify(fenetres);
    if (empreinte === empreintePrecedente) return empreintePrecedente;
    canal.postMessage(batirEtat(fenetres));
    return empreinte;
}
```

Puis, dans le même fichier, `installerLeBureau` — la reprise de `shell-page.ts`, avec l'élection en plus :

```ts
export interface DepsBureauPage {
    signalingUrl: string;
    jeton: string;
    /// Le préfixe de VM (`prefixe.ts::lirePrefixe`), ou `''` s'il n'y en a pas.
    prefixe: string;
    /// `?faute-fichiers=1` — variable de BANC, jamais une configuration
    /// livrée. Lue UNE fois par la page et passée en argument, jamais relue
    /// ici : c'est la convention de `PLEIN_ECRAN` et de `PART_SONDAGE` côté
    /// agent — le mécanisme lit un drapeau qu'on lui donne.
    fautesArmees: boolean;
    elements: {
        statut: HTMLDivElement;
        liste: HTMLUListElement;
        modele: HTMLTemplateElement;
        sectionFenetres: HTMLElement;
        sectionFichiers: HTMLDetailsElement;
        boutonDossier: HTMLButtonElement;
        etatFichiers: HTMLDivElement;
        ecrituresDues: HTMLDivElement;
        actionsFichiers: HTMLParagraphElement;
        boutonRafraichir: HTMLButtonElement;
        boutonReprendre: HTMLButtonElement;
    };
}

export function installerLeBureau(deps: DepsBureauPage): void {
    const el = deps.elements;
    const sessionDeControle = composer(deps.prefixe, 'bureau');
    const canal = new BroadcastChannel(nomDuVerrou(deps.prefixe));
    let empreinte = '';
    let porteur = false;
    // ⚠️ DÉCLARÉ AVANT `creerBureau`, dont le rappel `envoyer` le lit : une
    // fermeture qui capture un `let` déclaré plus bas compile, mais se lit
    // mal — et la zone morte temporelle est une classe d'erreur qu'on évite
    // par la disposition plutôt que par la vigilance.
    let socket: WebSocket | undefined;

    const poserTon = (element: HTMLElement, ton: Ton): void => {
        element.classList.remove('message--succes', 'message--alerte', 'message--danger');
        if (ton !== 'neutre') element.classList.add(`message--${ton}`);
    };

    const bureau = creerBureau({
        ouvrirFenetre(session) {
            // 🔴 `/index.html`, PAS `/` : la racine sert le HUB depuis le lot
            // 14, et `/?session=…` ouvrirait le hub avec un paramètre qu'il
            // ignore, jamais une session.
            return window.open(`/index.html?session=${encodeURIComponent(session)}`, `guac-${session}`);
        },
        envoyer(message) {
            if (socket?.readyState === WebSocket.OPEN) socket.send(JSON.stringify(message));
        },
        afficher(message, ton) { el.statut.textContent = message; poserTon(el.statut, ton); },
        afficherEtatFichiers(texte, ton) { el.etatFichiers.textContent = texte; poserTon(el.etatFichiers, ton); },
        afficherEcrituresDues(dues, vues, texte, ton) {
            // 🔴 LES NOMBRES DANS DES ATTRIBUTS `data-*`, LE TEXTE DANS LA
            // PAGE. Les pilotes de recette lisent les attributs, JAMAIS le
            // texte — piège de F1.
            el.ecrituresDues.dataset.dues = String(dues);
            el.ecrituresDues.dataset.vues = String(vues);
            el.ecrituresDues.textContent = texte;
            poserTon(el.ecrituresDues, ton);
        },
        afficherRetenues(retenues) {
            el.boutonReprendre.hidden = !retenues;
            el.actionsFichiers.dataset.retenues = String(retenues);
            // Un pont qui retient ses écritures a quelque chose à dire MAINTENANT.
            if (retenues) el.sectionFichiers.open = true;
        },
    });

    const redessiner = (): void => {
        const fenetres = bureau.liste();
        dessinerFenetres(fenetres, {
            liste: el.liste,
            modele: el.modele,
            section: el.sectionFenetres,
            rouvrir: (session) => { bureau.rouvrir(session); redessiner(); },
        });
        if (porteur) empreinte = diffuserSiChange(canal, fenetres, empreinte);
    };

    // ── LE SUIVEUR : il n'ouvre AUCUN socket, et peint ce qu'on lui dit ────
    canal.addEventListener('message', (evenement) => {
        if (porteur) return;
        const fenetres = lireEtat(evenement.data);
        if (fenetres === undefined) return;
        dessinerFenetres(fenetres, {
            liste: el.liste,
            modele: el.modele,
            section: el.sectionFenetres,
            // ⚠️ UN SUIVEUR OUVRE SA PROPRE FENÊTRE, DEPUIS SON PROPRE CLIC :
            // `window.open` exige l'activation de CET onglet-ci. Il ne
            // prévient pas le porteur, dont la liste continuera d'afficher
            // « fermée » jusqu'à sa prochaine relecture. **Limite déclarée** :
            // le remède serait un ordre sur le canal, que la conception exclut
            // (spec §4), ou une méthode neuve sur `shell.ts`, que la spec
            // laisse INCHANGÉ (spec §6). Recliquer « Rouvrir » ramène la même
            // fenêtre au premier plan : aucun dommage.
            rouvrir: (session) => {
                window.open(`/index.html?session=${encodeURIComponent(session)}`, `guac-${session}`);
            },
        });
    });

    const ouvrirLaSession = (): void => {
        porteur = true;
        socket = new WebSocket(deps.signalingUrl);
        socket.addEventListener('open', () => {
            socket!.send(JSON.stringify({ role: 'client', session: sessionDeControle, jeton: deps.jeton }));
            el.statut.textContent = 'bureau connecté';
            poserTon(el.statut, 'neutre');
        });
        socket.addEventListener('message', (evenement) => {
            const message = JSON.parse(evenement.data);
            if (message.type === 'fenetre-ouverte') bureau.fenetreOuverte(message.session, message.titre);
            else if (message.type === 'fenetre-fermee') bureau.fenetreFermee(message.session);
            else if (message.type === 'refus') bureau.refus(message.titre, message.motif);
            else if (message.type === 'error') {
                // 🔴 « LA PLACE EST PRISE » N'EST PAS UNE ERREUR À MONTRER.
                // Ce chemin n'est atteint que dans le REPLI (navigateur sans
                // Web Locks) : cet onglet a tenté, un autre tenait déjà. Il
                // redevient suiveur, EN SILENCE — un second onglet n'est pas
                // une faute de l'utilisateur. Tout autre refus, dont le frein
                // de volume, reste affiché.
                if (estPlacePrise(message)) { porteur = false; socket?.close(); return; }
                bureau.canalDeControleRefuse(message.reason, message.motif, message.retryApresS);
            }
            redessiner();
        });
        socket.addEventListener('close', () => {
            // ⚠️ SILENCIEUX SI NOUS AVONS CÉDÉ LA PLACE : `canalDeControlePerdu`
            // dirait « Rechargez la page », ce qui serait faux ici.
            if (porteur) bureau.canalDeControlePerdu();
        });
    };

    elire(nomDuVerrou(deps.prefixe), {
        verrou:
            typeof navigator !== 'undefined' && 'locks' in navigator
                ? (nom, pendant) => void navigator.locks.request(nom, { mode: 'exclusive' }, pendant)
                : undefined,
        devenirPorteur: ouvrirLaSession,
        devenirSuiveur: () => { porteur = false; },
    });

    installerLePont({
        bureau,
        signalingUrl: deps.signalingUrl,
        fautesArmees: deps.fautesArmees,
        boutonDossier: el.boutonDossier,
        boutonRafraichir: el.boutonRafraichir,
        boutonReprendre: el.boutonReprendre,
        section: el.sectionFichiers,
    });

    window.addEventListener('beforeunload', (evenement) => {
        if (!bureau.doitPrevenir()) return;
        evenement.preventDefault();
    });

    // Les pages de session annoncent leur viewport par `postMessage` sur leur
    // ouvreuse — c'est-à-dire ici.
    window.addEventListener('message', (evenement) => {
        if (evenement.origin !== window.location.origin) return;
        const message = evenement.data;
        if (message?.type === 'viewport') {
            bureau.viewportRecu(message.session, message.largeur, message.hauteur);
        }
    });

    // La fermeture d'une page par l'utilisateur ne prévient personne : on
    // relit l'état périodiquement plutôt que d'attendre un événement qui
    // n'existe pas.
    setInterval(redessiner, 1000);
}
```

⚠️ **`navigator.locks` n'est pas dans les types DOM de toutes les versions de TypeScript.** Si `tsc` s'en plaint, ajouter une déclaration minimale en tête du fichier plutôt qu'un `any` :
```ts
declare global {
    interface Navigator {
        locks?: { request(nom: string, options: { mode: 'exclusive' }, pendant: () => Promise<never>): Promise<void> };
    }
}
```

- [ ] **Step 4: Brancher dans `hub/page.ts`**

À la fin de `demarrer()`, après `await peupler()` :

```ts
    // ── LE BUREAU, DANS CETTE PAGE ────────────────────────────────────────
    // 🔴 LE HUB EST DÉSORMAIS LA SEULE SURFACE (décision du propriétaire,
    // 31 août 2026). Le lien « Mon bureau » et l'ouverture au clic sur
    // « Lancer » étaient les correctifs du 30 août ; ils n'ont plus d'objet.
    installerLeBureau({
        signalingUrl: adresseSignaling(window.location, params.get('signaling')),
        jeton: acces,
        prefixe: lirePrefixe(),
        fautesArmees: params.get('faute-fichiers') === '1',
        elements: {
            statut: document.querySelector<HTMLDivElement>('#statut')!,
            liste: document.querySelector<HTMLUListElement>('#fenetres')!,
            modele: document.querySelector<HTMLTemplateElement>('#modele-fenetre')!,
            sectionFenetres: document.querySelector<HTMLElement>('#section-fenetres')!,
            sectionFichiers: document.querySelector<HTMLDetailsElement>('#section-fichiers')!,
            boutonDossier: document.querySelector<HTMLButtonElement>('#choisir-dossier')!,
            etatFichiers: document.querySelector<HTMLDivElement>('#etat-fichiers')!,
            ecrituresDues: document.querySelector<HTMLDivElement>('#ecritures-dues')!,
            actionsFichiers: document.querySelector<HTMLParagraphElement>('#actions-fichiers')!,
            boutonRafraichir: document.querySelector<HTMLButtonElement>('#rafraichir')!,
            boutonReprendre: document.querySelector<HTMLButtonElement>('#reprendre-enregistrement')!,
        },
    });
```

Et **retirer** le lien « Mon bureau » : le bloc `if (elBureau !== null) { … }` de `hub/page.ts`, le `<a id="bureau">` de `hub.html`, et l'appel `ouvrirLeBureau` du gestionnaire « Lancer » — avec le message d'échec qui renvoyait à « Employez « Mon bureau » en haut de page », devenu faux.

⚠️ `client/src/hub/bureau.ts` et son test **deviennent morts**. Les supprimer dans cette tâche (`git rm`), en expliquant dans le commit que leur raison d'être — « aucun lien de navigation ne menait à shell.html » — n'existe plus.

⚠️ **Retirer aussi l'import correspondant en tête de `hub/page.ts`** :
`import { NOM_FENETRE_BUREAU, PAGE_DU_BUREAU, ouvrirLeBureau } from './bureau';`.
Un import laissé derrière est un `TS6133`, donc un **échec** de `tsc --noEmit` —
« une extraction n'est jamais rigoureusement verbatim : elle laisse ses imports
derrière elle ».

- [ ] **Step 5: Lancer tout**

```bash
cd client && npx vitest run && npx vitest run --dir ../proto && npx tsc --noEmit && npm run design:verifier && npm run build
```
Attendu : tout vert.

- [ ] **Step 6: Commit**

```bash
git add client/src/bureau/porteur-dom.ts client/src/bureau/porteur-dom.test.ts client/src/hub/page.ts client/hub.html
git rm client/src/hub/bureau.ts client/src/hub/bureau.test.ts
git commit -m "navigation : le hub TIENT la session de controle, et un seul onglet a la fois

Le hub cesse de renvoyer vers une seconde surface : il ouvre lui-meme la
session de controle, montre ses fenetres et son pont. hub/bureau.ts et son test
disparaissent -- leur raison d etre, aucun lien de navigation ne mene a
shell.html, n existe plus.

Un onglet est elu porteur par Web Locks ; les autres n ouvrent AUCUN socket et
peignent l etat diffuse. Le canal ne porte que de l ETAT, jamais un ordre :
window.open exige l activation de l onglet QUI A LE GESTE, et relayer un clic
ferait ouvrir le porteur hors activation -- ce serait deplacer le mur d un
cran.

La place est prise est avale en SILENCE (un second onglet n est pas une faute
de l utilisateur) ; tout autre refus, dont le frein de volume, reste affiche.

LIMITE DECLAREE : un suiveur qui rouvre une fenetre ne previent pas le porteur,
dont la liste dira fermee jusqu a sa prochaine relecture. Recliquer ramene la
meme fenetre au premier plan : aucun dommage. Le remede serait un ordre sur le
canal (exclu par conception) ou une methode neuve sur shell.ts (que la spec
laisse INCHANGE).

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 9: `shell.html` devient une redirection

**Files:**
- Modify: `client/shell.html` (vidé)
- Modify: `client/src/shell-page.ts` (vidé)
- Move: `client/src/shell.css` → `client/src/bureau/bureau.css`
- Modify: `client/hub.html` (le lien CSS)
- Modify: `client/src/connexion.ts:74` (le défaut de `suite`)
- Modify: `client/outils/classes-employees.mjs` (retirer `client/shell.html`)
- Test: `client/src/bureau/redirection.test.ts`

**Interfaces:**
- Produces: `export function cibleDeRedirection(recherche: string): string;`

- [ ] **Step 1: Écrire le test qui échoue**

Créer `client/src/bureau/redirection.test.ts` :

```ts
import { describe, expect, it } from 'vitest';
import { cibleDeRedirection } from './redirection';

describe('cibleDeRedirection', () => {
    it('conserve la chaine de requete', () => {
        // 🔴 LE POINT DE TOUTE LA TACHE : les PWA installees portent
        // `shell.html?app=<id>` dans un manifeste blob: qu elles ne reliront
        // JAMAIS. Perdre `?app=` les casserait aussi surement que supprimer le
        // fichier.
        expect(cibleDeRedirection('?app=u-1')).toBe('/?app=u-1');
    });

    it('sans requete, mene a la racine nue', () => {
        expect(cibleDeRedirection('')).toBe('/');
    });

    it('conserve PLUSIEURS parametres', () => {
        expect(cibleDeRedirection('?app=u-1&plateforme=https%3A%2F%2Fx')).toBe(
            '/?app=u-1&plateforme=https%3A%2F%2Fx',
        );
    });
});
```

- [ ] **Step 2: Lancer le test pour vérifier qu'il échoue**

```bash
cd client && npx vitest run src/bureau/redirection.test.ts
```
Attendu : ÉCHEC — module introuvable.

- [ ] **Step 3: Écrire la règle et vider les deux fichiers**

`client/src/bureau/redirection.ts` :

```ts
// LA REDIRECTION DE L'ANCIENNE PAGE-SHELL — une règle, pure et testée.
//
// 🔴 POURQUOI `shell.html` SURVIT COMME REDIRECTION PLUTÔT QUE D'ÊTRE
// SUPPRIMÉ (décision du propriétaire, 31 août 2026). Les manifestes des PWA
// par application portent `start_url: shell.html?app=<id>`, et ils sont
// publiés en `blob:` (`hub/manifeste.ts`, legs G5 déclaré) : une PWA installée
// **ne relira jamais son manifeste**, donc elle ouvrira cette adresse pour
// toujours. Supprimer le fichier les casserait définitivement. Ce n'est pas
// une transition : c'est le chemin définitif de ces installations-là.

/// L'adresse vers laquelle rediriger, chaîne de requête CONSERVÉE.
///
/// ⚠️ `?app=` EST LE POINT : le perdre casserait les PWA aussi sûrement que
/// supprimer le fichier.
export function cibleDeRedirection(recherche: string): string {
    return `/${recherche}`;
}
```

`client/src/shell-page.ts`, **remplacé en entier** :

```ts
// L'ANCIENNE PAGE-SHELL — devenue une simple redirection le 31 août 2026.
//
// 🔴 CE FICHIER PORTAIT 500 LIGNES ET LA MOITIÉ DU PRODUIT. Son contenu vit
// désormais dans `bureau/porteur-dom.ts`, `bureau/fenetres-dom.ts` et
// `bureau/fichiers-dom.ts`, employés par le hub — la SEULE surface depuis
// cette date. La règle métier `shell.ts`, elle, n'a pas bougé d'une ligne :
// elle était déjà pure et testée, et elle est réutilisée telle quelle.
//
// ⚠️ NE PAS SUPPRIMER CE FICHIER NI SA PAGE. Voir `bureau/redirection.ts`
// pour la raison — elle tient aux PWA déjà installées, pas à la prudence.

import { cibleDeRedirection } from './bureau/redirection';

// `replace` et non `href` : un retour arrière ramènerait sur cette page qui
// redirigerait de nouveau, et l'utilisateur serait piégé dans l'historique.
window.location.replace(cibleDeRedirection(window.location.search));
```

`client/shell.html`, **remplacé en entier** :

```html
<!doctype html>
<html lang="fr">
    <head>
        <meta charset="UTF-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1.0" />
        <title>Bureau</title>
        <!--
          🔴 CETTE PAGE N'EST PLUS QU'UNE REDIRECTION (31 août 2026). Le hub,
          servi à la racine, est désormais la seule surface : il tient la
          session de contrôle, montre les fenêtres et porte le pont fichiers.

          ⚠️ ELLE N'EST PAS SUPPRIMÉE, ET CE N'EST PAS DE LA PRUDENCE : les PWA
          par application déjà installées portent `shell.html?app=<id>` dans un
          manifeste `blob:` qu'elles ne reliront JAMAIS. Voir
          `src/bureau/redirection.ts`.

          ⚠️ ELLE RESTE DANS `rollupOptions.input` de `vite.config.ts` : une
          page absente de cette liste ne sort pas du build ET RIEN NE LE DIT.

          AUCUN `<link>` ICI, à dessein : la page ne peint rien, et lier une
          feuille ferait un éclair de style avant la redirection.
        -->
    </head>
    <body>
        <script type="module" src="/src/shell-page.ts"></script>
    </body>
</html>
```

- [ ] **Step 4: Déplacer la feuille et mettre à jour ses lecteurs**

```bash
cd /home/mallanic/Projects/Nivuus/packages/desk
git mv client/src/shell.css client/src/bureau/bureau.css
sed -i 's#/src/shell.css#/src/bureau/bureau.css#' client/hub.html
grep -rn 'shell\.css' client --include='*.html' --include='*.ts' --include='*.mjs' | grep -v node_modules
```
La dernière commande doit rendre **zéro ligne**. Ajouter en tête de `bureau.css` :
```css
/* La peinture des sections du bureau — « Mes fenêtres » et « Mes fichiers ».
   🔴 RENOMMÉE DEPUIS `shell.css` LE 31 AOÛT 2026 : la page-shell est devenue
   une redirection, et le nom `shell` mentait dès lors sur ce qu'elle peint.
   Elle est liée par `hub.html`, la seule surface du produit. */
```

- [ ] **Step 5: Corriger le défaut de `suite`**

Dans `client/src/connexion.ts:74` :
```ts
// 🔴 LA RACINE, PLUS `shell.html` (31 août 2026) : la page-shell est devenue
// une redirection, et y renvoyer ferait faire un aller-retour inutile à qui
// vient de se connecter.
const suite = params.get('suite') ?? '/';
```

- [ ] **Step 6: Retirer `shell.html` de `SURFACES_PRODUIT`**

Dans `client/outils/classes-employees.mjs`, retirer `'client/shell.html'` de la liste et mettre le commentaire à jour : la page ne porte plus aucune classe.

- [ ] **Step 7: Lancer tout**

```bash
cd client && npx vitest run && npx vitest run --dir ../proto && npx tsc --noEmit && npm run design:verifier && npm run build && ls dist/
```
Attendu : tout vert, et **`shell.html` présent dans `dist/`**.

- [ ] **Step 8: Vérifier la taille du dépôt**

```bash
cd /home/mallanic/Projects/Nivuus/packages/desk
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
```
Attendu : `client/src/shell-page.ts` a **disparu** de cette liste.

- [ ] **Step 9: Commit**

```bash
git add client/shell.html client/src/shell-page.ts client/src/bureau/redirection.ts client/src/bureau/redirection.test.ts client/src/bureau/bureau.css client/hub.html client/src/connexion.ts client/outils/classes-employees.mjs
git commit -m "navigation : shell.html devient une redirection, et sort de la dette

500 lignes et la moitie du produit deviennent trois lignes. Le contenu vit dans
bureau/porteur-dom.ts, fenetres-dom.ts et fichiers-dom.ts, employes par le hub.
shell.ts n a pas bouge d une ligne : deja pur et teste, reutilise tel quel.

La page N EST PAS SUPPRIMEE, et ce n est pas de la prudence : les PWA par
application deja installees portent shell.html?app=<id> dans un manifeste blob:
qu elles ne reliront JAMAIS. Ce n est pas une transition, c est le chemin
definitif de ces installations-la -- d ou le test qui fige la conservation de
la chaine de requete.

Elle reste dans rollupOptions.input : une page absente ne sort pas du build ET
RIEN NE LE DIT.

shell.css devient bureau/bureau.css -- le nom shell mentait des lors sur ce qu
elle peint -- et connexion.ts renvoie desormais a la racine.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 10: Le manifeste — `start_url` migre, `id` reste

**Files:**
- Modify: `client/src/hub/manifeste.ts:244-245`
- Test: `client/src/hub/manifeste.test.ts`

- [ ] **Step 1: Écrire les tests qui échouent**

Dans `client/src/hub/manifeste.test.ts`, **remplacer** les quatre assertions existantes sur `start_url`/`id` (l. 48, 50, 59, 64-65) et ajouter :

```ts
it('start_url mene a la RACINE, ou le hub tient la session', () => {
    const m = batirManifeste({ id: 'u-1', nom: 'x' }, 'https://exemple.test', '#000');
    expect(m.start_url).toBe('https://exemple.test/?app=u-1');
});

it('id NE BOUGE PAS : il porte l identite de la PWA installee', () => {
    // 🔴 CHANGER `id` N EST PAS UNE MISE A JOUR : c est une SECONDE
    // application, la premiere devenant orpheline. Et comme le manifeste est
    // publie en blob:, une PWA installee ne le relit JAMAIS -- elle
    // continuera d ouvrir shell.html, que la redirection rattrape.
    const m = batirManifeste({ id: 'u-1', nom: 'x' }, 'https://exemple.test', '#000');
    expect(m.id).toBe('https://exemple.test/shell.html?app=u-1');
});

it('id et start_url DIVERGENT desormais, et c est voulu', () => {
    const m = batirManifeste({ id: 'u-1', nom: 'x' }, 'https://exemple.test', '#000');
    expect(m.id === m.start_url).toBe(false);
});

it('start_url reste DANS le scope, condition que Chromium verifie', () => {
    const m = batirManifeste({ id: 'u-1', nom: 'x' }, 'https://exemple.test', '#000');
    expect(m.start_url.startsWith(m.scope)).toBe(true);
});

it('l identifiant d application reste encode dans les DEUX', () => {
    const m = batirManifeste({ id: 'a/b?c', nom: 'x' }, 'https://x', '#000');
    expect(m.start_url).toBe('https://x/?app=a%2Fb%3Fc');
    expect(m.id).toBe('https://x/shell.html?app=a%2Fb%3Fc');
});
```

⚠️ Adapter la forme de `Sujet` aux champs réellement requis — les relever avec `sed -n '85,100p' client/src/hub/manifeste.ts`.

- [ ] **Step 2: Lancer les tests pour vérifier qu'ils échouent**

```bash
cd client && npx vitest run src/hub/manifeste.test.ts
```
Attendu : ÉCHEC sur `start_url`.

- [ ] **Step 3: Modifier `manifeste.ts`**

Remplacer les deux lignes et **réécrire le commentaire 🔴 qui les précède**, devenu faux :

```ts
        // 🔴 `id` ET `start_url` DIVERGENT DEPUIS LE 31 AOÛT 2026, ET C'EST
        // DÉLIBÉRÉ.
        //
        // `id` est l'IDENTITÉ de l'application installée. Le changer n'est pas
        // une mise à jour : c'est une SECONDE application, la première
        // devenant orpheline. Il reste donc figé sur `shell.html?app=`, la
        // valeur que portent les installations déjà faites — et comme le
        // manifeste est publié en `blob:` (voir l'en-tête de ce fichier), une
        // PWA installée ne relira JAMAIS le sien : elle ouvrira `shell.html`
        // pour toujours, où `client/src/bureau/redirection.ts` la rattrape.
        //
        // `start_url`, lui, migre vers la RACINE : le hub y est servi et y
        // tient désormais la session de contrôle (décision du propriétaire,
        // 31 août 2026). C'est la valeur que prendront les installations
        // FUTURES.
        //
        // ⚠️ IL RESTE DANS LE `scope` (`${base}/`), condition que Chromium
        // vérifie et refuse en toutes lettres sinon.
        id: `${base}/shell.html?app=${app}`,
        start_url: `${base}/?app=${app}`,
```

Corriger aussi le commentaire d'en-tête de `batirManifeste` (« `start_url` POINTE LA PAGE-SHELL, ET NON LE HUB (décision D9 du plan) »), devenu faux : **la décision D9 est renversée**, et le dire plutôt que d'effacer.

- [ ] **Step 4: Lancer les tests**

```bash
cd client && npx vitest run src/hub/manifeste.test.ts && npx tsc --noEmit
```
Attendu : PASS.

- [ ] **Step 5: Chercher les autres affirmations devenues fausses**

```bash
cd /home/mallanic/Projects/Nivuus/packages/desk
grep -rn 'shell\.html\|page-shell\|shell-page' client/src client/*.html plateforme/src --include='*.ts' --include='*.html' --include='*.mjs' | grep -v node_modules
```
🔴 **Énumérer les occurrences AVANT d'écrire, et les relire une par une APRÈS.** « Corrigé à sa place » est une affirmation de **complétude** : ce dépôt a payé neuf fois le fait de corriger là où on nous l'a montré. Chercher aussi **par le sens** : « la seule page qui traite `fenetre-ouverte` », « la seconde surface », « le lien Mon bureau ».

- [ ] **Step 6: Commit**

```bash
git add client/src/hub/manifeste.ts client/src/hub/manifeste.test.ts
git commit -m "manifeste : start_url migre vers la racine, id reste fige

Changer id n est pas une mise a jour : c est une SECONDE application, la
premiere devenant orpheline. Il reste donc sur shell.html?app=, la valeur que
portent les installations deja faites -- et comme le manifeste est publie en
blob:, une PWA installee ne relira JAMAIS le sien : elle ouvrira shell.html
pour toujours, ou la redirection la rattrape.

start_url migre vers la racine pour les installations FUTURES, et reste dans le
scope, condition que Chromium verifie et refuse en toutes lettres sinon.

La decision D9 du plan de G5 -- start_url pointe la page-shell -- est
RENVERSEE, et son commentaire le dit plutot que de l effacer.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 11: Revue transverse et clôture

**Files:**
- Modify: `CLAUDE.md` (index des chantiers, tableau de dette)
- Create: `docs/superpowers/plans/2026-08-31-navigation-hub-unique-resultats.md`

🔴 **UNE REVUE PAR TÂCHE NE PEUT PAS VOIR UN DÉFAUT QUI FRANCHIT UNE FRONTIÈRE DE TÂCHE** — chacun est correct des deux côtés pris séparément. C'est une propriété du découpage, pas un défaut de rigueur, et c'est ce qui justifie cette tâche. Une revue transverse a déjà trouvé **vingt-sept** affirmations devenues fausses en une branche.

- [ ] **Step 1: Relever les tailles APRÈS la dernière édition**

```bash
cd /home/mallanic/Projects/Nivuus/packages/desk
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
wc -l client/src/hub/page.ts client/src/bureau/*.ts client/src/shell.ts
```
⚠️ **Relever APRÈS la dernière édition de la ronde, revue transverse comprise** : une table mesurée en début de ronde est fausse à la fin de la même ronde. **Corriger le tableau de dette de `CLAUDE.md` dans le même mouvement.**

- [ ] **Step 2: Chercher par le SENS les affirmations devenues fausses**

Six formulations à chercher, pas une :
```bash
grep -rn 'seule page qui traite\|page-shell\|seconde surface\|Mon bureau\|nivuus-bureau\|PAGE_DU_BUREAU' \
  client/src client/*.html plateforme/src CLAUDE.md --include='*.ts' --include='*.html' --include='*.md' | grep -v node_modules
grep -rn 'assurerAcces\b' client/src --include='*.ts'
```
🔴 **Une négation se dit de plusieurs façons, et c'est celle qu'on n'a pas listée qui survit.** Corriger chaque occurrence, puis **les relire une par une** — pas seulement les remplacer.

- [ ] **Step 3: Jouer la suite complète, depuis un shell PROPRE**

```bash
cd /home/mallanic/Projects/Nivuus/packages/desk
env -u TURN_URL -u TURN_SECRET ./scripts/verify-all.sh
```
🔴 **`verify-all.sh` n'est PAS hermétique** : avec `TURN_URL`/`TURN_SECRET` dans l'environnement, six tests de signaling échouent. Attendu : toutes les étapes vertes.

- [ ] **Step 4: Écrire le document de résultats**

Créer `docs/superpowers/plans/2026-08-31-navigation-hub-unique-resultats.md`, avec :
- ce qui a été fait, tâche par tâche ;
- **les rouges vues**, et laquelle a rougi (pas seulement « vue rouge ») ;
- 🔴 **un § « Ce que ce lot n'établit PAS »** reprenant le §10 de la spec, **plus** ce qui aura été découvert en chemin ;
- les legs neufs, **nommés**.

🔴 **UNE PREUVE NE DOIT JAMAIS VIVRE DANS UN RAPPORT GITIGNORÉ** : ce dépôt a perdu six constats de revue ainsi. Ce document est **versionné**.

- [ ] **Step 5: Ajouter UNE ligne à l'index des chantiers de `CLAUDE.md`**

Dans la section « Chantier package-nivuus », une ligne renvoyant au document de résultats. **Une ligne à l'index, le récit au journal — jamais l'inverse.** Y nommer les deux limites qui comptent : aucune recette navigateur jouée, et le suiveur qui ne prévient pas le porteur.

- [ ] **Step 6: Commit**

```bash
git add CLAUDE.md docs/superpowers/plans/2026-08-31-navigation-hub-unique-resultats.md
git commit -m "cloture(navigation) : le hub est la seule surface, et ce que ce lot n etablit PAS

Document de resultats VERSIONNE -- ce depot a perdu six constats de revue dans
un rapport gitignore -- et UNE ligne a l index de CLAUDE.md.

Ce que ce lot n etablit PAS : aucune recette navigateur n a ete jouee (le role
client est exclusif et le proprietaire est connecte, un pilote lui prendrait sa
place) ; plus de deux onglets n est mesure par rien ; la reconnexion du
WebSocket apres une coupure reseau reste due, legs du lot 34 ; le legs une
fenetre Vivante n est jamais redite n est pas ferme.

Le jugement d usage appartient au proprietaire, et rien ici ne le remplace.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```
