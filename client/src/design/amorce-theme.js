/* The anti-FOUC bootstrap: it sets `data-theme` before first paint.

   🔴 THIS FILE GOES VERBATIM INTO EVERY BUILT PAGE, COMMENTS INCLUDED.
   It goes through NO transform: the `guac-amorce-theme` plugin of
   `client/vite.config.ts` reads its raw text and renders it as is inline in
   the `<head>`. Unlike the comments of `theme.ts`, which the bundler
   strips, these are shipped — measured on August 19th, 2026: a
   1,921-byte reasoning header weighed 1,921 bytes IN EACH of the pages, and
   none of the nine checks would have said so (§7.7 only weighs CSS).
   That is why the reasoning lives where it is FREE: the plugin, in
   `vite.config.ts`, which is build time only, and `theme.ts`.

   What must be known here, and nothing more: no rule is carried by this
   file — it ignores the three states and what the attribute's absence means
   (see `theme.ts`); its key string is the sub-block's only duplication,
   unavoidable since an inline script cannot import anything, and
   `theme.test.ts` refuses to let it diverge.
   ⚠️ THIS KEY IS NAMED NOWHERE IN THIS COMMENT, AND THAT IS INTENDED.
   A first draft wrote it there in quotes; the guard of
   `theme.test.ts` then became VACUOUS — measured on August 19th, 2026 by
   replacing the whole call with `var t = null;`: both assertions
   stayed GREEN, satisfied by the comment alone. A check that
   can be satisfied from a comment is not one.
   Finally, the `try` covers ACCESS to
   `localStorage`, which itself throws in strict private mode. */
(function () {
    try {
        var t = localStorage.getItem('guac.theme');
        if (t === 'clair' || t === 'sombre') {
            document.documentElement.setAttribute('data-theme', t);
        }
    } catch (_) {
        /* storage unavailable: we stay on the system preference. */
    }
})();
