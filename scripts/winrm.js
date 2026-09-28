// Runs a PowerShell command on the Windows VM through WinRM.
// Usage: node scripts/winrm.js "Get-ChildItem C:\\"
// ── DEAD PATH, 29 August 2026 — see scripts/voie-morte.sh ──────────────────
//
// 🔴 THIS SCRIPT NO LONGER WORKS, AND IT LOOKED LIKE IT WORKED.
// It talks to the VM over **Basic** transport, which the guest no longer offers since
// `Enable-PSRemoting` (401 measured on 22 August 2026; "Failed to process the
// request, status Code:" measured on 5 September 2026).
//
// ⚠️ ITS FAILURE WAS TREACHEROUS: it printed the error stack on STDOUT and
// exited with code 0. A caller doing `| tr -dc '0-9'` picked up
// the stack line numbers and reported an absurd count — actually seen
// on the VM on 28 August 2026: "🔴 22246232650828772271221761422508285591251033905
// surviving agent(s)". That is exactly the pattern this guard removes.
//
// The original body stays below, readable, as a historical record.
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
// French-language Windows: the account is "Administrateur". "Administrator"
// fails authentication on this machine. policy: allow-fr
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
