/// <reference types="vite/client" />
/**
 * LE RENDU DE LA GALERIE — et le PREMIER appelant de `getComputedStyle` du
 * dépôt.
 *
 * ⚠️ CE MODULE N'EST PAS DU PRODUIT : il ne sert que `client/design.html`,
 * l'instrument du jugement humain du §8. Aucune surface du produit ne
 * l'importe, et il n'a pas de test — ce qu'il pourrait avoir de testable
 * (parser des tokens) vit déjà dans `tokens.ts`, qui est testé.
 *
 * ═══════════════════════════════════════════════════════════════════════════
 * 🔴 LA LISTE DES TOKENS N'EST PAS ÉCRITE ICI : elle est PARSÉE de
 * `tokens.css`, par le même `lireBlocsDeTheme` que les contrôles §7.1, §7.4 et
 * §7.6. C'est le point de conception du §7.1 — « un contrôle qui a sa propre
 * copie des valeurs valide sa copie » — appliqué à un instrument plutôt qu'à
 * un contrôle : une galerie avec sa propre liste montrerait sa liste, et un
 * token neuf n'y apparaîtrait jamais. C'est aussi ce qui rend vraie la phrase
 * du §7.6 « la galerie rend tous les tokens PAR CONSTRUCTION », sur laquelle
 * repose son exclusion du périmètre « employé ».
 *
 * 🔴 ET C'EST LE PREMIER APPELANT DE `getComputedStyle` — la spec §4.1
 * écrivait la règle « pour le premier qui en aura besoin, pas pour un besoin
 * constaté », et déclarait qu'« aucun appelant n'existe aujourd'hui ». Ce
 * fichier la rend fausse, et il s'y conforme : la valeur est lue par
 * `getComputedStyle(document.documentElement).getPropertyValue('--…')`, un
 * seul mécanisme, zéro duplication. Le contrat du §4.1 est qu'elle n'est faite
 * QU'À UN CHANGEMENT DE THÈME, jamais par image : c'est exactement ce que fait
 * `rendre()`, appelée au chargement et à chaque clic de thème, et jamais
 * ailleurs.
 * ═══════════════════════════════════════════════════════════════════════════
 */
import { lireBlocsDeTheme } from './tokens';
import { CLE_THEME, appliquer, choisir, surStockageModifie, themeStocke } from './theme';
import type { Theme } from './theme';
import tokensCss from './tokens.css?raw';

const racine = document.documentElement;

/** Tous les noms déclarés, dans l'ordre du fichier source. */
const NOMS: string[] = [];
for (const bloc of lireBlocsDeTheme(tokensCss)) {
    for (const nom of bloc.tokens.keys()) if (!NOMS.includes(nom)) NOMS.push(nom);
}

/** La valeur COURANTE, donc celle du thème appliqué — voir l'en-tête. */
const valeur = (nom: string) => getComputedStyle(racine).getPropertyValue(nom).trim();

const famille = (prefixe: string) => NOMS.filter((n) => n.startsWith(prefixe));
const dans = (noms: string[]) => NOMS.filter((n) => noms.includes(n));

const COULEURS = [
    '--fond-0', '--fond-1', '--fond-2', '--bord', '--bord-fort',
    '--texte-fort', '--texte', '--texte-faible',
    '--accent', '--sur-accent', '--succes', '--alerte', '--danger',
];

function vide(id: string): HTMLElement {
    const hote = document.getElementById(id);
    if (!hote) throw new Error(`la galerie attend un élément #${id}`);
    hote.textContent = '';
    return hote;
}

/** Une pastille : un échantillon, le nom, la valeur lue à la source. */
function pastille(hote: HTMLElement, nom: string, style: Partial<CSSStyleDeclaration>): void {
    const carte = document.createElement('div');
    carte.className = 'pastille';
    const echantillon = document.createElement('div');
    echantillon.className = 'echantillon';
    Object.assign(echantillon.style, style);
    const etiquette = document.createElement('code');
    etiquette.className = 'nom';
    etiquette.textContent = nom;
    const val = document.createElement('span');
    val.className = 'valeur';
    val.textContent = valeur(nom);
    carte.append(echantillon, etiquette, val);
    hote.append(carte);
}

/** Une ligne « nom du token — démonstration ». */
function ligne(hote: HTMLElement, nom: string, demo: HTMLElement): void {
    const rangee = document.createElement('div');
    rangee.className = 'ligne';
    const etiquette = document.createElement('code');
    etiquette.textContent = `${nom} ${valeur(nom)}`;
    rangee.append(etiquette, demo);
    hote.append(rangee);
}

function texte(contenu: string, style: Partial<CSSStyleDeclaration>): HTMLElement {
    const span = document.createElement('span');
    span.textContent = contenu;
    Object.assign(span.style, style);
    return span;
}

function rendre(): void {
    const couleurs = vide('couleurs');
    for (const nom of dans(COULEURS)) pastille(couleurs, nom, { background: `var(${nom})` });

    const voiles = vide('voiles');
    for (const nom of [...famille('--video-'), ...famille('--voile-')]) {
        pastille(voiles, nom, { background: `var(${nom})` });
    }

    const typo = vide('typo');
    for (const nom of famille('--t-')) {
        ligne(typo, nom, texte('Session distante — Aa Éé 0123', { fontSize: `var(${nom})` }));
    }

    const interlignes = vide('interlignes');
    for (const nom of famille('--lh-')) {
        const bloc = document.createElement('span');
        bloc.textContent =
            'Deux lignes suffisent à voir un interligne : celle-ci est écrite assez longue pour se replier au moins une fois dans la colonne de la galerie.';
        bloc.style.lineHeight = `var(${nom})`;
        bloc.style.display = 'block';
        ligne(interlignes, nom, bloc);
    }

    const espacement = vide('espacement');
    for (const nom of famille('--e-')) {
        const barre = document.createElement('span');
        barre.className = 'barre';
        barre.style.width = `var(${nom})`;
        barre.style.display = 'inline-block';
        ligne(espacement, nom, barre);
    }

    const rayons = vide('rayons');
    for (const nom of famille('--r-')) {
        pastille(rayons, nom, { background: 'var(--fond-2)', borderRadius: `var(${nom})` });
    }

    const traits = vide('traits');
    for (const nom of famille('--trait')) {
        const echantillon = document.createElement('span');
        echantillon.style.display = 'inline-block';
        echantillon.style.width = 'var(--e-8)';
        echantillon.style.borderBlockEnd = `var(${nom}) solid var(--accent)`;
        ligne(traits, nom, echantillon);
    }

    const polices = vide('polices');
    for (const nom of famille('--police-')) {
        ligne(polices, nom, texte('Session distante — Aa Éé 0123', { fontFamily: `var(${nom})` }));
    }
}

/**
 * Les trois boutons de thème — le PREMIER ET SEUL appelant de `choisir()` du
 * sous-bloc S1, et il vit sur une page qui n'est pas le produit. Le sélecteur
 * de thème du produit appartient à S3.
 */
function boutons(): void {
    const hote = vide('themes');
    const etats: Theme[] = ['systeme', 'clair', 'sombre'];
    const marquer = () => {
        const courant = themeStocke(localStorage);
        for (const bouton of hote.querySelectorAll('button')) {
            bouton.setAttribute('aria-pressed', String(bouton.dataset.theme === courant));
        }
    };
    for (const etat of etats) {
        const bouton = document.createElement('button');
        bouton.type = 'button';
        bouton.dataset.theme = etat;
        bouton.textContent = etat;
        bouton.addEventListener('click', () => {
            choisir(localStorage, racine, etat);
            marquer();
            rendre();
        });
        hote.append(bouton);
    }
    marquer();
}

// L'amorce a déjà posé l'attribut avant la première peinture ; ce rappel couvre
// le cas où le stockage a changé entre l'amorce et l'exécution de ce module.
appliquer(racine, themeStocke(localStorage));

// La bascule venue d'une AUTRE fenêtre — c'est la moitié que `choisir()` ne
// peut pas couvrir, `storage` ne se déclenchant jamais chez l'écrivain.
window.addEventListener('storage', (evenement) => {
    if (evenement.key !== CLE_THEME) return;
    surStockageModifie(racine, evenement.key, evenement.newValue);
    rendre();
});

boutons();
rendre();
