// Câblage de l'écran de connexion : un formulaire DOM d'un côté,
// `POST /auth/connexion` de l'autre. Aucune règle ici — elles sont dans
// `jeton.ts`, qui est testé.
//
// ⚠️ CE FICHIER N'EST PAS TESTÉ UNITAIREMENT, et c'est DÉCLARÉ plutôt que
// subi : c'est la même convention que `shell-page.ts` et `main.ts`, qui ne le
// sont pas non plus. Ce qui la rend tenable est la clause qui l'accompagne :
// **toute règle que ce fichier porterait doit descendre dans `jeton.ts`**. Si
// une condition apparaît ici, c'est qu'elle est au mauvais endroit.
//
// ⚠️ LE MESSAGE D'ÉCHEC EST CELUI DU SERVICE, TEL QUEL. Il ne distingue pas
// « courriel inconnu » de « mot de passe faux » (`plateforme/src/http/
// routes-auth.ts` : un message qui les distinguerait serait un oracle
// d'énumération de comptes). Enrichir le texte ici défairait cette propriété
// depuis le seul endroit où personne ne penserait à la chercher.

import { poser } from './jeton';

const params = new URLSearchParams(window.location.search);
// La MÊME convention que le signaling de `shell-page.ts` : un paramètre de
// requête, sinon l'hôte de la page et le port 8080. Inventer une seconde
// convention obligerait à savoir laquelle s'applique où.
const plateformeUrl = params.get('plateforme') ?? `http://${window.location.hostname}:8080`;
// Où l'on repart une fois connecté. Le paramètre existe pour que l'écran
// puisse renvoyer vers la page qui a exigé la connexion, et pas seulement vers
// la shell.
const suite = params.get('suite') ?? 'shell.html';

const formulaire = document.querySelector<HTMLFormElement>('#connexion')!;
const champEmail = document.querySelector<HTMLInputElement>('#email')!;
const champMotDePasse = document.querySelector<HTMLInputElement>('#motdepasse')!;
const bouton = document.querySelector<HTMLButtonElement>('#valider')!;
const message = document.querySelector<HTMLDivElement>('#message')!;

formulaire.addEventListener('submit', async (evenement) => {
    evenement.preventDefault();
    bouton.disabled = true;
    message.textContent = 'connexion…';

    try {
        const reponse = await fetch(`${plateformeUrl}/auth/connexion`, {
            method: 'POST',
            headers: { 'content-type': 'application/json' },
            body: JSON.stringify({
                email: champEmail.value,
                motdepasse: champMotDePasse.value,
            }),
        });
        const corps = await reponse.json().catch(() => undefined);

        if (!reponse.ok) {
            // Le motif du service, tel quel — voir l'en-tête.
            message.textContent = `refusé : ${corps?.refus ?? reponse.status}`;
            return;
        }

        poser(window.localStorage, {
            acces: corps.acces,
            rafraichissement: corps.rafraichissement,
        });
        // Le mot de passe ne survit pas à la connexion : le champ est vidé
        // avant de quitter la page, pour qu'un retour arrière du navigateur ne
        // le retrouve pas rempli.
        champMotDePasse.value = '';
        window.location.href = suite;
    } catch (cause) {
        // Un échec RÉSEAU se dit comme tel : sur une autre origine, c'est le
        // symptôme d'une `PLATEFORME_ORIGINE_CLIENT` absente côté service
        // (`plateforme/src/config.ts`), et le confondre avec un refus
        // d'identifiants enverrait chercher le défaut au mauvais endroit.
        message.textContent = `plateforme injoignable (${String(cause)})`;
    } finally {
        bouton.disabled = false;
    }
});
