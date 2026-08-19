const express = require('express');
const gulp = require('gulp');
const babel = require('gulp-babel');
const browserify = require('gulp-browserify');
const GuacamoleLite = require('guacamole-lite');
const fs = require('fs').promises;
const http = require('http');
const uuid = require('uuid');
const { spawn } = require('child_process');
const onExit = require('./cleanup');
const apps = require('./app');
const createFileSystem = require('./file');
const { WebSocketServer } = require('ws');
const lodash = require('lodash');
const winrm = require('nodejs-winrm');
const waitPort = require('wait-port');

// Cleanup function for graceful shutdown
async function cleanupAllSessions(guacdInstances) {
    console.log('Cleaning up all active sessions...');
    const sessionIds = Object.keys(guacdInstances);

    if (sessionIds.length === 0) {
        console.log('No active sessions to clean up');
        return;
    }

    console.log(`Cleaning up ${sessionIds.length} active sessions`);

    const cleanupPromises = sessionIds.map(async (sessionId) => {
        const instance = guacdInstances[sessionId];
        try {
            console.log(`Cleaning up session ${sessionId}...`);

            if (instance.guacServer) {
                instance.guacServer.close();
            }

            if (instance.ws) {
                instance.ws.close();
            }

            if (instance.fileSystem && instance.fileSystem.close) {
                await instance.fileSystem.close();
            }

            console.log(`Session ${sessionId} cleaned up`);
        } catch (e) {
            console.error(`Error cleaning up session ${sessionId}:`, e);
        }
    });

    await Promise.all(cleanupPromises);
    console.log('All sessions cleaned up');
}

module.exports = function (app, server) {

    let guacdInstances = {};

    // Helper function to run PowerShell commands via WinRM
    async function runInPowerShell(command) {
        await waitPort({
            host: process.env.WINDOWS_HOSTNAME,
            port: 5985,
            output: 'silent'
        });
        return winrm.runCommand(command, process.env.WINDOWS_HOSTNAME, process.env.WINDOWS_ADMIN_USERNAME, process.env.WINDOWS_ADMIN_PASSWORD, 5985, true);
    }

    const templateData = {};

    async function getAppHtml(short, content) {
        if (!templateData[short]) {
            const content = await fs.readFile('./assets/app.tpl.html', 'utf8');
            templateData[short] = lodash.template(content);
        }
        return templateData[short](content);
    }

    app.get('/:app/:uuid', async (req, res) => {
        if (!guacdInstances[req.params.uuid]) {
            res.redirect(`/${req.params.app}`);
        }
        else {
            const short = req.params.app;

            try {
                await apps.wait;
            } catch (e) {
                console.error('Error waiting for apps:', e);
                return res.status(500).send('Failed to load applications');
            }

            const app = apps.find(app => app.short == short);
            if (!app) {
                return res.status(404).send('Not found');
            }

            console.log(app);
            const content = await getAppHtml(short, app);

            res.set('ContentType', 'application/json')
                .send(content);

            // res.sendFile('assets/app.html', {
            //     root: './'
            // });
        }
    });





    const child = spawn('guacd', ['-b', '0.0.0.0', '-l', `4822`, '-f'], {

    });

    child.stdout.setEncoding('utf8');
    child.stdout.on('data', function (data) {
        //Here is where the output goes

        console.log('stdout: ' + data);
    });

    child.stderr.setEncoding('utf8');
    child.stderr.on('data', function (data) {
        //Here is where the error output goes

        console.log('stderr: ' + data);
    });

    onExit(() => {
        child.kill('SIGUSR1');
    });

    app.get('/:app', async (req, res) => {
        const short = req.params.app;

        try {
            await apps.wait;
        } catch (e) {
            console.error('Error waiting for apps:', e);
            return res.status(500).send('Failed to load applications');
        }

        const app = apps.find(app => app.short == short);
        if (!app) {
            return res.status(404).send('Not found');
        }

        const guacdIndex = uuid.v4().slice(0, 32);

        const ws = new WebSocketServer({
            noServer: true,

        });

        const upgradeHandler = function upgrade(request, socket, head) {
            const pathname = request.url;

            if (pathname.startsWith(`/${app.short}/${guacdIndex}/filesystem`)) {
                console.log('upgrade');
                ws.handleUpgrade(request, socket, head, function done(ws) {
                    ws.emit('connection', ws, request);
                });
            }
        };

        server.on('upgrade', upgradeHandler);

        const fileSystem = await createFileSystem(guacdIndex, ws);

        let port = 4822;

        const guacdOptions = {
            host: '127.0.0.1',
            port: port
        };


        const clientOptions = {
            crypt: {
                cypher: 'AES-256-CBC',
                key: guacdIndex
            },
            connectionDefaultSettings: {
                "rdp": {
                    "hostname": process.env.WINDOWS_HOSTNAME,
                    "username": process.env.WINDOWS_USERNAME,

                    "client-name": guacdIndex + 'p',
                    "domain": "WORKGROUP",
                    "cursor": "remote",
                    "password": process.env.WINDOWS_PASSWORD,
                    "enable-printing": true,
                    "disable-upload": true,
                    "disable-download": true,
                    "enable-drive": true,
                    "create-drive-path": false,
                    "drive-name": "Mes Fichiers",
                    "drive-path": fileSystem.path,
                    "security": "auto",
                    "ignore-cert": true,
                    "remote-app": app.program,

                    "disable-reconnect": true,
                    "resize-method": 'display-update',
                    "normalize-clipboard": "windows",
                    //"force-lossless": true,
                    "color-depth": "24",
                    "enable-audio": true,
                    "enable-audio-input": true,
                 
                    "enable-touch": true,
                    "enable-menu-animations": false,
                    "enable-font-smoothing": true,
                    "dpi": "500"

                }
            }
        };

        const callbackOptions = {
            processConnectionSettings: function (settings, callback) {
                settings.name = guacdIndex + 'p';
                settings.connection.type = "rdp";
                settings.connection.name = guacdIndex + 'p';

                callback(null, settings);
            }
        };

        const guacServer = new GuacamoleLite({
            server,
            skipUTF8Validation: true,
            noServer: true,
            allowSynchronousEvents : true,
            path: `/${app.short}/${guacdIndex}`
        }, guacdOptions, clientOptions, callbackOptions);

        // Active session monitoring - DISABLED
        // const clientName = guacdIndex + 'p';
        // let sessionCheckInterval;

        // async function checkRdpSession() {
        //     try {
        //         const result = await runInPowerShell(
        //             `qwinsta /server:${process.env.WINDOWS_HOSTNAME} | Select-String "${clientName}"`
        //         );
        //         return result && result.includes(clientName);
        //     } catch (e) {
        //         console.error(`Error checking RDP session ${clientName}:`, e.message);
        //         return false;
        //     }
        // }

        guacServer.on('close', async (clientConnection) => {
            console.log('Client disconnected ' + `/${app.short}/${guacdIndex}`);
            // clearInterval(sessionCheckInterval);

            try {
                // Close Guacamole server
                guacServer.close();

                // Close WebSocket server
                ws.close();

                // Gracefully shutdown filesystem
                if (fileSystem && fileSystem.close) {
                    console.log('Closing filesystem for session', guacdIndex);
                    await fileSystem.close();
                }

                // Remove upgrade handler
                server.removeListener('upgrade', upgradeHandler);

                // Delete session instance
                delete guacdInstances[guacdIndex];

                console.log('Session cleanup complete for', guacdIndex);
            } catch (e) {
                console.error('Error during session cleanup:', e);
            }
        });


        guacServer.on('error', console.error);

        guacdInstances[guacdIndex] = {
            port,
            guacServer,
            ws,
            fileSystem
        }

        res.redirect(`/${app.short}/${guacdIndex}`);
    });

    // Export cleanup function
    return {
        cleanupAll: () => cleanupAllSessions(guacdInstances)
    };
};