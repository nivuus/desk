// THE OLD SHELL PAGE — turned into a mere redirect on August 31st, 2026.
//
// 🔴 THIS FILE CARRIED 500 LINES AND HALF THE PRODUCT. Its content now lives
// in `bureau/porteur-dom.ts`, `bureau/fenetres-dom.ts` and
// `bureau/files-dom.ts`, used by the hub — the ONLY surface since
// that date. The business rule `shell.ts`, for its part, has not moved by a line:
// it was already pure and tested, and it is reused as is.
//
// ⚠️ DO NOT DELETE THIS FILE NOR ITS PAGE. See `bureau/redirection.ts`
// for the reason — it has to do with already installed PWAs, not with caution.

import { cibleDeRedirection } from './bureau/redirection';

// `replace` and not `href`: going back would return to this page, which
// would redirect again, and the user would be trapped in the history.
window.location.replace(cibleDeRedirection(window.location.search));
