#!/usr/bin/env node
const winrm = require('nodejs-winrm');

// Configuration from Guacamole project
const config = {
    hostname: '192.168.3.2',
    adminUsername: 'Administrator',
    adminPassword: 'mOMY5F!7NL5^7q7k',
    username: 'guacamole',
    password: '***RETIRE-DE-L-HISTORIQUE***`*W\\2&^FC`~I\'(sElpbVke|^lh5aL]',
    port: 5985,
    useHttps: true
};

function parseResult(result) {
    if (typeof result === 'string') {
        return result.trim();
    } else if (Buffer.isBuffer(result)) {
        return result.toString('utf-8').trim();
    } else if (result && result.stdout) {
        return result.stdout.trim();
    }
    return String(result).trim();
}

async function runCommand(command, user, pass) {
    try {
        const result = await winrm.runCommand(
            command,
            config.hostname,
            user,
            pass,
            config.port,
            config.useHttps
        );
        return parseResult(result);
    } catch (error) {
        throw new Error(`WinRM command failed: ${error.message}`);
    }
}

async function checkGPU() {
    console.log(`\n=== Checking GPU (NVIDIA RTX 4070) ===`);
    try {
        // Check all PCI devices for NVIDIA
        const pciCommand = `powershell -Command "Get-CimInstance -ClassName Win32_PnPEntity | Where-Object {$_.DeviceID -like '*VEN_10DE*'} | Select-Object Name,DeviceID,Status,ConfigManagerErrorCode | ConvertTo-Json"`;
        const result = await runCommand(pciCommand, config.adminUsername, config.adminPassword);

        console.log('Raw result:', result);

        try {
            const devices = JSON.parse(result);
            const deviceArray = Array.isArray(devices) ? devices : [devices];

            console.log(`\nFound ${deviceArray.length} NVIDIA device(s):`);

            deviceArray.forEach((device, i) => {
                console.log(`\n  Device ${i + 1}:`);
                console.log(`    Name: ${device.Name}`);
                console.log(`    Status: ${device.Status}`);
                console.log(`    Error Code: ${device.ConfigManagerErrorCode}`);

                switch (device.ConfigManagerErrorCode) {
                    case 0:
                        console.log(`    ✓ Device working properly`);
                        break;
                    case 28:
                        console.log(`    ✗ Drivers not installed`);
                        break;
                    case 43:
                        console.log(`    ✗ Code 43 - Windows has stopped this device`);
                        break;
                    default:
                        console.log(`    ⚠ Error code: ${device.ConfigManagerErrorCode}`);
                }
            });
        } catch (e) {
            console.log('Failed to parse GPU JSON:', e.message);
            console.log('Result was:', result);
        }

        // Check video controllers
        console.log(`\n=== Video Controllers ===`);
        const gpuCommand = `powershell -Command "Get-CimInstance -ClassName Win32_VideoController | Select-Object Name,Status,DriverVersion | ConvertTo-Json"`;
        const gpuResult = await runCommand(gpuCommand, config.adminUsername, config.adminPassword);

        console.log('GPU Result:', gpuResult);

    } catch (error) {
        console.log(`✗ Failed: ${error.message}`);
    }
}

async function installNVIDIADriver() {
    console.log(`\n=== Check if NVIDIA Driver Installer Exists ===`);
    try {
        const checkCommand = `powershell -Command "Test-Path 'C:\\NVIDIA\\*.exe'"`;
        const exists = await runCommand(checkCommand, config.adminUsername, config.adminPassword);

        console.log(`Driver installer exists: ${exists}`);

        if (exists.toLowerCase().includes('true')) {
            console.log('\nNVIDIA driver installer found in C:\\NVIDIA\\');
            console.log('To install, run in Windows:');
            console.log('  C:\\NVIDIA\\<installer>.exe /s /noreboot');
        } else {
            console.log('\nNo NVIDIA driver installer found.');
            console.log('Download RTX 4070 driver from: https://www.nvidia.com/Download/index.aspx');
        }
    } catch (error) {
        console.log(`Error: ${error.message}`);
    }
}

async function main() {
    console.log('=== WinRM GPU Diagnostic ===');
    console.log(`Target: ${config.hostname}:${config.port}\n`);

    // Test connection
    console.log('Testing connection...');
    try {
        const hostname = await runCommand('hostname', config.adminUsername, config.adminPassword);
        console.log(`✓ Connected to: ${hostname}\n`);
    } catch (error) {
        console.log(`✗ Connection failed: ${error.message}`);
        return;
    }

    await checkGPU();
    await installNVIDIADriver();

    console.log('\n=== Test Complete ===\n');
}

main().catch(console.error);
