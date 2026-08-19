const fs = require('fs').promises;

/**
 * Parse a Windows .lnk shortcut file to extract target information
 * Based on MS-SHLLINK specification
 */
async function parseLnk(lnkPath) {
    try {
        const buffer = await fs.readFile(lnkPath);

        // Verify LNK header
        const headerSize = buffer.readUInt32LE(0x00);
        if (headerSize !== 0x4C) {
            throw new Error('Invalid LNK file header');
        }

        // Read LinkFlags
        const flags = buffer.readUInt32LE(0x14);
        console.log(`[LNK Parser] Flags: 0x${flags.toString(16)}`);

        let offset = 0x4C; // After ShellLinkHeader

        // Skip LinkTargetIDList if present
        if (flags & 0x01) {
            const idListSize = buffer.readUInt16LE(offset);
            console.log(`[LNK Parser] Skipping IDList of size ${idListSize}`);
            offset += 2 + idListSize;
        }

        let targetPath = null;
        let iconPath = null;
        let iconIndex = 0;

        // Read LinkInfo if present (contains local path)
        if (flags & 0x02) {
            console.log(`[LNK Parser] Has LinkInfo at offset ${offset}`);
            const linkInfoStart = offset;
            const linkInfoSize = buffer.readUInt32LE(offset);
            const linkInfoHeaderSize = buffer.readUInt32LE(offset + 0x04);
            const linkInfoFlags = buffer.readUInt32LE(offset + 0x08);

            console.log(`[LNK Parser] LinkInfo size: ${linkInfoSize}, header size: ${linkInfoHeaderSize}, flags: 0x${linkInfoFlags.toString(16)}`);

            if (linkInfoFlags & 0x01) {
                // Has local path
                const localPathOffset = buffer.readUInt32LE(offset + 0x10);
                const pathStart = linkInfoStart + localPathOffset;

                console.log(`[LNK Parser] Local path offset: ${localPathOffset}, absolute: ${pathStart}`);

                // Read null-terminated string
                let pathEnd = pathStart;
                while (pathEnd < buffer.length && buffer[pathEnd] !== 0) {
                    pathEnd++;
                }

                targetPath = buffer.toString('latin1', pathStart, pathEnd);
                console.log(`[LNK Parser] Extracted target from LinkInfo: ${targetPath}`);
            }

            offset += linkInfoSize;
        }

        // Read StringData sections
        console.log(`[LNK Parser] Reading StringData at offset ${offset}`);

        // Read NAME_STRING if present
        if (flags & 0x04) {
            if (offset + 2 <= buffer.length) {
                const nameSize = buffer.readUInt16LE(offset);
                offset += 2;
                if (nameSize > 0 && offset + nameSize * 2 <= buffer.length) {
                    const nameStr = buffer.toString('utf16le', offset, offset + nameSize * 2);
                    console.log(`[LNK Parser] NAME_STRING: ${nameStr}`);
                    offset += nameSize * 2;
                }
            }
        }

        // Read RELATIVE_PATH if present (might contain the exe path)
        if (flags & 0x08) {
            if (offset + 2 <= buffer.length) {
                const relPathSize = buffer.readUInt16LE(offset);
                offset += 2;
                if (relPathSize > 0 && offset + relPathSize * 2 <= buffer.length) {
                    const relPath = buffer.toString('utf16le', offset, offset + relPathSize * 2);
                    console.log(`[LNK Parser] RELATIVE_PATH: ${relPath}`);

                    // Use relative path as target if we don't have a valid target yet
                    // LinkInfo sometimes only returns "C:\" which is incomplete
                    if ((!targetPath || targetPath === 'C:\\') && relPath) {
                        // Clean up relative path: remove leading ..\..\.. and add C:\
                        const cleanPath = relPath.replace(/^(?:\.\.\\)+/, '');
                        targetPath = cleanPath.startsWith('C:\\') ? cleanPath : `C:\\${cleanPath}`;
                        console.log(`[LNK Parser] Using cleaned RELATIVE_PATH: ${targetPath}`);
                    }
                    offset += relPathSize * 2;
                }
            }
        }

        // Read WORKING_DIR if present
        if (flags & 0x10) {
            if (offset + 2 <= buffer.length) {
                const workDirSize = buffer.readUInt16LE(offset);
                offset += 2;
                if (workDirSize > 0 && offset + workDirSize * 2 <= buffer.length) {
                    const workDir = buffer.toString('utf16le', offset, offset + workDirSize * 2);
                    console.log(`[LNK Parser] WORKING_DIR: ${workDir}`);
                    offset += workDirSize * 2;
                }
            }
        }

        // Read COMMAND_LINE_ARGUMENTS if present
        if (flags & 0x20) {
            if (offset + 2 <= buffer.length) {
                const argsSize = buffer.readUInt16LE(offset);
                offset += 2;
                if (argsSize > 0 && offset + argsSize * 2 <= buffer.length) {
                    const args = buffer.toString('utf16le', offset, offset + argsSize * 2);
                    console.log(`[LNK Parser] COMMAND_LINE_ARGUMENTS: ${args}`);
                    offset += argsSize * 2;
                }
            }
        }

        // Read ICON_LOCATION if present
        if (flags & 0x40) {
            if (offset + 2 <= buffer.length) {
                const iconLocSize = buffer.readUInt16LE(offset);
                offset += 2;

                if (iconLocSize > 0 && offset + iconLocSize * 2 <= buffer.length) {
                    iconPath = buffer.toString('utf16le', offset, offset + iconLocSize * 2);
                    console.log(`[LNK Parser] ICON_LOCATION: ${iconPath}`);

                    // Extract icon index from path (format: "path,index")
                    const match = iconPath.match(/^(.*),(\d+)$/);
                    if (match) {
                        iconPath = match[1];
                        iconIndex = parseInt(match[2], 10);
                    }
                    offset += iconLocSize * 2;
                }
            }
        }

        console.log(`[LNK Parser] Final targetPath: ${targetPath}`);

        return {
            targetPath: targetPath || '',
            iconPath: iconPath || targetPath,
            iconIndex: iconIndex
        };

    } catch (error) {
        console.error(`Error parsing LNK file ${lnkPath}:`, error.message);
        return {
            targetPath: '',
            iconPath: '',
            iconIndex: 0
        };
    }
}

/**
 * Convert Windows path to Linux /media/vm path
 * Handles absolute paths, environment variables, and user-relative paths
 */
function convertToLinuxPath(windowsPath, lnkPath = '') {
    if (!windowsPath) return '';

    let result = windowsPath;

    // Handle user-relative paths (starting with ..\AppData or similar)
    if (result.startsWith('..\\')) {
        // Extract username from lnkPath which is like /media/vm/Users/username/Desktop/file.lnk
        const userMatch = lnkPath.match(/\/media\/vm\/Users\/([^\/]+)/);
        if (userMatch) {
            const username = userMatch[1];
            // Remove leading ..\ and prepend with user path
            result = result.replace(/^\.\.\\/, '');
            result = `C:\\Users\\${username}\\${result}`;
        }
    }

    // Convert to Linux path
    result = result
        .replace(/^C:\\/i, '/media/vm/')
        .replace(/^%ProgramFiles%/i, '/media/vm/Program Files')
        .replace(/^%ProgramFiles\(x86\)%/i, '/media/vm/Program Files (x86)')
        .replace(/^%SystemRoot%/i, '/media/vm/Windows')
        .replace(/^%windir%/i, '/media/vm/Windows')
        .replace(/\\/g, '/');

    return result;
}

module.exports = { parseLnk, convertToLinuxPath };
