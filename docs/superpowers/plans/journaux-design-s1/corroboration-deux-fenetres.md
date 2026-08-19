# Corroboration à deux fenêtres réelles — HORS CRITÈRE

**Commit `604f91c`, 19 août 2026. Navigateur Chromium piloté par Playwright,
sur l'hôte. `npx vite preview --port 4319` servant `client/dist/`.**

🔴 **CECI N'EST PAS UN CRITÈRE**, et c'est dit au même endroit que c'est
rapporté. Le contrôle §7.5 éprouve **notre gestionnaire** ; il n'établit pas
que le navigateur déclenche `storage` entre deux fenêtres réelles. Cette page
comble ce trou-là et rien d'autre — même statut que les confirmations sur VM
réelle du sous-projet ⑤. **Une exécution, aucun taux.**

## ① La bascule atteint la fenêtre voisine

| Geste | Fenêtre ÉCRIVAINE (onglet 1) | Fenêtre VOISINE (onglet 0) |
| --- | --- | --- |
| état initial | `data-theme` = `null`, `--fond-0` = `#ffffff` | idem |
| clic sur « sombre » | `data-theme` = `sombre`, `--fond-0` = `#0b0d10`, `guac.theme` = `sombre` | `data-theme` = **`sombre`**, `--fond-0` = **`#0b0d10`** |
| clic sur « systeme » | `data-theme` = **`null`** (attribut RETIRÉ), `guac.theme` = `systeme` | `data-theme` = **`null`**, `--fond-0` = **`#ffffff`** |

**Deux faits que le test unitaire ne pouvait pas donner :**

1. la voisine a **re-rendu** — la valeur affichée sous la première pastille est
   passée à `#0b0d10`, donc le gestionnaire `storage` a bien rappelé
   `rendre()`, il n'a pas seulement posé un attribut ;
2. le retour à `systeme` **retire** l'attribut chez la voisine aussi, et la
   page retombe sur la préférence du système (`#ffffff`). C'est le chemin
   `removeAttribute` traversé de bout en bout entre deux documents.

## ② L'amorce anti-FOUC est prouvée S'EXÉCUTER, et par ISOLATION

`client/connexion.html` **n'importe aucun module de thème** — son seul script
est `connexion.ts`, qui ne connaît ni `theme.ts` ni `guac.theme`. Avec
`guac.theme = 'sombre'` posé puis navigation vers cette page :

```
{ page: 'connexion.html (aucun module de theme)',
  attribut: 'sombre',
  fond: 'rgb(11, 13, 16)',      // --fond-0 sombre
  encre: 'rgb(230, 232, 235)',  // --texte-fort sombre
  colorScheme: 'dark',
  police: 'system-ui, -apple-system, "Seg…' }
```

**Seule l'amorce en ligne peut avoir posé cet attribut** : rien d'autre sur
cette page ne lit `localStorage`. Le §7.3 prouvait que l'amorce est *injectée* ;
ceci prouve qu'elle **s'exécute et agit**.

⚠️ **Cela ne prouve toujours PAS l'absence d'éclair de mauvais thème.** Aucune
capture chronométrée n'a été faite, et le §7.8 l'écarte. « L'amorce agit » et
« aucun FOUC n'est visible » sont deux énoncés distincts ; seul le premier est
mesuré.

## ③ `color-scheme` suit le thème — la divergence D9 corroborée

Avec `guac.theme = 'clair'` sur `connexion.html`, qui porte deux `<input>` dont
un `type="password"` :

```
{ attribut: 'clair', colorScheme: 'light',
  fond: 'rgb(255, 255, 255)', champMdp: 'rgb(255, 255, 255)' }
```

Le champ de mot de passe rend en **natif clair**. C'était le défaut le plus
visible que le socle pouvait introduire (une page claire aux champs restés
sombres), et il n'est attrapé par **aucun** des sept contrôles.

## Ce que cette corroboration N'établit PAS

- **Une exécution, un seul navigateur** (Chromium de bureau) : rien de Firefox,
  rien de Safari, rien du mobile.
- **Deux fenêtres de la GALERIE**, pas une page-shell qui ouvre N sessions par
  `window.open`. Que le thème atteigne les N fenêtres du produit reste non
  mesuré.
- **Aucun jugement visuel** n'a été porté : les huit lignes du §8 restent
  entières.
- `deviceScaleFactor` = 1 : **rien du HiDPI**.
