
process.env.WINDOWS_PASSWORD = "<MASQUE — le fichier vivant, gitignore, porte la valeur>";
process.env.WINDOWS_ADMIN_USERNAME = "<MASQUE — le fichier vivant, gitignore, porte la valeur>";
process.env.WINDOWS_ADMIN_PASSWORD = "<MASQUE — le fichier vivant, gitignore, porte la valeur>";
const express = require('express');
const http = require('http');


const session = require('./src/session');
const asset = require('./src/asset');
const apps = require('./src/app');
const onExit = require('./src/cleanup');

const app = express();

const server = http.createServer(app, {
    keepAlive: true
});

app.get('/apps', async (req, res) => {
    try {
        await apps.wait;
        res.json(apps);
    } catch (e) {
        console.error('Error waiting for apps:', e);
        res.status(500).json({ error: 'Failed to load apps' });
    }
});

asset(app);
const sessionManager = session(app, server);

// Setup graceful shutdown
onExit(async (options) => {
    console.log('Shutting down server...');

    // Cleanup all active sessions
    if (sessionManager && sessionManager.cleanupAll) {
        try {
            await sessionManager.cleanupAll();
        } catch (e) {
            console.error('Error during session cleanup:', e);
        }
    }

    // Close server
    server.close(() => {
        console.log('Server closed');
    });

    // Force exit after 10 seconds
    setTimeout(() => {
        console.warn('Forcing exit after timeout');
        process.exit(0);
    }, 10000);
});




server.listen(3445);
