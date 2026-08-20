// Le balayage « aucun secret dans un fichier versionné ».
//
// 🔴 POURQUOI LES NOMS ET NON LES VALEURS. Chercher des VALEURS est
// indécidable — un secret est une chaîne quelconque, et rien ne la distingue
// d'un identifiant de test. Chercher une AFFECTATION D'UN NOM CONNU est
// décidable, et c'est exactement le geste qu'un humain fait par inadvertance :
// coller la valeur de `TURN_SECRET` dans un journal, un plan, ou un fichier de
// composition.
//
// 🔴 CE TEST NE DOIT JAMAIS IMPRIMER UNE VALEUR. Son message d'échec nomme le
// FICHIER, la LIGNE et le NOM — jamais ce qui suit le signe égal. Un test de
// sécurité qui recopierait le secret dans la sortie de la suite de tests
// l'écrirait dans tous les journaux de CI qui la capturent, et le fuirait par
// la porte qu'il gardait. C'est la même règle que
// `docker compose … config`, dont le plan de P5 relève qu'il imprime
// `--static-auth-secret` en clair.
//
// ⚠️ IL BALAIE `git ls-files` À LA RACINE DU DÉPÔT, `docs/` COMPRIS. C'est
// délibéré et c'est même le cas le plus probable : les journaux de recette
// sont ce qu'on verse le plus vite et ce qu'on relit le moins.

import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFileSync, statSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const RACINE = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');

/// Les noms dont une affectation littérale est un secret fuité.
const NOMS = [
    'TURN_SECRET',
    'PLATEFORME_SECRET_JETON',
    'AGENT_SECRET',
    'POSTGRES_PASSWORD',
    'WINDOWS_PASSWORD',
    'WINDOWS_ADMIN_PASSWORD',
    'RECETTE_MOTDEPASSE',
] as const;

/// L'empreinte d'une valeur — jamais la valeur.
///
/// 🔴 C'EST CE QUI REND UNE EXCEPTION HONNÊTE. Une exception nommée seulement
/// par `<fichier>:<NOM>` autoriserait N'IMPORTE QUELLE valeur future à cet
/// endroit : le jour où quelqu'un remplacerait la fixture de recette par un
/// vrai secret, l'exception le couvrirait EN SILENCE. En épinglant
/// l'empreinte, tout changement de valeur fait ROUGIR le test, et il faut
/// alors regarder.
///
/// ⚠️ ELLE NE SERT PAS À CACHER, ET IL FAUT LE DIRE : les valeurs exemptées
/// ci-dessous sont des fixtures publiques, et plusieurs de leurs empreintes
/// sont devinables en une seconde (`ba7816bf8f01cfea` est le SHA-256 de
/// « abc »). L'empreinte sert à DÉTECTER UN CHANGEMENT, pas à protéger un
/// secret — et c'est exactement ce qu'on lui demande, puisqu'aucune valeur
/// exemptée n'est un secret.
function empreinte(valeur: string): string {
    return createHash('sha256').update(valeur, 'utf8').digest('hex').slice(0, 16);
}

interface Exception {
    fichier: string;
    nom: string;
    /// L'empreinte de la valeur AUTORISÉE, et d'elle seule.
    empreinte: string;
    raison: string;
}

/// Les exceptions, CHACUNE AVEC SA RAISON ÉCRITE — sur le patron de
/// `DRAPEAUX_INTERDITS` d'`admin/enroler-agent.ts`.
///
/// ⚠️ AUCUNE N'EST UN SECRET RÉEL. Le balayage a été joué sur l'état du dépôt
/// au 20 août 2026, et les treize affectations qu'il a d'abord dénoncées ont
/// été LUES UNE PAR UNE : deux étaient des faux positifs de l'expression
/// régulière (l'idiome `${VAR:?message}`, corrigé depuis), et les onze autres
/// sont des fixtures — de test, de recette, ou des exemples de documentation.
/// Aucun secret de production n'a jamais été versionné dans ce dépôt.
const EXCEPTIONS: readonly Exception[] = [
    {
        fichier: 'docker-compose.plateforme.yml',
        nom: 'POSTGRES_PASSWORD',
        empreinte: '3b132f52b3b4ad4d',
        raison:
            "L'instance Postgres de TEST, jetable, dont l'en-tête du fichier déclare " +
            "en toutes lettres qu'« une base de production n'emploiera JAMAIS ce " +
            'fichier ». La valeur est la même pour tout le monde et ne protège rien.',
    },
    {
        fichier: 'docs/superpowers/plans/2026-08-19-plateforme-p5.md',
        nom: 'POSTGRES_PASSWORD',
        empreinte: '921c3773f8e7b716',
        raison:
            'Le plan de P5 CITE la ligne du fichier de composition ci-dessus, pour ' +
            "expliquer pourquoi elle est exemptée. C'est la même valeur de fixture, " +
            'recopiée dans une phrase.',
    },
    {
        fichier: 'docs/superpowers/plans/2026-07-27-jalon1-tranche-verticale.md',
        nom: 'WINDOWS_ADMIN_PASSWORD',
        empreinte: 'ab5df625bc76dbd4',
        raison:
            "Un exemple de ligne de commande dont la valeur est littéralement « ... » : " +
            "c'est un OBJET DE REMPLACEMENT que le lecteur doit substituer, pas une " +
            'valeur.',
    },
    {
        fichier: 'docs/superpowers/plans/2026-07-29-traversee-nat.md',
        nom: 'TURN_SECRET',
        empreinte: '2bb80d537b1da3e3',
        raison:
            "Une fixture de test recopiée dans le plan : la valeur est le mot " +
            '« secret » lui-même, et le même littéral vit dans `signaling/ice.test.ts`.',
    },
    {
        fichier: 'plateforme/src/signaling/ice.test.ts',
        nom: 'TURN_SECRET',
        empreinte: '2bb80d537b1da3e3',
        raison:
            'La fixture du test de `configurationIce` : la valeur est le mot ' +
            "« secret ». Un vrai secret y serait inutile — le test vérifie la FORME " +
            "de l'identifiant dérivé, pas sa résistance.",
    },
    {
        fichier: 'docs/superpowers/plans/2026-08-19-plateforme-p3.md',
        nom: 'AGENT_SECRET',
        empreinte: 'ba7816bf8f01cfea',
        raison:
            "Une valeur de trois lettres dans une commande de contrôle de SYNTAXE " +
            "(`bash -n`) : le script n'est jamais exécuté, et la valeur n'atteint " +
            'aucune VM.',
    },
    {
        fichier: 'docs/superpowers/plans/journaux-corrections/instrument/compter-enrolements.sh',
        nom: 'PLATEFORME_SECRET_JETON',
        empreinte: '6b82a0dca0d6fa4d',
        raison:
            "Le secret de signature d'un instrument de RECETTE, tiré pour cette " +
            "recette-là et mort avec elle. Il ne signe aucun jeton d'un service vivant.",
    },
    {
        fichier: 'docs/superpowers/plans/journaux-plateforme-p3/instrument/jouer.sh',
        nom: 'TURN_SECRET',
        empreinte: '20e73cf9ccbda64a',
        raison:
            "Le secret TURN d'un instrument de recette P3, nommé « recette-p3-… » " +
            "précisément pour qu'on ne le confonde pas avec celui du relais réel.",
    },
    {
        fichier: 'docs/superpowers/plans/journaux-plateforme-p3/instrument/rouge-1a.ts',
        nom: 'PLATEFORME_SECRET_JETON',
        empreinte: '2923f1439452d95c',
        raison:
            'Idem : la fixture de signature de la rouge ①A de P3, nommée ' +
            '« recette-p3-… », morte avec sa recette.',
    },
    {
        fichier: 'plateforme/src/config.test.ts',
        nom: 'PLATEFORME_SECRET_JETON',
        empreinte: '94e4c4bc7d176bd9',
        raison:
            "La valeur est littéralement « trop-court » : c'est le test qui vérifie " +
            "que `lireConfig` REFUSE un secret sous `LONGUEUR_SECRET_MIN`.",
    },
];

/// Une valeur est INOFFENSIVE si elle ne peut pas être un secret collé.
///
/// ⚠️ CHAQUE BRANCHE EST UN TROU POTENTIEL, et c'est pourquoi elles sont
/// énumérées ici plutôt que noyées dans une expression régulière : un
/// successeur qui en ajoutera une devra écrire pourquoi.
function inoffensive(valeur: string): boolean {
    // Vide : `TURN_SECRET=` ne porte rien.
    if (valeur === '') return true;
    // Interpolation shell, docker compose ou Windows : la valeur vient
    // d'ailleurs, et « ailleurs » n'est pas versionné.
    if (/^[$%]/.test(valeur)) return true;
    // Une SUBSTITUTION de commande : `$(openssl rand -hex 32)`.
    if (valeur.startsWith('(')) return true;
    // Un IDENTIFIANT de code, en majuscules : `PLATEFORME_SECRET_JETON: SECRET`
    // désigne une constante TypeScript, pas une valeur. Un secret réel en
    // majuscules pures et sans chiffre minuscule serait un secret risible.
    if (/^[A-Z][A-Z0-9_]*$/.test(valeur)) return true;
    // Un OBJET de remplacement explicite : `<généré>`, `<votre secret>`.
    if (valeur.startsWith('<')) return true;
    return false;
}

interface Trouvaille {
    fichier: string;
    ligne: number;
    nom: string;
    /// L'empreinte de la valeur — jamais la valeur.
    empreinte: string;
}

/// Extrait la valeur affectée, sans jamais la rendre à l'appelant autrement
/// que pour la classer.
function valeurApres(reste: string): string {
    const t = reste.trimStart();
    // Une chaîne citée : on lit jusqu'au guillemet fermant.
    const cite = /^(['"`])(.*?)\1/.exec(t);
    if (cite) return cite[2];
    // Sinon, jusqu'au premier séparateur.
    return /^[^\s,;)}\]]*/.exec(t)?.[0] ?? '';
}

function balayer(): Trouvaille[] {
    const fichiers = execFileSync('git', ['ls-files', '-z'], {
        cwd: RACINE,
        maxBuffer: 64 * 1024 * 1024,
    })
        .toString('utf8')
        .split('\0')
        .filter((f) => f !== '');

    // `(?:=(?!=))` : `===` et `==` sont des COMPARAISONS, pas des
    // affectations. Sans cette garde, `AGENT_SECRET === x` serait dénoncé.
    // `(?<!\$)\{` : dans `${VAR:?message}` ou `${VAR:+…}`, le `:` fait partie
    // d'une INTERPOLATION, pas d'une affectation. Sans cette garde, l'idiome
    // `\${VAR:?message}` — celui-là même que le déploiement emploie pour rendre
    // une variable obligatoire — serait dénoncé comme un secret. MESURÉ : il
    // produisait 3 des 13 premières trouvailles.
    //
    // `(?:=(?!=))` : `===` et `==` sont des COMPARAISONS. Sans cette garde,
    // `AGENT_SECRET === x` serait dénoncé.
    const motif = new RegExp(
        `(?:^|[\\s"'\`(,;]|(?<!\\$)\\{)(${NOMS.join('|')})\\s*(?::|=(?!=))(.*)$`,
    );

    const trouvailles: Trouvaille[] = [];
    for (const fichier of fichiers) {
        const chemin = path.join(RACINE, fichier);
        let taille: number;
        try {
            taille = statSync(chemin).size;
        } catch {
            // Un fichier suivi mais absent du disque (suppression non
            // commitée) : il n'y a rien à lire, et ce n'est pas notre sujet.
            continue;
        }
        // Les binaires volumineux n'ont pas d'affectation lisible, et les lire
        // coûterait sans rien apprendre.
        if (taille > 4 * 1024 * 1024) continue;
        let contenu: string;
        try {
            contenu = readFileSync(chemin, 'utf8');
        } catch {
            continue;
        }
        if (!NOMS.some((n) => contenu.includes(n))) continue;

        const lignes = contenu.split('\n');
        for (let i = 0; i < lignes.length; i++) {
            const m = motif.exec(lignes[i]);
            if (!m) continue;
            const valeur = valeurApres(m[2]);
            if (inoffensive(valeur)) continue;
            trouvailles.push({
                fichier,
                ligne: i + 1,
                nom: m[1],
                empreinte: empreinte(valeur),
            });
        }
    }
    return trouvailles;
}

describe('aucun secret dans un fichier versionné', () => {
    it('🔴 ne trouve aucune affectation littérale hors des exceptions déclarées', () => {
        const trouvailles = balayer();
        const autorisees = new Set(
            EXCEPTIONS.map((e) => `${e.fichier}:${e.nom}:${e.empreinte}`),
        );
        const hors = trouvailles.filter(
            (t) => !autorisees.has(`${t.fichier}:${t.nom}:${t.empreinte}`),
        );
        // ⚠️ LE MESSAGE NOMME LE FICHIER, LA LIGNE ET LE NOM — JAMAIS LA
        // VALEUR. Voir l'en-tête : un test de sécurité qui imprimerait le
        // secret le fuirait par la porte qu'il garde.
        expect(
            hors.map((t) => `${t.fichier}:${t.ligne} affecte ${t.nom} [empreinte ${t.empreinte}]`),
            "des secrets sont affectés en clair dans des fichiers versionnés " +
                "(la valeur n'est volontairement pas affichée)",
        ).toEqual([]);
    });

    it('chaque exception porte une RAISON non vide', () => {
        // Sans cette assertion, la liste deviendrait en deux chantiers une
        // liste de choses qu'on a renoncé à comprendre.
        for (const e of EXCEPTIONS) {
            expect(e.raison.trim().length, `l'exception ${e.fichier}:${e.nom} n'a pas de raison`)
                .toBeGreaterThan(30);
            expect(e.empreinte, `l'exception ${e.fichier}:${e.nom} n'épingle aucune valeur`)
                .toMatch(/^[0-9a-f]{16}$/);
        }
    });

    it('🔴 chaque exception correspond à une trouvaille RÉELLE', () => {
        // 🔴 UNE EXCEPTION QUI NE COUVRE PLUS RIEN EST UN MENSONGE QUI DORT :
        // le jour où le fichier change, elle continue d'autoriser un chemin
        // que personne ne relit. Cette assertion la fait tomber le jour où
        // elle cesse d'être nécessaire.
        const reelles = new Set(
            balayer().map((t) => `${t.fichier}:${t.nom}:${t.empreinte}`),
        );
        for (const e of EXCEPTIONS) {
            const cle = `${e.fichier}:${e.nom}:${e.empreinte}`;
            expect(reelles.has(cle), `l'exception ${e.fichier}:${e.nom} ne couvre plus rien`)
                .toBe(true);
        }
    });

    it('le balayage voit RÉELLEMENT des fichiers — il ne peut pas être vide par accident', () => {
        // ⚠️ CONTRÔLE DU CONTRÔLE. Un `git ls-files` qui rendrait zéro fichier
        // — mauvais `cwd`, dépôt absent — ferait passer le test principal en
        // vert sans avoir rien balayé. C'est le patron du contrôle vacueux,
        // que ce dépôt a payé quatre fois.
        const fichiers = execFileSync('git', ['ls-files', '-z'], {
            cwd: RACINE,
            maxBuffer: 64 * 1024 * 1024,
        })
            .toString('utf8')
            .split('\0')
            .filter((f) => f !== '');
        expect(fichiers.length).toBeGreaterThan(500);
        expect(fichiers).toContain('docker-compose.plateforme.yml');
    });
});
