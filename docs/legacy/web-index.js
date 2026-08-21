const Guacamole = require('guacamole-common-js');
const { over, set } = require('lodash');

async function encrypt(value, clientOptions) {
    const encoder = new TextEncoder();
    const data = encoder.encode(JSON.stringify(value));

    const iv = crypto.getRandomValues(new Uint8Array(16));
    const key = await crypto.subtle.importKey(
        'raw',
        encoder.encode(clientOptions.key),
        { name: 'AES-CBC', length: 256 },
        false,
        ['encrypt']
    );

    const encryptedData = await crypto.subtle.encrypt(
        { name: 'AES-CBC', iv: iv },
        key,
        data
    );

    const encryptedArray = new Uint8Array(encryptedData);
    const base64Encrypted = btoa(String.fromCharCode(...encryptedArray));

    const result = {
        iv: btoa(String.fromCharCode(...iv)),
        value: base64Encrypted
    };

    return btoa(JSON.stringify(result));
}

function debounce(func, wait, immediate) {
    var timeout;

    return function executedFunction() {
        var context = this;
        var args = arguments;

        var later = function () {
            timeout = null;
            if (!immediate) func.apply(context, args);
        };

        var callNow = immediate && !timeout;

        clearTimeout(timeout);

        timeout = setTimeout(later, wait);

        if (callNow) func.apply(context, args);
    };
};

let scale = 1;


async function resize(guac) {
    

    const display = guac.getDisplay();
    const layer = display.getDefaultLayer();

    scale = 1;//window.devicePixelRatio * 0.4;
     

    
    guac.sendSize(window.innerWidth * scale, window.innerHeight * scale);
    display.scale(1/scale);
    display.resize(layer, (window.innerWidth + 1) * scale, window.innerHeight * scale);
    // var xscale = window.innerWidth * origWidth;
    // var yscale = window.innerHeight / origHeigth;
    // console.log(xscale, yscale, window.devicePixelRatio);
    // //This is done to handle both X and Y axis slacing
    // scale = Math.min(xscale, yscale);
    //Add 10% to scale because window always less than screen resolution
    //scale += scale ;
    //Change Cuacamole Display scale
    //display.scale(1/window.devicePixelRatio);
    //layer.getElement().style['transform'] = `scale(${1/(window.devicePixelRatio)})`;
    //console.log('scale', layer.getElement()); 
}

function blobToBase64(blob) {
    return new Promise((res, _) => {
        const reader = new FileReader();
        reader.onloadend = () => res(reader.result);
        reader.readAsDataURL(blob);
    });
}

function getDPI() {
    const d = document.createElement('div');
    d.style = 'height: 1in; left: -100%; position: absolute; top: -100%; width: 1in; visibility: hidden;';
    document.body.appendChild(d);
    const devicePixelRatio = window.devicePixelRatio || 1;
    const dpi_x = Math.round(d.offsetWidth * devicePixelRatio);
    d.remove();
    return dpi_x;
}

(async function () {

    // Get display div from document
    var display = document.getElementById("display");
    const matched = window.location.pathname.match(/^\/([^\/]+)\/(.*)/);
    const appName = matched[1];
    const sessionID = matched[2];

    const tunnel = new Guacamole.WebSocketTunnel(`/${appName}/${sessionID}`);
    tunnel.setUUID(sessionID);

    // Tunnel state change handler - close window when tunnel is definitively closed
    tunnel.onstatechange = function(state) {
        console.log('Tunnel state changed:', state);
        if (state === Guacamole.Tunnel.State.CLOSED) {
            console.log('Tunnel closed - will close window in 1 second');
            // Wait 1 second to ensure this is a real closure, not a premature one
            setTimeout(function() {
                window.close();
            }, 1000);
        } else if (state === Guacamole.Tunnel.State.UNSTABLE) {
            console.log('Tunnel unstable - attempting to reconnect');
            // Don't close on unstable - allow potential reconnection
        }
    };

    tunnel.onerror = function(error) {
        console.log('Tunnel error:', error);
        // Don't immediately close - let the state change handler deal with it
    };

    // Instantiate client, using an HTTP tunnel for communications.
    var guac = new Guacamole.Client(
        tunnel
    );

    // Add client to display div
    display.appendChild(guac.getDisplay().getElement());

    // Error handler
    guac.onerror = function (error) {
        console.log('error:', error);
    };

    // Connect

    const clientOptions = {
        cypher: 'AES-256-CBC',
        key: sessionID
    }
    console.log(getDPI());
    const token = await encrypt({
        "connection": {
            "type": "rdp",
            "settings": {
                "dpi": getDPI() + "",
                "color-depth": 24 + "",
            }
        }
    }, clientOptions);

    resize(guac);

    //const token = "***RETIRE-DE-L-HISTORIQUE***";
    guac.connect("token=" + token + "&height=" + (window.innerHeight * scale) + '&width=' + ((window.innerWidth + 1) * scale) + '&GUAC_AUDIO=audio/L16');

    // Disconnect on close
    window.onunload = function () {
        guac.disconnect();
    }

    guac.onaudio = function clientAudio(stream, mimetype) {

        let context = Guacamole.AudioContextFactory.getAudioContext();
 
        context.resume()
    }

    // Mouse
    var mouse = new Guacamole.Mouse(guac.getDisplay().getElement());
    var touch = new Guacamole.Mouse.Touchscreen(guac.getDisplay().getElement());

    // let touchOrMouseDownStart = null;
    // mouse.onmousedown = function (mouseState) {
    //     touchOrMouseDownStart = JSON.parse(JSON.stringify(mouseState));
    //     guac.sendMouseState(mouseState);
    // };
    // mouse.onmouseup = function (mouseState) {
    //     if (titleBarAreaRect && touchOrMouseDownStart && touchOrMouseDownStart.x < titleBarAreaRect.right && touchOrMouseDownStart.y < titleBarAreaRect.bottom && (mouseState.x > titleBarAreaRect.right || mouseState.y > titleBarAreaRect.bottom))
    //         return;
    //     guac.sendMouseState(mouseState);
    // };
    // mouse.onmousemove = function (mouseState) {
    //     if (titleBarAreaRect && touchOrMouseDownStart && touchOrMouseDownStart.x < titleBarAreaRect.right && touchOrMouseDownStart.y < titleBarAreaRect.bottom && mouseState.left === true)
    //         return;
    //     mouseState.y =  mouseState.y * scale;
    //     mouseState.x =  mouseState.x * scale;

    //     guac.sendMouseState(mouseState);
    // };

    // touch.onmousedown = function (touchState) {
    //     touchOrMouseDownStart = touchState;
    //     guac.sendMouseState(touchState);
    // }
    // touch.onmouseup = function (touchState) {      
    //     guac.sendMouseState(touchState);
    // };
    
    // touch.onmousemove = function (touchState) {
    //     if (titleBarAreaRect && touchOrMouseDownStart && touchOrMouseDownStart.x < titleBarAreaRect.right && touchOrMouseDownStart.y < titleBarAreaRect.bottom)
    //         return;
    //     touchState.y =  touchState.y * scale;
    //     touchState.x =  touchState.x * scale;
    
    //     guac.sendMouseState(touchState);
    // };

    let touchOrMouseDownStart = null;
    let isDragging = false;
    let startX, startY;
    let initialWindowX, initialWindowY;
    let isDraggingTitleBar = false;
    let titleBarAreaRect = null;

    let mouseScreenX = 0, mouseScreenY = 0;
    guac.getDisplay().getElement().addEventListener('mousemove', (e) => {
        mouseScreenX = e.screenX;
        mouseScreenY = e.screenY;
    });

    mouse.onmousedown = function (mouseState) {
        touchOrMouseDownStart = JSON.parse(JSON.stringify(mouseState));
        startX = mouseScreenX;
        startY = mouseScreenY;
        initialWindowX = window.screenX;
        initialWindowY = window.screenY;
        isDragging = true;
        guac.sendMouseState(mouseState);
    };

    mouse.onmouseup = function (mouseState) {
        isDragging = false;
        if (isDraggingTitleBar) {
            isDraggingTitleBar = false;
            touchOrMouseDownStart.left = false;
            console.log(touchOrMouseDownStart);
            guac.sendMouseState(touchOrMouseDownStart);
            return;
        }
        document.body.style.cursor = 'default';
        guac.sendMouseState(mouseState);
    };

    mouse.onmousemove = function (mouseState) {
        if (isDragging && titleBarAreaRect && touchOrMouseDownStart.x < titleBarAreaRect.right && touchOrMouseDownStart.y < titleBarAreaRect.bottom) {
            isDraggingTitleBar = true;
            const dx = mouseScreenX - startX;
            const dy = mouseScreenY - startY;
            console.log(dx, dy, initialWindowX, initialWindowY, mouseScreenX, mouseScreenY, startX, startY);
            window.moveTo(initialWindowX + dx, initialWindowY + dy);
            return;
        }
        mouseState.y = mouseState.y * scale;
        mouseState.x = mouseState.x * scale;
        guac.sendMouseState(mouseState);
    };

    touch.onmousedown = function (touchState) {
        touchOrMouseDownStart = touchState;
        startX = touchState.clientX;
        startY = touchState.clientY;
        initialWindowX = window.screenX;
        initialWindowY = window.screenY;
        isDragging = true;
        document.body.style.cursor = 'move';
        guac.sendMouseState(touchState);
    };

    touch.onmouseup = function (touchState) {
        isDragging = false;
        if (isDraggingTitleBar) {
            isDraggingTitleBar = false;
            touchOrMouseDownStart.left = false;
            console.log(touchOrMouseDownStart)
            guac.sendMouseState(touchOrMouseDownStart);
            return;
        }
        document.body.style.cursor = 'default';
        guac.sendMouseState(touchState);
    };

    touch.onmousemove = function (touchState) {
        if (isDragging && titleBarAreaRect && touchOrMouseDownStart.x < titleBarAreaRect.right && touchOrMouseDownStart.y < titleBarAreaRect.bottom) {
            isDraggingTitleBar = true;
            const dx = mouseScreenX - startX;
            const dy = mouseScreenY - startY;
            window.moveTo(initialWindowX + dx, initialWindowY + dy);
            return;
        }
        touchState.y = touchState.y * scale;
        touchState.x = touchState.x * scale;
        guac.sendMouseState(touchState);
    };

    // Keyboard
    var keyboard = new Guacamole.Keyboard(document);

    keyboard.onkeydown = function (keysym) {
        guac.sendKeyEvent(1, keysym);
    };

    keyboard.onkeyup = function (keysym) {
        guac.sendKeyEvent(0, keysym);
    };

    guac.getDisplay().showCursor(false);

    window.addEventListener('resize', debounce(() => {
        resize(guac);
        setTimeout(() => {
            setOverlay();
        }, 250);
    }, 250));
    //window.addEventListener('load', () => resize(guac));


    const handleServerClipboardChange = (stream, mimetype) => {
        console.log('clipboard', stream, mimetype);
        // don't do anything if this is not active element
        // if (document.activeElement !== displayRef.current)
        //     return;
        stream.onblob = async function (base64) {
            if (!navigator.clipboard)
                return;
            const res = await fetch(`data:${mimetype};base64,${base64}`)

            //get a blob you can do whatever you like with
            const blob = await res.blob();
            const data = [new ClipboardItem({ [mimetype]: blob })];
            navigator.clipboard.write(data);
        };
    };

    let currentCopiedItems = [];
    // Read client's clipboard
    const onFocusHandler = async () => {

        let context = Guacamole.AudioContextFactory.getAudioContext();
        context.resume()

        if (!navigator.clipboard)
            return;

        // when focused, read client clipboard text
        const permission = await navigator.permissions.query({ name: 'clipboard-read' });

        if (permission.state === 'denied') {
            throw new Error('Not allowed to read clipboard.');
        }

        const items = await navigator.clipboard.read();

        const blobToBase64s = [];
        await items.reduce(async (acc, item) => {
            await acc;
            const blob = await item.getType(item.types[0]);
            const blobAsDataUrl = await blobToBase64(blob);
            const blobAsB64 = blobAsDataUrl.split(",")[1];

            if (!!currentCopiedItems.find((currentCopiedItem) => {
                return blobAsB64 == currentCopiedItem;
            })) {
                blobToBase64s.push(blobAsB64);
                return;
            }

            let stream = guac.createClipboardStream(item.types[0], "remote");
            await stream.sendBlob(blob);
            stream.onack = () => {
                stream.sendEnd();
            }
            blobToBase64s.push(blobAsB64);
            await stream.sendEnd();
        }, Promise.resolve());
        currentCopiedItems = blobToBase64s;
    };

    window.addEventListener("click", onFocusHandler);
    window.addEventListener("keypress", onFocusHandler);
    guac.onclipboard = handleServerClipboardChange;

    guac.onstatechange = function (code) {
        if (code === 5) {
            window.close();
            document.getElementById('closed').style.opacity = 1;
        }

        if (code === 3) {
            setTimeout(() => {
                document.getElementById('loading').style.opacity = 0;
            }, 1000);
        }

    }

    guac.onfile = function () {
        console.log(arguments);
    }

    window.addEventListener('unload', () => {
        guac.endStream();
    })


    if ('serviceWorker' in navigator) {
        navigator.serviceWorker.register(
            '/sw.js',
            {
                scope: `/${appName}/`
            }
        );
    }

    const ws = new WebSocket(`wss://${window.location.host}/${appName}/${sessionID}/filesystem`);

    // Filesystem WebSocket handlers - don't close window on failure
    ws.onclose = function() {
        console.log('Filesystem WebSocket closed (filesystem features disabled)');
    };

    ws.onerror = function(error) {
        console.log('Filesystem WebSocket error:', error, '(filesystem features disabled)');
    };

    // Heartbeat to keep connection alive (every 30 seconds)
    let heartbeatInterval;
    ws.onopen = function() {
        console.log('Filesystem WebSocket connected, starting heartbeat');
        heartbeatInterval = setInterval(() => {
            if (ws.readyState === WebSocket.OPEN) {
                try {
                    ws.send(JSON.stringify({ heartbeat: true }));
                } catch (e) {
                    console.log('Failed to send heartbeat (filesystem features disabled)');
                    clearInterval(heartbeatInterval);
                }
            } else {
                console.log('WebSocket not open (filesystem features disabled)');
                clearInterval(heartbeatInterval);
            }
        }, 30000);
    };

    let fsHandler;
    async function getFSHandler() {
        if (!fsHandler) {
            try {
                fsHandler = await window.showDirectoryPicker({
                    id: appName,
                    mode: 'readwrite'
                });
            } catch (e) {
                console.error('User denied directory picker or error occurred:', e);
                return null;
            }
        }
        return fsHandler;
    }

    async function getFileOrDirectoryHandler(path, createIfNotExist, isFile) {
        const directoryHandler = await getFSHandler();
        if (!directoryHandler) {
            throw new Error('No directory handler available');
        }
        if (path === '')
            return directoryHandler;

        
        const parts = path.split('/');
        let current = directoryHandler;
        for (let i = 0; i < parts.length; i++) {
            const part = parts[i];

            if (part === '')
                continue;
            try {
                if (isFile && i === (parts.length - 1)) {
                    current = await current.getFileHandle(part, {
                        create: createIfNotExist
                    });
                }
                else {
                    current = await current.getDirectoryHandle(part, {
                        create: createIfNotExist
                    });
                }
            } catch (e) {
                return await current.getFileHandle(part);
            }
        }
        return current;
    }

    ws.onmessage = async function (message) {
        const parsed = JSON.parse(message.data);
        try {
            const result = await (async (data) => {

                if (data.getattr) {
                    const dirPath = data.getattr.replace(/^\//, '');
                    if (dirPath === '')
                        return {
                            mode: 'dir',
                            size: 1000000000
                        };
                    else {
                        try {
                            const d = await getFileOrDirectoryHandler(dirPath);

                            if (d instanceof FileSystemDirectoryHandle) {
                                return {
                                    mode: 'dir',
                                    size: 1048576
                                };
                            }
                            else if (d instanceof FileSystemFileHandle) {
                                const file = await d.getFile();
                                console.log(file);
                                return {
                                    mtime: file.lastModified,
                                    mode: 'file',
                                    size: file.size
                                };
                            }
                        } catch (e) {
                            return null;           
                        }
                    }
                }
                else if (data.readdir) {
                    const dirPath = data.readdir.replace(/^\//, '');

                    try {

                        const dir = await getFileOrDirectoryHandler(dirPath);
                        const entries = [];
                        for await (const entry of dir.values()) {
                            entries.push(entry.name);
                        }
                        return entries;
                    } catch (e) {
                        return null;
                    }
                    
                }
                else if (data.read) {
                    const filePath = data.read.replace(/^\//, '');
                    const file = await getFileOrDirectoryHandler(filePath);
                    const fileStream = await file.getFile();
                    const arrayBuffer = await fileStream.arrayBuffer();
                    return arrayBuffer.slice(data.position, data.position + data.length);
                }
                else if (data.write) {
                    const filePath = data.write.replace(/^\//, '');

                    let uint8Array = new Uint8Array(data.data);
                    const file = await getFileOrDirectoryHandler(filePath);
                    const writable = await file.createWritable({
                        keepExistingData: true
                    });
                    await writable.write({
                        data: uint8Array,
                        type: "write",
                        position: data.position,
                        size: data.length
                    });
                    await writable.close();


                    return {};
                }
                else if (data.mkdir) {
                    const dirPath = data.mkdir.replace(/^\//, '');

                    await getFileOrDirectoryHandler(dirPath, true);
                    return {};
                }
                else if (data.create) {

                    const dirPath = data.create.replace(/^\//, '');

                    console.log(data);

                    await getFileOrDirectoryHandler(dirPath, true, true);

                    return 42;
                }
                else if (data.rmdir) {
                    const dirPath = data.rmdir.replace(/^\//, '');

                    const directory = await getFileOrDirectoryHandler(dirPath);
                    await directory.remove();
                    return {};
                }
                else if (data.unlink) {
                    const dirPath =  data.unlink.replace(/^\//, '');

                    const directory = await getFileOrDirectoryHandler(dirPath);
                    await directory.remove();
                    return {};
                }
                else if (data.rename) {
                    const dirPath = data.rename.replace(/^\//, '');
                    const newPath = data.new.replace(/^\//, '');
                    

                    const oldFile = await getFileOrDirectoryHandler(dirPath);
                    if (oldFile instanceof FileSystemDirectoryHandle) {
                        const newDirectory = await getFileOrDirectoryHandler(newPath, true);
                        await (async function moveFilesRecursively(oldDir, newDir) {
                            for await (const entry of oldDir.values()) {
                                if (entry.kind === 'file') {
                                    //const file = await entry.getFile();
                                    console.log(newDir, entry.name);
                                    await entry.move(newDir, entry.name);
                                }
                                else if (entry.kind === 'directory') {
                                    const newDir = await newDir.getDirectoryHandle(entry.name, {
                                        create: true
                                    });
                                    console.log(newDir);
                                    await moveFilesRecursively(entry, newDir);
                                }
                            }
                        })(oldFile, newDirectory);
                        await oldFile.remove();
                    }
                    else {
                        const newParentPath = newPath.split('/').slice(0, -1).join('/');
                        const newParent = await getFileOrDirectoryHandler(newParentPath);
                        await oldFile.move(newParent, newPath.split('/').pop());
                    }
                    

                    return {};
                }
                return {};
            })(parsed.data);

            const isBinary = result instanceof ArrayBuffer;
            let resultData = result;
            if (isBinary) {
                resultData = new Int8Array(result);
            }
            
            ws.send(JSON.stringify({
                data: resultData,
                id: parsed.id,
                isBinary
            }));
        }
        catch (e) {
            console.error(e);
            ws.send(JSON.stringify({
                id: parsed.id,
                error: JSON.stringify(e)
            }));
        }
        
        

    }

    // window.addEventListener('focus', async () => {

    //     const [handle] = await window.showDirectoryPicker();
    //     console.log(handle);
    // })
    let intervalRefresh;
    function setOverlay() {
        const overlay = navigator.windowControlsOverlay;
        clearInterval(intervalRefresh);
        if (overlay.visible) {
            titleBarAreaRect = overlay.getTitlebarAreaRect();
            const marginTop = titleBarAreaRect.bottom - 23;
            document.body.style.marginTop = `${marginTop}px`;
            const canva = guac.getDisplay().getElement().querySelector('canvas');
            
            intervalRefresh = setInterval(() => {
                const color = getPixelColor(canva, titleBarAreaRect.left + titleBarAreaRect.width, titleBarAreaRect.bottom - marginTop);
                document.body.style.backgroundColor = color;
                document.head.querySelector('meta[name="theme-color"]').content = color;
            }, 3000);
            const color = getPixelColor(canva, titleBarAreaRect.left + titleBarAreaRect.width, titleBarAreaRect.bottom - marginTop);
            document.body.style.backgroundColor = color;
            document.head.querySelector('meta[name="theme-color"]').content = color;
        } else {
            console.log("Le window controls overlay n'est pas visible.");
            document.body.style.marginTop = `0px`;
        }
    }

    if ('windowControlsOverlay' in navigator) {
        const overlay = navigator.windowControlsOverlay;
    
        overlay.ongeometrychange = () => {
            setTimeout(() => {
                setOverlay();
            }, 250);
        };
        setOverlay();
    } else {
        console.log("L'API windowControlsOverlay n'est pas supportée par ce navigateur.");
    }

    // const canvas = guac.getDisplay().getElement();
    // let isDragging = false;
    // let startX, startY, startClientX, startClientY;
    // let initialWindowX, initialWindowY;

    // canvas.addEventListener('mousedown', (e) => {
    //     isDragging = true;
    //     startX = e.screenX;
    //     startY = e.screenY;
    //     startClientX = e.clientX;
    //     startClientY = e.clientY;
    //     initialWindowX = window.screenX;
    //     initialWindowY = window.screenY;
    //     //e.preventDefault(); // Empêcher le comportement par défaut
    // });

    // canvas.addEventListener('mousemove', (e) => {
    //     if (isDragging && startClientX < titleBarAreaRect.right && startClientY < titleBarAreaRect.bottom) {
    //         e.stopImmediatePropagation();
    //         const dx = e.screenX - startX;
    //         const dy = e.screenY - startY;
    //         window.moveTo(initialWindowX + dx, initialWindowY + dy);
    //         e.preventDefault(); // Empêcher le comportement par défaut
    //         e.stopPropagation();
    //     }
    //     else {
    //         console.log('not dragging');
    //     }
    // });

    // canvas.addEventListener('mouseup', (e) => {
    //     if (isDragging) {
    //         isDragging = false;
    //         document.body.style.cursor = 'default';
    //     }
    // });

    // canvas.addEventListener('mouseleave', (e) => {
    //     if (isDragging) {
    //         isDragging = false;
    //         document.body.style.cursor = 'default';
    //     }
    // });

})();

function getPixelColor(canvas, x, y) {
    const context = canvas.getContext('2d');
    const imageData = context.getImageData(x, y, 1, 1).data;
    const [r, g, b, a] = imageData;
    return `rgba(${r}, ${g}, ${b}, ${a / 255})`;
}

// const regionDrag = document.querySelector('canvas');
// let isDragging = false;
// let startX, startY;
// let initialWindowX, initialWindowY;

// regionDrag.addEventListener('mousedown', (e) => {
//     isDragging = true;
//     startX = e.screenX;
//     startY = e.screenY;
//     initialWindowX = window.screenX;
//     initialWindowY = window.screenY;
//     //document.body.style.cursor = 'move';
// });

// regionDrag.addEventListener('mousemove', (e) => {
//     if (isDragging) {
//         const dx = e.screenX - startX;
//         const dy = e.screenY - startY;
//         window.moveTo(initialWindowX + dx, initialWindowY + dy);
//     }
// });

// regionDrag.addEventListener('mouseup', (e) => {
//     isDragging = false;
//     document.body.style.cursor = 'default';

// });

// regionDrag.addEventListener('mouseleave', (e) => {
//     isDragging = false;
//     document.body.style.cursor = 'default';
// });

