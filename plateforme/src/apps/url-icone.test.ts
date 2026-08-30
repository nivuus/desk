// La règle de l'URL signée d'icône, éprouvée SANS serveur ni base : le module
// est pur, son horloge est un paramètre, et c'est ce qui permet de l'assiéger
// des deux côtés de sa borne d'expiration.
//
// 🔴 LES TROIS ROUGES DE SÉCURITÉ DE CE LOT SONT ICI, ET ELLES ONT ÉTÉ VUES
// ROUGES : une signature falsifiée est refusée, une URL expirée est refusée,
// une URL signée pour une application ne vaut pas pour une autre. Un contrôle
// qu'on n'a jamais vu rouge n'est pas un contrôle.

import { createHmac } from 'node:crypto';
import { describe, expect, it } from 'vitest';
import { DUREE_JETON_ACCES_MS } from '../identite/jeton';
import {
    DUREE_URL_ICONE_MS,
    PAS_URL_ICONE_MS,
    signature,
    signerUrlIcone,
    sousCleIcone,
    verifierUrlIcone,
} from './url-icone';

/// La MÊME fixture publique que `http/routes-harnais.ts`, et pour la même
/// raison : elle ne protège rien, elle satisfait seulement la longueur
/// minimale d'un secret de plateforme.
const SECRET = 'un-secret-de-plateforme-de-quarante-octets';
const MS = 1_787_136_773_742;
const APP = '11111111-2222-3333-4444-555555555555';
const VM = 'vm-1';
const EMPREINTE = 'a'.repeat(64);

function parametresDe(url: string): URLSearchParams {
    return new URL(url, 'http://interne').searchParams;
}

describe("l'URL signée d'icône", () => {
    it('rend un chemin relatif, portant les quatre paramètres', () => {
        const url = signerUrlIcone(APP, VM, EMPREINTE, SECRET, MS);
        expect(url.startsWith(`/application/${APP}/icone?`)).toBe(true);
        const p = parametresDe(url);
        expect(p.get('e')).toBe(EMPREINTE);
        expect(p.get('v')).toBe(VM);
        // L'expiration est ARRONDIE au pas supérieur — voir `PAS_URL_ICONE_MS`.
        const x = Number(p.get('x'));
        expect(x % PAS_URL_ICONE_MS).toBe(0);
        expect(x).toBeGreaterThanOrEqual(MS + DUREE_URL_ICONE_MS);
        expect(x).toBeLessThan(MS + DUREE_URL_ICONE_MS + PAS_URL_ICONE_MS);
        expect(p.get('s')).toMatch(/^[A-Za-z0-9_-]{43}$/);
    });

    it('se vérifie, et rend la VM qu’elle porte', () => {
        const v = verifierUrlIcone(APP, parametresDe(signerUrlIcone(APP, VM, EMPREINTE, SECRET, MS)), SECRET, MS);
        expect(v).toEqual({ ok: true, vm: VM });
    });

    /* ── ROUGE ① : LA SIGNATURE FALSIFIÉE ─────────────────────────────── */

    it('🔴 REFUSE une signature falsifiée — un octet suffit', () => {
        const p = parametresDe(signerUrlIcone(APP, VM, EMPREINTE, SECRET, MS));
        const vraie = p.get('s')!;
        // Un SEUL caractère change, et il change vraiment : `a` -> `b`.
        const faux = (vraie[0] === 'a' ? 'b' : 'a') + vraie.slice(1);
        expect(faux).not.toBe(vraie);
        p.set('s', faux);
        expect(verifierUrlIcone(APP, p, SECRET, MS)).toEqual({
            ok: false,
            motif: 'signature-invalide',
        });
    });

    it('🔴 REFUSE une signature d’une AUTRE clé — la sous-clé n’est pas le secret', () => {
        // 🔴 C'EST LE CONTRÔLE QUI DIT QUE LA DÉRIVATION SERT À QUELQUE CHOSE.
        // Une signature calculée avec le secret de jeton BRUT — c'est-à-dire
        // ce qu'on aurait écrit sans dériver — doit être REFUSÉE. Sans lui,
        // remplacer `sousCleIcone(secret)` par `secret` laisserait tout vert.
        const expiration = String(MS + DUREE_URL_ICONE_MS);
        const message = ['v1', `${String(APP.length)}:${APP}`, `${String(VM.length)}:${VM}`,
            `${String(expiration.length)}:${expiration}`].join('\n');
        const brute = createHmac('sha256', SECRET).update(message, 'utf8').digest('base64url');
        const p = new URLSearchParams({ e: EMPREINTE, v: VM, x: expiration, s: brute });
        expect(verifierUrlIcone(APP, p, SECRET, MS)).toEqual({
            ok: false,
            motif: 'signature-invalide',
        });
        // Et le témoin POSITIF, sans lequel le refus ci-dessus ne prouverait
        // rien : la MÊME URL, signée par la sous-clé, est acceptée.
        p.set('s', signature({ application: APP, vm: VM, expiration }, SECRET));
        expect(verifierUrlIcone(APP, p, SECRET, MS).ok).toBe(true);
    });

    it('la sous-clé n’est PAS le secret, et elle est stable', () => {
        const a = sousCleIcone(SECRET);
        expect(a.length).toBe(32);
        expect(a.toString('utf8')).not.toBe(SECRET);
        expect(a.equals(sousCleIcone(SECRET))).toBe(true);
        expect(a.equals(sousCleIcone(`${SECRET}-autre`))).toBe(false);
    });

    /* ── ROUGE ② : L'URL EXPIRÉE ──────────────────────────────────────── */

    it('🔴 REFUSE une URL EXPIRÉE, et la borne est assiégée des DEUX côtés', () => {
        const p = parametresDe(signerUrlIcone(APP, VM, EMPREINTE, SECRET, MS));
        const x = Number(p.get('x'));
        // Une milliseconde avant : encore bonne.
        expect(verifierUrlIcone(APP, p, SECRET, x - 1).ok).toBe(true);
        // À l'instant EXACT : la borne est franche, l'URL est morte.
        expect(verifierUrlIcone(APP, p, SECRET, x)).toEqual({ ok: false, motif: 'url-expiree' });
        // Bien après : idem.
        expect(verifierUrlIcone(APP, p, SECRET, x + 3_600_000)).toEqual({
            ok: false,
            motif: 'url-expiree',
        });
    });

    it('🔴 une URL forgée ET périmée s’entend dire « signature », jamais « expirée »', () => {
        // 🔴 L'ORDRE DES CONTRÔLES EST UNE PROPRIÉTÉ DE SÉCURITÉ : dire
        // « expirée » à un faussaire lui apprendrait que sa signature était
        // bonne. Le contrôle de signature vient donc AVANT celui du temps.
        const p = parametresDe(signerUrlIcone(APP, VM, EMPREINTE, SECRET, MS));
        p.set('s', 'z'.repeat(43));
        expect(verifierUrlIcone(APP, p, SECRET, MS + 10_000_000)).toEqual({
            ok: false,
            motif: 'signature-invalide',
        });
    });

    it('🔴 une expiration NON ENTIÈRE, pourtant signée, est refusée', () => {
        // 🔴 `Number('x')` rend `NaN`, et `maintenant >= NaN` est FAUX : sans
        // ce garde, une expiration illisible serait ÉTERNELLE. Le seul chemin
        // qui l'atteigne est une signature calculée avec la VRAIE clé — donc
        // ce test la calcule, plutôt que de laisser le garde vert par
        // construction.
        for (const x of ['pas-un-nombre', '1.5', 'Infinity']) {
            const p = new URLSearchParams({
                e: EMPREINTE,
                v: VM,
                x,
                s: signature({ application: APP, vm: VM, expiration: x }, SECRET),
            });
            expect(verifierUrlIcone(APP, p, SECRET, MS), x).toEqual({
                ok: false,
                motif: 'signature-invalide',
            });
        }
    });

    /* ── ROUGE ③ : UNE URL NE VAUT QUE POUR SA PORTÉE ─────────────────── */

    it('🔴 une URL signée pour une application NE VAUT PAS pour une autre', () => {
        const p = parametresDe(signerUrlIcone(APP, VM, EMPREINTE, SECRET, MS));
        const autre = '99999999-8888-7777-6666-555555555555';
        expect(verifierUrlIcone(autre, p, SECRET, MS)).toEqual({
            ok: false,
            motif: 'signature-invalide',
        });
        // Témoin : la MÊME URL, sur SON application, est acceptée.
        expect(verifierUrlIcone(APP, p, SECRET, MS).ok).toBe(true);
    });

    it('🔴 une URL signée pour une VM NE VAUT PAS pour une autre', () => {
        const p = parametresDe(signerUrlIcone(APP, VM, EMPREINTE, SECRET, MS));
        p.set('v', 'vm-2');
        expect(verifierUrlIcone(APP, p, SECRET, MS)).toEqual({
            ok: false,
            motif: 'signature-invalide',
        });
    });

    it('🔴 les champs ne GLISSENT pas les uns dans les autres', () => {
        // 🔴 LE PRÉFIXE DE LONGUEUR EXISTE POUR CELA : sans lui, un simple
        // `join('\n')` ferait qu'une application nommée `x\n3:vm` et une VM
        // `abc` produiraient le même message qu'une autre paire. On éprouve
        // que les deux paires rendent des signatures DIFFÉRENTES.
        const a = signature({ application: 'x', vm: 'y|z', expiration: '1' }, SECRET);
        const b = signature({ application: 'x|y', vm: 'z', expiration: '1' }, SECRET);
        expect(a).not.toBe(b);
        const c = signature({ application: 'x\n3:abc', vm: 'q', expiration: '1' }, SECRET);
        const d = signature({ application: 'x', vm: 'abc', expiration: '1' }, SECRET);
        expect(c).not.toBe(d);
    });

    /* ── LA FORME ─────────────────────────────────────────────────────── */

    it('refuse un paramètre absent ou vide, sans le confondre avec une signature fausse', () => {
        const base = parametresDe(signerUrlIcone(APP, VM, EMPREINTE, SECRET, MS));
        for (const nom of ['v', 'x', 's']) {
            const manque = new URLSearchParams(base);
            manque.delete(nom);
            expect(verifierUrlIcone(APP, manque, SECRET, MS), `sans ${nom}`).toEqual({
                ok: false,
                motif: 'parametre-absent',
            });
            const vide = new URLSearchParams(base);
            vide.set(nom, '');
            expect(verifierUrlIcone(APP, vide, SECRET, MS), `${nom} vide`).toEqual({
                ok: false,
                motif: 'parametre-absent',
            });
        }
    });

    /* ── LA DURÉE ─────────────────────────────────────────────────────── */

    it('🔴 deux frappes de la MÊME minute rendent la MÊME URL — sinon le cache est mort', () => {
        // 🔴 C'EST LA PROPRIÉTÉ QUI SAUVE `Cache-Control: immutable`. Sans
        // l'arrondi, `x` et `s` changeraient à chaque frappe, la clé de cache
        // aussi, et aucune entrée ne serait jamais relue.
        const a = signerUrlIcone(APP, VM, EMPREINTE, SECRET, MS);
        const b = signerUrlIcone(APP, VM, EMPREINTE, SECRET, MS + 1_000);
        expect(b).toBe(a);
        // Et le témoin NÉGATIF, sans lequel l'égalité ci-dessus pourrait
        // venir d'une horloge ignorée : un pas plus loin, l'URL DIFFÈRE.
        expect(signerUrlIcone(APP, VM, EMPREINTE, SECRET, MS + PAS_URL_ICONE_MS)).not.toBe(a);
    });

    it('🔴 le PLANCHER de durée est garanti, sur tout un pas', () => {
        // L'arrondi est vers le HAUT : à aucun instant du pas la durée de vie
        // ne descend sous `DUREE_URL_ICONE_MS`.
        for (let d = 0; d < PAS_URL_ICONE_MS; d += 997) {
            const t = MS + d;
            const x = Number(parametresDe(signerUrlIcone(APP, VM, EMPREINTE, SECRET, t)).get('x'));
            expect(x - t, `à +${String(d)} ms`).toBeGreaterThanOrEqual(DUREE_URL_ICONE_MS);
            expect(x - t, `à +${String(d)} ms`).toBeLessThan(
                DUREE_URL_ICONE_MS + PAS_URL_ICONE_MS,
            );
        }
    });

    it('🔴 ne survit JAMAIS au jeton porteur qui l’a fait naître', () => {
        // 🔴 C'EST LA RAISON ÉCRITE DE LA VALEUR, ÉPINGLÉE PLUTÔT QUE LAISSÉE
        // DANS UN COMMENTAIRE. Les deux constantes se recalibrent ENSEMBLE :
        // baisser le jeton d'accès sous cinq minutes rendrait la borne fausse,
        // et sans ce test personne ne le verrait.
        expect(DUREE_URL_ICONE_MS).toBeLessThanOrEqual(DUREE_JETON_ACCES_MS);
        expect(DUREE_URL_ICONE_MS).toBe(300_000);
    });
});
