const sharp = require('sharp');
const { decode } = require('sharp-ico');
const fs = require('fs').promises;
const path = require('path');
const { spawn } = require('child_process');
const util = require('util');
const execPromise = util.promisify(require('child_process').exec);
const winrm = require('nodejs-winrm');
const waitPort = require('wait-port');

async function runInPowerShell(command) {
    await waitPort({
        host: process.env.WINDOWS_HOSTNAME,
        port: 5985,
        output: 'silent'
    });
    return winrm.runCommand(command, process.env.WINDOWS_HOSTNAME, process.env.WINDOWS_ADMIN_USERNAME, process.env.WINDOWS_ADMIN_PASSWORD, 5985, true);
}

/**
 * Extract icon from Windows executable or icon file
 * Returns base64 encoded PNG at target size
 */
async function extractIcon(exePath, iconIndex = 0, targetSize = 512) {
    try {
        console.log(`Extracting icon from: ${exePath}, index: ${iconIndex}`);

        // Check if the exe file exists
        try {
            const stat = await fs.stat(exePath);
            console.log(`File exists: ${exePath} (size: ${stat.size} bytes)`);
        } catch (e) {
            console.error(`File does not exist: ${exePath}`);
            throw new Error(`File not found: ${exePath}`);
        }

        let iconBuffer = null;

        // Try different methods to extract the icon

        // Method 1: Check for standalone .ico file (same name as exe)
        const icoPath = exePath.replace(/\.exe$/i, '.ico');
        try {
            const stat = await fs.stat(icoPath);
            if (stat.isFile()) {
                console.log(`Found .ico file: ${icoPath}`);
                iconBuffer = await fs.readFile(icoPath);
            }
        } catch (e) {
            console.log(`No .ico file at ${icoPath}`);
        }

        // Method 1b: Look for common icon files in the same directory
        if (!iconBuffer) {
            try {
                const exeDir = path.dirname(exePath);
                const commonIconNames = ['icon.ico', 'app.ico', 'logo.ico', 'icon_256.ico', 'icon_512.ico'];

                for (const iconName of commonIconNames) {
                    const iconPath = path.join(exeDir, iconName);
                    try {
                        const stat = await fs.stat(iconPath);
                        if (stat.isFile()) {
                            console.log(`Found icon file: ${iconPath}`);
                            iconBuffer = await fs.readFile(iconPath);
                            break;
                        }
                    } catch (e) {
                        // File doesn't exist, continue
                    }
                }
            } catch (e) {
                console.log(`Error searching for icon files: ${e.message}`);
            }
        }

        // Method 2: Use icotool to extract from .exe (better for modern PE files)
        if (!iconBuffer) {
            try {
                iconBuffer = await extractIconWithIcotool(exePath, iconIndex);
                console.log(`Extracted icon using icotool`);
            } catch (e) {
                console.error(`icotool extraction failed: ${e.message}`);
            }
        }

        // Method 3: Use wrestool to extract from .exe
        if (!iconBuffer) {
            try {
                iconBuffer = await extractIconWithWrestool(exePath, iconIndex);
                console.log(`Extracted icon using wrestool`);
            } catch (e) {
                console.error(`wrestool extraction failed: ${e.message}`);
            }
        }

        // Method 4: Try sharp-ico directly on .exe
        if (!iconBuffer) {
            try {
                const exeBuffer = await fs.readFile(exePath);
                const icons = await decode(exeBuffer);

                if (icons && icons.length > 0) {
                    // Select largest icon or specific index
                    const icon = iconIndex < icons.length ?
                        icons[iconIndex] :
                        icons.reduce((max, icon) => icon.width > max.width ? icon : max);

                    iconBuffer = icon.data;
                    console.log(`Extracted icon using sharp-ico: ${icon.width}x${icon.height}`);
                }
            } catch (e) {
                console.error(`sharp-ico extraction failed: ${e.message}`);
            }
        }

        // Method 5: Use PowerShell to extract and save to temp file
        if (!iconBuffer) {
            try {
                // Convert Linux path back to Windows path
                const windowsPath = exePath
                    .replace(/^\/media\/vm\//, 'C:\\')
                    .replace(/\//g, '\\');

                iconBuffer = await extractIconWithPowerShell(windowsPath, iconIndex, targetSize);
                console.log(`Extracted icon using PowerShell (native size)`);
                // Don't return here - let Sharp resize it below for better quality
            } catch (e) {
                console.error(`PowerShell extraction failed: ${e.message}`);
            }
        }

        if (!iconBuffer) {
            throw new Error('No icon could be extracted');
        }

        // Decode and resize with sharp
        const resized = await sharp(iconBuffer)
            .resize(targetSize, targetSize, {
                fit: 'contain',
                background: { r: 0, g: 0, b: 0, alpha: 0 },
                kernel: sharp.kernel.lanczos3
            })
            .png()
            .toBuffer();

        console.log(`Icon resized to ${targetSize}x${targetSize}`);

        return resized.toString('base64');

    } catch (error) {
        console.error(`Error extracting icon from ${exePath}:`, error.message);
        // Return empty transparent PNG as fallback
        return await createEmptyIcon(targetSize);
    }
}

/**
 * Extract icon using PowerShell and save to temp file
 */
async function extractIconWithPowerShell(windowsPath, iconIndex, targetSize) {
    const tempFile = `C:\\temp\\icon_${Date.now()}.png`;
    const tempFileLinux = `/media/vm/temp/icon_${Date.now()}.png`;

    const script = `
if (-not (Test-Path 'C:\\temp')) { New-Item -ItemType Directory -Path 'C:\\temp' | Out-Null }
Add-Type -AssemblyName System.Drawing
$icon = [System.Drawing.Icon]::ExtractAssociatedIcon('${windowsPath}')
if ($icon) {
    $bitmap = $icon.ToBitmap()
    $bitmap.Save('${tempFile}', [System.Drawing.Imaging.ImageFormat]::Png)
    $bitmap.Dispose()
    $icon.Dispose()
    echo 'SUCCESS'
} else {
    echo 'FAILED'
}
`.replace(/\n/g, '; ');

    try {
        console.log(`Trying PowerShell extraction for ${windowsPath}`);
        const result = await runInPowerShell(script);

        if (result.includes('SUCCESS')) {
            // Wait a bit for file to be written
            await new Promise(resolve => setTimeout(resolve, 100));

            // Read the file from /media/vm
            const iconBuffer = await fs.readFile(tempFileLinux);

            // Clean up temp file
            try {
                await runInPowerShell(`Remove-Item '${tempFile}' -Force`);
            } catch (e) {
                console.error(`Failed to clean up temp file: ${e.message}`);
            }

            return iconBuffer;
        } else {
            throw new Error('PowerShell extraction returned FAILED');
        }
    } catch (error) {
        throw new Error(`PowerShell extraction failed: ${error.message}`);
    }
}

/**
 * Extract icon using icotool command (better for modern PE files)
 */
async function extractIconWithIcotool(exePath, iconIndex) {
    return new Promise((resolve, reject) => {
        const extractCmd = spawn('icotool', [
            '-x',
            '-i', (iconIndex + 1).toString(), // icotool uses 1-based index
            exePath
        ]);

        let iconData = Buffer.alloc(0);
        let errorOutput = '';

        extractCmd.stdout.on('data', (data) => {
            iconData = Buffer.concat([iconData, data]);
        });

        extractCmd.stderr.on('data', (data) => {
            errorOutput += data.toString();
        });

        extractCmd.on('close', (code) => {
            if (code !== 0 || iconData.length === 0) {
                return reject(new Error(`icotool extraction failed: ${errorOutput}`));
            }

            resolve(iconData);
        });

        extractCmd.on('error', reject);
    });
}

/**
 * Extract icon using wrestool command
 */
async function extractIconWithWrestool(exePath, iconIndex) {
    return new Promise((resolve, reject) => {
        // List icons first
        const listCmd = spawn('wrestool', ['-l', '-t', '14', exePath]);
        let listOutput = '';

        listCmd.stdout.on('data', (data) => {
            listOutput += data.toString();
        });

        listCmd.on('close', (code) => {
            if (code !== 0) {
                return reject(new Error('wrestool list failed'));
            }

            // Parse output to find icon groups
            const lines = listOutput.split('\n');
            const iconLine = lines.find(line => line.includes('group icon'));

            if (!iconLine) {
                return reject(new Error('No icons found'));
            }

            // Extract icon name (format: --name=XXX)
            const nameMatch = iconLine.match(/--name[=\s]+(\S+)/);
            if (!nameMatch) {
                return reject(new Error('Could not parse icon name'));
            }

            const iconName = nameMatch[1];

            // Extract the icon
            const extractCmd = spawn('wrestool', [
                '-x',
                '-t', '14',
                '-n', iconName,
                exePath
            ]);

            let iconData = Buffer.alloc(0);

            extractCmd.stdout.on('data', (data) => {
                iconData = Buffer.concat([iconData, data]);
            });

            extractCmd.on('close', (code) => {
                if (code !== 0 || iconData.length === 0) {
                    return reject(new Error('wrestool extraction failed'));
                }

                resolve(iconData);
            });

            extractCmd.on('error', reject);
        });

        listCmd.on('error', reject);
    });
}

/**
 * Create empty transparent icon as fallback
 */
async function createEmptyIcon(size) {
    const empty = await sharp({
        create: {
            width: size,
            height: size,
            channels: 4,
            background: { r: 0, g: 0, b: 0, alpha: 0 }
        }
    })
    .png()
    .toBuffer();

    return empty.toString('base64');
}

module.exports = { extractIcon };
