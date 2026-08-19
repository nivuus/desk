const winrm = require('nodejs-winrm');
const fs = require('fs').promises;
const fsSync = require('fs');
const path = require('path');
const waitPort = require('wait-port');
const ColorThief = require('colorthief');
const mime = require('mime-types');
const { parseLnk, convertToLinuxPath } = require('./lnkParser');
const { extractIcon } = require('./iconExtractor');

module.exports = [];

const { exec } = require('child_process');

async function getAppFileAssociations(appName) {

    appName = appName.replaceAll(/[0-9]/g, '').trim().replaceAll(/\s/g, '.');

    const script = `$associations = @()
$regPath = "HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\FileExts"
$extensions = Get-ChildItem -Path $regPath
foreach ($ext in $extensions) { $openWithProgidsPath = "$regPath\\$($ext.PSChildName)\\OpenWithProgids"; if (Test-Path $openWithProgidsPath) { $progIds = Get-ItemProperty -Path $openWithProgidsPath; foreach ($progId in $progIds.PSObject.Properties.Name) { if ($progId -like "*${appName}*") { $associations += [PSCustomObject]@{ Extension = $ext.PSChildName; } } } } }
if ($associations.Count -gt 0) { $associations | ConvertTo-Json -Compress } else { echo '[]' }`.replace(/\"/g, '\\"').replace(/\n\s*\n?/g, '; ');
    const result = await runInPowerShell(script);
    console.log(result);
    try {
        var associations = JSON.parse(result);
        // PowerShell ConvertTo-Json returns an object if there's only one item
        if (!Array.isArray(associations)) {
            associations = [associations];
        }
        return associations;
    } catch (e) {
        console.error(`Failed to parse associations for ${appName}:`, e.message);
        return [];
    }
}



const IconSize = 512;

async function runInPowerShell(command) {
    await waitPort({
        host: process.env.WINDOWS_HOSTNAME,
        port: 5985,
        output: 'silent'
    });
    return winrm.runCommand(command, process.env.WINDOWS_HOSTNAME, process.env.WINDOWS_ADMIN_USERNAME, process.env.WINDOWS_ADMIN_PASSWORD, 5985, true);
}

async function fetchApps() {
    const apps = [];
    const desktopPath = `/media/vm/Users/${process.env.WINDOWS_USERNAME}/Desktop`;

    console.log(`Reading desktop shortcuts from: ${desktopPath}`);

    // Read desktop directory
    const files = await fs.readdir(desktopPath);
    const lnks = files.filter(f => f.endsWith('.lnk'));

    console.log(`Found ${lnks.length} shortcuts`);

    for (const lnk of lnks) {
        try {
            const name = lnk.replace(/\.lnk$/, '');
            const short = name.replace(/\s*[0-9\.]/g, '').replace(/\s/g, '-').toLowerCase();
            const lnkPath = path.join(desktopPath, lnk);

            console.log(`\n[${name}] Processing shortcut: ${lnk}`);

            // Parse .lnk file to get target exe and icon paths
            const { targetPath, iconPath, iconIndex } = await parseLnk(lnkPath);

            if (!targetPath) {
                console.error(`[${name}] Failed to parse LNK file, skipping`);
                continue;
            }

            // Convert Windows paths to Linux paths
            const program = convertToLinuxPath(targetPath, lnkPath);
            const finalIconPath = iconPath ? convertToLinuxPath(iconPath, lnkPath) : program;

            console.log(`[${name}] Target: ${targetPath}`);
            console.log(`[${name}] Icon path: ${finalIconPath}`);
            console.log(`[${name}] Icon index: ${iconIndex}`);

            // Extract icon at 512x512
            const iconBase64 = await extractIcon(finalIconPath, iconIndex, IconSize);
            const image = `data:image/png;base64,${iconBase64}`;

            // Extract color with ColorThief
            let color = [255, 255, 255];
            try {
                const extractedColor = await ColorThief.getColor(image);
                if (extractedColor && Array.isArray(extractedColor) && extractedColor.length === 3) {
                    color = extractedColor;
                    console.log(`[${name}] Color extracted: RGB(${color.join(', ')})`);
                }
            } catch (e) {
                console.error(`[${name}] Failed to extract color:`, e.message);
            }

            // Get file associations (still using WinRM for this)
            const fileHandlers = [];
            const associations = await getAppFileAssociations(name);

            associations.forEach(association => {
                const mimeType = mime.lookup(association.Extension);
                if (mimeType) {
                    fileHandlers.push({
                        extension: association.Extension,
                        mimeType: mimeType
                    });
                }
            });

            apps.push({
                name,
                short,
                program: targetPath, // Keep Windows path for RDP
                color,
                icon: image,
                fileHandlers
            });

            console.log(`[${name}] Successfully processed`);

        } catch (error) {
            console.error(`Error processing ${lnk}:`, error.message);
        }
    }

    return apps;
}

async function update() {
    try {
        const apps = await fetchApps();
        console.log('fetched');
        module.exports.splice(0, module.exports.length);
        module.exports.push(...apps);
    } catch (e) {
        console.error('Error fetching apps:', e);
        // Keep existing apps if update fails
    }
}

module.exports.wait = update();


setInterval(update, 1000 * 60 * 60);