const express = require('express');
const gulp = require('gulp');
const babel = require('gulp-babel');
const browserify = require('gulp-browserify');
const GuacamoleLite = require('guacamole-lite');
const http = require('http');
const uuid = require('uuid');
const { spawn } = require('child_process');
const onExit = require('./cleanup');
const apps = require('./app');
const sharp = require('sharp');
const ico = require("to-ico");
const rgbToHex = require('rgb2hex');

module.exports = function (app) {
    app.use('/dist', express.static(__dirname + '/../dist'));
    app.use('/assets', express.static(__dirname + '/../assets'));

    gulp.src(__dirname + '/../web/index.js')
        .pipe(babel({
            presets: ['@babel/env']
        }))
        .pipe(browserify({
            insertGlobals: true,
            debug: true
        }))
        .pipe(gulp.dest(__dirname + '/../dist'));

    gulp.src(__dirname + '/../web/home.js')
        .pipe(babel({
            presets: ['@babel/env']
        }))
        .pipe(browserify({
            insertGlobals: true,
            debug: true
        }))
        .pipe(gulp.dest(__dirname + '/../dist'));




    app.get('/:app/manifest.json', (req, res) => {
        const short = req.params.app;
        const app = apps.find(app => app.short == short);
        const manifest = {
            name: app.name,
            description: 'test',
            start_url: "/" + app.short,
            "xxscope": "/" + app.short,
            "scope": "/" + app.short,
            "icons": [
                {
                    "src": `/${app.short}/image/192x192.png`,
                    "sizes": "192x192",
                    "type": "image/png"
                },
                {
                    "src": `/${app.short}/image/512x512.png`,
                    "sizes": "512x512",
                    "type": "image/png"
                },
                {
                    "src": `/${app.short}/image/32x32.png`,
                    "sizes": "32x32",
                    "type": "image/png"
                }
            ],
            "screenshots" : [
                {
                  "src": `/${app.short}/image/screenshot.png`,
                  "sizes": "1280x720",
                  "type": "image/webp",
                  "form_factor": "wide",
                  "label": "Transparent"
                },
               
              ],
            "theme_color": `#${rgbToHex(`rgb(${app.color.join(',')})`)}`,
            "background_color": `#${rgbToHex(`rgb(${app.color.join(',')})`)}`,
            "display": "standalone",
            "display_override": ["window-controls-overlay"],
            "file_handlers":
                app.fileHandlers && app.fileHandlers.length > 0 ?
                (() => {
                    const acceptsByMimeType = {};
                    app.fileHandlers.forEach(handler => {
                        if (handler.mimeType && handler.extension) {
                            if (!acceptsByMimeType[handler.mimeType]) {
                                acceptsByMimeType[handler.mimeType] = [];
                            }
                            if (!acceptsByMimeType[handler.mimeType].includes(handler.extension)) {
                                acceptsByMimeType[handler.mimeType].push(handler.extension);
                            }
                        }
                    });
                    return Object.keys(acceptsByMimeType).length > 0 ? [{
                        action: "/" + app.short,
                        accept: acceptsByMimeType
                    }] : [];
                })() : []
        };
        res.status(200).json(manifest);
    });
    

    app.get('/sw.js', (req, res) => {
        res.sendFile('web/sw.js', {
            root: './'
        });
    });

    app.get('/', (req, res) => {
        res.sendFile('assets/index.html', {
            root: './'
        });
    });

    app.get('/:app/image/screenshot.png', async (req, res) => {
        const buffer = await sharp({
            create: {
              width: 1280,
              height: 720,
              channels: 4,
              background: { r: 255, g: 255, b: 255, alpha: 0 }
            }
        }).png().toBuffer()
        

        res.set('Content-Type', 'image/png');
        res.send(buffer);

    });

    app.get('/:app/image/:type', async (req, res) => {
        const short = req.params.app;
        const app = apps.find(app => app.short == short);

        const match = app.icon.match(/^data:([^\;]+);base64,(.*)$/);
        const base64 = match[2];

        res.set('Content-Type', match[1])

        const buffer = Buffer.from(base64, "base64");
        const matchSize = req.params.type.match(/([0-9]+)x([0-9]+)/);
        if (req.params.type == 'favicon.ico') {
            const size32 = await sharp(buffer).resize(32, 32).toBuffer();
            const buf = await ico([size32], {
                sizes: [32],
                resize: true
            });
            res.set('Content-Type', 'image/x-icon');
            res.send(buf);
        }
        else if (!matchSize) {
           
            res.send(buffer);
        }
        else {
            const x = parseInt(matchSize[1]);
            const y = parseInt(matchSize[2]);
            const resizeBuffer = await sharp(buffer).resize(x, y).toBuffer();
            res.send(resizeBuffer);
        }
    });
};