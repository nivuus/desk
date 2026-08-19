// `InventaireStatique` : l'unique implémentation d'`Orchestrateur` de la v1.
//
// 🔴 CE QUE « STATIQUE » VEUT DIRE, ET CE QU'IL NE VEUT PAS DIRE. La spec §3.6
// l'annonçait « alimenté par un fichier de configuration déclaratif » ; ce
// backend lit LA BASE (divergence E1, décision D1). Les trois champs qu'un tel
// fichier aurait portés — nom, adresse, empreinte du secret d'enrôlement —
// sont déjà écrits en base par `admin/enroler-agent.ts`, et deux sources de
// vérité pour la même chose divergent en silence : c'est celle que personne ne
// lit qui gagne, le jour où l'on s'y fie.
//
// « Statique » signifie donc : IL NE PILOTE AUCUN HYPERVISEUR. Il n'allume
// rien, n'éteint rien, ne photographie rien, ne crée aucune machine. Son
// inventaire est ce qu'un administrateur a enrôlé, et il ne peut agir sur le
// monde qu'à travers la seule colonne dont il soit propriétaire :
// `vm.utilisateur_id`. Cette lecture est PLUS FORTE que « fichier
// rechargeable » — un fichier aurait pu être rechargé, ce qui aurait laissé
// croire à une forme de gestion.
//
// 🔴 L'HORLOGE EST INJECTÉE (`maintenant`), jamais `Date.now()` lu ici. C'est
// ce qui rend la borne de `fraicheur.etatDe` assiégeable des deux côtés.

import type { Pilote } from '../base/pilote';
import { etatDe } from '../agents/fraicheur';
import { attribuerSiLibre, lister as listerLignes, type LigneVm } from '../depot/vm';
import type { EtatVm, Operation, Orchestrateur, Vm } from './interface';
import { BACKEND_STATIQUE, refuser, type Resultat } from './refus';
import { vmsDe } from './selection';

/// Convertit une ligne de dépôt en `Vm`.
///
/// ⚠️ DEUX RENOMMAGES, DONC DEUX OCCASIONS DE RENDRE `undefined` EN SILENCE :
/// `prefixe_session` -> `prefixe` et `vu_a` -> `vuA`. Et `undefined` n'est PAS
/// `null` — `etatDe` distingue le second (une VM jamais vue) de ce qui serait
/// une erreur de conversion. Le test de `lister` compare l'objet ENTIER.
function enVm(l: LigneVm): Vm {
    return {
        id: l.id,
        nom: l.nom,
        adresse: l.adresse,
        utilisateurId: l.utilisateur_id,
        prefixe: l.prefixe_session,
        vuA: l.vu_a,
    };
}

async function inventaireDe(p: Pilote): Promise<Vm[]> {
    return (await listerLignes(p)).map(enVm);
}

/// Refuse une opération que ce backend ne sait pas faire, ET l'écrit au
/// journal.
///
/// ⚠️ LES TROIS VERBES D'ACTION PARTAGENT CETTE SEULE FONCTION, et il faut le
/// dire : un test sur `instantane` éprouve donc la même ligne qu'un test sur
/// `demarrer`. Chacun des trois a néanmoins son propre `it()` — non pour
/// éprouver trois lignes différentes, mais pour qu'un verbe qui cesserait un
/// jour de passer par ici se voie.
///
/// 🔴 LE JOURNAL EST UN `warn!`, PAS UN `debug`. Un backend qui refuse
/// silencieusement une opération que l'exploitant croit avoir déclenchée est
/// une panne muette ; l'exploitation tourne en niveau ordinaire, et une
/// mitigation muette n'en est pas une.
function refuserNonSupporte(operation: Operation): Resultat {
    console.warn(
        `opération refusée : ${operation} n'est pas supportée par le backend ` +
            `${BACKEND_STATIQUE}, qui ne pilote aucun hyperviseur — il inventorie ce ` +
            "qu'un administrateur a enrôlé, et n'écrit que vm.utilisateur_id.",
    );
    return refuser('non-supporte', operation);
}

export function inventaireStatique(base: Pilote, maintenant: () => number): Orchestrateur {
    return {
        async lister(): Promise<Vm[]> {
            return inventaireDe(base);
        },

        async etat(vm: string): Promise<EtatVm> {
            const inventaire = await inventaireDe(base);
            const cible = inventaire.find((v) => v.id === vm);
            // 🔴 UNE VM INCONNUE REND `injoignable`, JAMAIS UNE EXCEPTION.
            // C'est vrai — on n'en sait rien, donc on ne peut pas s'en servir —
            // et c'est déjà ce que `agents/fraicheur.ts` dit d'une VM jamais
            // vue. Une exception remonterait en 500 là où il n'y a rien
            // d'anormal, et serait sur une route un oracle d'énumération.
            if (cible === undefined) return 'injoignable';
            // 🔴 C'EST L'APPELANT DE PRODUCTION QUE `agents/fraicheur.ts`
            // DÉCLARE ATTENDRE DEPUIS P3, et il ferme le legs n°2 de ce
            // sous-bloc : « il n'a aucun appelant de production dans P3, et
            // c'est déclaré plutôt que dissimulé […] ce qui est le sujet de
            // P4 ».
            return etatDe(cible.vuA, maintenant());
        },

        async demarrer(vm: string): Promise<Resultat> {
            // 🔴 CE REFUS N'EST PAS UNE COMMODITÉ : c'est le contenu du
            // critère ④. Le cadrage promet « VM injoignable -> le hub
            // l'indique, PROPOSE REDÉMARRAGE ». Avec ce backend, le hub
            // INDIQUE et dit qu'il ne peut PAS redémarrer. Ce que P4 livre est
            // l'aveu, pas la fonction — et la spec §3.6 le nomme « conséquence
            // produit à assumer ».
            void vm;
            return refuserNonSupporte('demarrer');
        },

        async arreter(vm: string): Promise<Resultat> {
            void vm;
            return refuserNonSupporte('arreter');
        },

        async instantane(vm: string, nom: string): Promise<Resultat> {
            void vm;
            void nom;
            return refuserNonSupporte('instantane');
        },

        async attribuer(vm: string, utilisateur: string): Promise<Resultat> {
            try {
                return await base.transaction(async (t) => {
                    // ① LIRE D'ABORD — et c'est pour NOMMER le bon motif,
                    // jamais pour garantir quoi que ce soit. `changes = 0`
                    // confond TROIS causes (VM inconnue, VM déjà prise,
                    // ré-attribution au même utilisateur, divergence E8) : un
                    // code qui déciderait sur ce seul nombre rendrait un refus
                    // qui n'informe pas.
                    const inventaire = await inventaireDe(t);
                    const cible = inventaire.find((v) => v.id === vm);
                    if (cible === undefined) return refuser('vm-inconnue', 'attribuer');
                    if (cible.utilisateurId !== null) {
                        return refuser('vm-deja-attribuee', 'attribuer');
                    }
                    if (vmsDe(inventaire, utilisateur).length > 0) {
                        return refuser('utilisateur-servi', 'attribuer');
                    }

                    // ② PUIS ÉCRIRE, SOUS LA CLAUSE CONDITIONNELLE. 🔴 C'est
                    // ELLE qui garantit, et non la lecture ci-dessus : elle est
                    // RÉÉVALUÉE PAR LE MOTEUR au moment de l'écriture. Un code
                    // qui lirait puis écrirait sans clause serait juste dans les
                    // tests et faux en production, et le test séquentiel ne le
                    // verrait pas. Mesuré sur PostgreSQL 16.15 : de deux
                    // transactions visant la même VM libre, la seconde bloque
                    // puis rend `0 ligne` — exactement un gagnant.
                    const lignes = await attribuerSiLibre(t, vm, utilisateur);
                    // Zéro ligne ICI ne peut plus être qu'une chose : la course
                    // a été perdue entre la lecture et l'écriture, les deux
                    // autres causes ayant déjà été écartées.
                    if (lignes === 0) return refuser('vm-deja-attribuee', 'attribuer');
                    return { ok: true };
                });
            } catch (cause) {
                // ③ ET SI L'ÉCRITURE A LEVÉ : relire, et ne traduire QUE ce que
                // la relecture explique.
                //
                // 🔴 C'EST LE SEUL ENDROIT DU SERVICE OÙ UNE EXCEPTION EST
                // RATTRAPÉE, et il ne doit pas devenir un `catch` muet. Un
                // `catch` qui traduirait TOUTE exception en `utilisateur-servi`
                // avalerait une base injoignable et la présenterait comme un
                // refus métier — la panne muette exacte que la spec §6 interdit.
                //
                // 🔴 AUCUNE COMPARAISON DU TEXTE DE L'EXCEPTION : les deux
                // moteurs n'écrivent pas le même (`UNIQUE constraint failed:
                // vm.utilisateur_id` contre `duplicate key value violates unique
                // constraint "vm_un_utilisateur"`). C'est l'ÉTAT relu qui
                // tranche, jamais le message.
                //
                // ⚠️ LA RELECTURE SE FAIT HORS TRANSACTION, ET C'EST STRUCTUREL,
                // pas une préférence : quand l'exception arrive ici,
                // `Pilote.transaction` a DÉJÀ émis son `ROLLBACK` et relâché le
                // client — cela se lit dans `base/pilote-postgres.ts` et
                // `base/pilote-sqlite.ts`. Il n'y a plus de transaction dans
                // laquelle lire ; `base` est donc la seule voie possible.
                const inventaire = await inventaireDe(base);
                // `vmsDe` et non `laVmDe` : cette dernière LÈVE sur un doublon,
                // et lever depuis un `catch` remplacerait la cause par une
                // autre.
                const siennes = vmsDe(inventaire, utilisateur);
                if (siennes.length > 0 && !siennes.some((v) => v.id === vm)) {
                    // L'utilisateur a bien une AUTRE VM : c'est l'index partiel
                    // `vm_un_utilisateur` qui a levé, et le refus est typé.
                    return refuser('utilisateur-servi', 'attribuer');
                }
                // Rien dans l'état ne l'explique : la cause est ailleurs, et
                // elle remonte telle quelle.
                throw cause;
            }
        },
    };
}
