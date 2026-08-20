/* L'amorce anti-FOUC : elle pose `data-theme` avant la première peinture.

   🔴 CE FICHIER PART VERBATIM DANS CHAQUE PAGE BÂTIE, COMMENTAIRES COMPRIS.
   Il n'est passé par AUCUN transform : le greffon `guac-amorce-theme` de
   `client/vite.config.ts` lit son texte brut et le rend tel quel en ligne dans
   le `<head>`. À l'inverse des commentaires de `theme.ts`, que le bundler
   retire, ceux-ci sont livrés — mesuré le 19 août 2026 : un en-tête de
   raisonnement de 1 921 octets pesait 1 921 octets DANS CHACUNE des pages, et
   aucun des neuf contrôles ne l'aurait dit (§7.7 ne pèse que le CSS).
   C'est pourquoi le raisonnement vit là où il est GRATUIT : le greffon, dans
   `vite.config.ts`, qui n'est que du temps de build, et `theme.ts`.

   Ce qu'il faut savoir ici, et rien de plus : aucune règle n'est portée par ce
   fichier — il ignore les trois états et ce que l'absence d'attribut signifie
   (voir `theme.ts`) ; sa chaîne de clé est la seule duplication du sous-bloc,
   inévitable puisqu'un script en ligne ne peut rien importer, et
   `theme.test.ts` refuse qu'elle diverge.
   ⚠️ CETTE CLÉ N'EST NOMMÉE NULLE PART DANS CE COMMENTAIRE, ET C'EST VOULU.
   Une première rédaction l'y écrivait entre quotes ; le garde de
   `theme.test.ts` devenait alors VACUEUX — mesuré le 19 août 2026 en
   remplaçant tout l'appel par `var t = null;` : les deux assertions
   restaient VERTES, satisfaites par le commentaire seul. Un contrôle qu'on
   peut satisfaire depuis un commentaire n'en est pas un.
   Enfin, le `try` couvre l'ACCÈS à
   `localStorage`, qui lève lui-même en mode privé strict. */
(function () {
    try {
        var t = localStorage.getItem('guac.theme');
        if (t === 'clair' || t === 'sombre') {
            document.documentElement.setAttribute('data-theme', t);
        }
    } catch (_) {
        /* stockage indisponible : on reste sur la préférence du système. */
    }
})();
