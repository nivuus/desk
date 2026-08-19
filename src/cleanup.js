process.stdin.resume();//so the program will not close instantly

let cleanupCallbacks = [];

function exitHandler(options, exitCode) {
    if (options.cleanup) console.log('clean');
    if (exitCode || exitCode === 0) console.log(exitCode);
    if (options.exit) process.exit();
}

async function asyncExitHandler(options, exitCode) {
    if (options.cleanup) {
        console.log('Running cleanup callbacks...');

        // Run all registered cleanup callbacks
        for (const callback of cleanupCallbacks) {
            try {
                await callback(options, exitCode);
            } catch (e) {
                console.error('Error in cleanup callback:', e);
            }
        }
    }

    if (exitCode || exitCode === 0) console.log('Exit code:', exitCode);
    if (options.exit) process.exit();
}

//do something when app is closing
process.on('exit', exitHandler.bind(null,{cleanup:true}));

//catches ctrl+c event
process.on('SIGINT', asyncExitHandler.bind(null, {exit:true}));

// catches "kill pid" (for example: nodemon restart)
process.on('SIGUSR1', asyncExitHandler.bind(null, {exit:true}));
process.on('SIGUSR2', asyncExitHandler.bind(null, {exit:true}));

//catches uncaught exceptions
process.on('uncaughtException', (err) => {
    console.error('Uncaught exception:', err);
    asyncExitHandler({exit:true}, 1);
});

// Register a cleanup callback
module.exports = function(callback) {
    cleanupCallbacks.push(callback);
};