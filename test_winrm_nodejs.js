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
    useHttps: true  // This is actually "allowUnauthorized" in nodejs-winrm
};

async function testConnection(user, pass, label) {
    console.log(`\n=== Testing ${label} ===`);
    try {
        const result = await winrm.runCommand(
            'hostname',
            config.hostname,
            user,
            pass,
            config.port,
            config.useHttps
        );
        console.log(`✓ Success!`);
        console.log(`Hostname: ${result.trim()}`);
        return true;
    } catch (error) {
        console.log(`✗ Failed: ${error.message}`);
        return false;
    }
}

async function getSystemInfo(user, pass) {
    console.log(`\n=== Getting System Info ===`);
    try {
        const command = 'powershell -Command "Get-ComputerInfo | Select-Object CsName,WindowsVersion,OsArchitecture,CsProcessors | ConvertTo-Json"';
        const result = await winrm.runCommand(
            command,
            config.hostname,
            user,
            pass,
            config.port,
            config.useHttps
        );
        console.log('System Info:');
        const info = JSON.parse(result);
        console.log(`  Computer: ${info.CsName}`);
        console.log(`  Windows: ${info.WindowsVersion}`);
        console.log(`  Arch: ${info.OsArchitecture}`);
        console.log(`  CPUs: ${info.CsProcessors.length}`);
    } catch (error) {
        console.log(`✗ Failed: ${error.message}`);
    }
}

async function checkGPU(user, pass) {
    console.log(`\n=== Checking GPU ===`);
    try {
        const command = 'powershell -Command "Get-CimInstance -ClassName Win32_VideoController | Select-Object Name,DriverVersion,Status,AdapterRAM | ConvertTo-Json"';
        const result = await winrm.runCommand(
            command,
            config.hostname,
            user,
            pass,
            config.port,
            config.useHttps
        );
        const gpus = JSON.parse(result);
        const gpuArray = Array.isArray(gpus) ? gpus : [gpus];

        gpuArray.forEach((gpu, i) => {
            console.log(`\n  GPU ${i + 1}:`);
            console.log(`    Name: ${gpu.Name}`);
            console.log(`    Driver: ${gpu.DriverVersion}`);
            console.log(`    Status: ${gpu.Status}`);
            console.log(`    VRAM: ${(gpu.AdapterRAM / 1024 / 1024 / 1024).toFixed(2)} GB`);
        });
    } catch (error) {
        console.log(`✗ Failed: ${error.message}`);
    }
}

async function main() {
    console.log('=== WinRM Connection Test ===');
    console.log(`Target: ${config.hostname}:${config.port}`);

    // Test Administrator account
    const adminSuccess = await testConnection(
        config.adminUsername,
        config.adminPassword,
        'Administrator Account'
    );

    if (adminSuccess) {
        await getSystemInfo(config.adminUsername, config.adminPassword);
        await checkGPU(config.adminUsername, config.adminPassword);
    }

    // Test guacamole account
    await testConnection(
        config.username,
        config.password,
        'Guacamole Account'
    );

    console.log('\n=== Test Complete ===\n');
}

main().catch(console.error);
