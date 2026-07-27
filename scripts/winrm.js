// Exécute une commande PowerShell sur la VM Windows via WinRM.
// Usage : node scripts/winrm.js "Get-ChildItem C:\\"
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
