console.log('test');

// Helper to check if app is installed
function isAppInstalled(appShort) {
    const installedApps = JSON.parse(localStorage.getItem('installedApps') || '[]');
    return installedApps.includes(appShort);
}

// Helper to mark app as installed
function markAppInstalled(appShort) {
    const installedApps = JSON.parse(localStorage.getItem('installedApps') || '[]');
    if (!installedApps.includes(appShort)) {
        installedApps.push(appShort);
        localStorage.setItem('installedApps', JSON.stringify(installedApps));
    }
}

// Helper to mark app as uninstalled
function markAppUninstalled(appShort) {
    const installedApps = JSON.parse(localStorage.getItem('installedApps') || '[]');
    const filtered = installedApps.filter(app => app !== appShort);
    localStorage.setItem('installedApps', JSON.stringify(filtered));
}

async function addApp(app) {
    const div = document.createElement('div');
    const right = document.createElement('div');
    const title = document.createElement('h3');
    const img = document.createElement('img');
    const action = document.createElement('div');
    const launch = document.createElement('a');
    const install = document.createElement('a');

    title.textContent = app.name;
    action.classList.add('action');

    launch.href = `/${app.short}`;
    launch.textContent = 'Launch';
    launch.target = '_blank';

    img.src = app.icon;

    // Check if app is installed from localStorage
    let isInstalled = isAppInstalled(app.short);

    // Update button text based on installation status
    function updateInstallButton() {
        install.textContent = isInstalled ? 'Uninstall' : 'Install';
    }
    updateInstallButton();

    install.href = `#`;
    install.onclick = async (e) => {
        e.preventDefault();

        if (isInstalled) {
            // Mark as uninstalled and guide user
            markAppUninstalled(app.short);
            isInstalled = false;
            updateInstallButton();
            alert(`${app.name} marqué comme désinstallé.\n\nPour désinstaller complètement, allez dans:\nChrome/Edge: Menu > Applications > Gérer les applications\nFirefox: about:preferences > Applications Web`);
        } else {
            // Open the app in a new window/tab to trigger PWA install
            const appWindow = window.open(`/${app.short}`, '_blank', 'width=1200,height=800');

            // Mark as installed after a short delay
            setTimeout(() => {
                markAppInstalled(app.short);
                isInstalled = true;
                updateInstallButton();
            }, 2000);

            alert(`${app.name} va s'ouvrir dans une nouvelle fenêtre.\n\nPour l'installer comme application:\n• Chrome/Edge: Cliquez sur l'icône d'installation dans la barre d'adresse\n• Firefox: Menu > Installer cette application`);
        }
    };

    action.appendChild(launch);
    action.appendChild(install);
    right.appendChild(title);
    right.appendChild(action);

    div.appendChild(img);
    div.appendChild(right);
    document.querySelector('#apps').appendChild(div);
}

fetch('/apps')
    .then(res => res.json())
    .then(apps => {
        apps.forEach(app => {
            return addApp(app);
        });
    })
    .catch(console.error);