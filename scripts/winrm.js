// Exécute une commande PowerShell sur la VM Windows via WinRM.
// Usage : node scripts/winrm.js "Get-ChildItem C:\\"
// ── VOIE MORTE, 29 août 2026 — voir scripts/voie-morte.sh ──────────────────
//
// 🔴 CE SCRIPT NE FONCTIONNE PLUS, ET IL AVAIT L'AIR DE FONCTIONNER.
// Il parle à la VM en transport **Basic**, que l'invité n'offre plus depuis
// `Enable-PSRemoting` (401 mesuré le 22 août 2026 ; « Failed to process the
// request, status Code: » mesuré le 5 septembre 2026).
//
// ⚠️ SON ÉCHEC ÉTAIT TRAÎTRE : il imprimait la pile de l'erreur sur STDOUT et
// sortait avec le code 0. Un appelant qui faisait `| tr -dc '0-9'` y ramassait
// les numéros de ligne de la pile et rendait un compte absurde — vu réellement
// sur la VM le 28 août 2026 : « 🔴 22246232650828772271221761422508285591251033905
// agent(s) survivant(s) ». C'est exactement le patron que ce garde supprime.
//
// Le corps d'origine reste dessous, lisible, comme relevé historique.
{
    const successeur = [
        '🔴 VOIE MORTE : scripts/winrm.js',
        '',
        "  Ce script exécutait une commande PowerShell sur la VM Windows en WinRM,",
        '  transport BASIC, compte « Administrateur ».',
        '',
        "  L'invité n'offre plus que Negotiate depuis la bascule appliance du",
        '  29 août 2026 (chantier package-nivuus).',
        '',
        '  CE QUI LE REMPLACE :',
        '     python3 ../installer/console/guest/winrm_exec.py {cmd|ps} <commande>',
        '       transport NTLM, compte « Administrator » (l\'invité est en anglais),',
        '       mot de passe lu depuis /root/.config/nivuus/windows-admin.pass et',
        "       JAMAIS sur l'argv. Il rend un code de sortie non nul sur échec de",
        '       transport — ce que celui-ci ne faisait pas.',
        '',
        "     Dans une séquence du lot 3, passer par le harnais, qui l'enveloppe :",
        '       source docs/superpowers/plans/journaux-lot3/instrument/harnais-appliance.sh',
        '',
        '  Voir CLAUDE.md § « Cycle de vie de la VM Windows ».',
        "  ⚠️ Ce script n'est PAS supprimé : `cat $0` pour le lire sans l'exécuter.",
    ].join('\n');
    console.error(successeur);
    process.exit(78);   // EX_CONFIG
}

const winrm = require('nodejs-winrm');

const HOST = process.env.WINDOWS_HOSTNAME || '192.168.3.2';
// Windows en français : le compte est « Administrateur ». « Administrator »
// échoue à l'authentification sur cette machine.
const USER = process.env.WINDOWS_ADMIN_USERNAME || 'Administrateur';
const PASS = process.env.WINDOWS_ADMIN_PASSWORD;

async function main() {
    const command = process.argv.slice(2).join(' ');
    if (!command) {
        console.error('usage : node scripts/winrm.js <commande powershell>');
        process.exit(2);
    }
    if (!PASS) {
        console.error('WINDOWS_ADMIN_PASSWORD non défini');
        process.exit(2);
    }
    const output = await winrm.runCommand(command, HOST, USER, PASS, 5985, true);
    process.stdout.write(output || '');
}

main().catch((e) => {
    console.error(e.message || e);
    process.exit(1);
});
