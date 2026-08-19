const e = require('express');
const { WebSocket } = require('ws');
const {FtpSrv, FileSystem} = require('ftp-srv');
const {exec} = require("child_process");
const util = require("util");
const Fuse = require('fuse-native');
const {v4} = require('uuid');
const stream = require('stream');
const fs = require('fs').promises;
const execPromise = util.promisify(exec);

// LRU Cache implementation
class LRUCache {
  constructor(maxSize = 100, maxMemory = 100 * 1024 * 1024) { // 100 MB max
    this.maxSize = maxSize;
    this.maxMemory = maxMemory;
    this.cache = new Map();
    this.currentMemory = 0;
  }

  get(key) {
    if (!this.cache.has(key)) return undefined;
    const value = this.cache.get(key);
    // Move to end (most recently used)
    this.cache.delete(key);
    this.cache.set(key, value);
    return value;
  }

  set(key, value) {
    // Remove if already exists
    if (this.cache.has(key)) {
      const old = this.cache.get(key);
      this.currentMemory -= old.data ? old.data.length : 0;
      this.cache.delete(key);
    }

    // Calculate new size
    const newSize = value.data ? value.data.length : 0;
    this.currentMemory += newSize;

    // Evict LRU items if needed
    while (this.cache.size >= this.maxSize || this.currentMemory > this.maxMemory) {
      const firstKey = this.cache.keys().next().value;
      const firstValue = this.cache.get(firstKey);
      this.currentMemory -= firstValue.data ? firstValue.data.length : 0;
      this.cache.delete(firstKey);
    }

    this.cache.set(key, value);
  }

  delete(key) {
    if (this.cache.has(key)) {
      const value = this.cache.get(key);
      this.currentMemory -= value.data ? value.data.length : 0;
      this.cache.delete(key);
    }
  }

  clear() {
    this.cache.clear();
    this.currentMemory = 0;
  }
}

const ports = [];
const START_PORT = 21;

async function createFS(ws, path) {
  // Track active operations for graceful shutdown
  let activeOperations = 0;
  let isShuttingDown = false;
  const pendingCallbacks = new Map();

  const stat = function (st) {
    return {
      mtime: st.mtime,
      atime: st.atime || st.mtime,
      ctime: st.ctime || st.mtime,
      size: st.size !== undefined ? st.size : 0,
      mode: st.mode === 'dir' ? 16877 : (st.mode === 'file' ? 33152 : (st.mode === 'link' ? 41453 : st.mode)),
      uid: st.uid !== undefined ? st.uid : process.getuid(),
      gid: st.gid !== undefined ? st.gid : process.getgid()
    }
  };

  // Enhanced send function with timeout and proper cleanup
  async function send(data, callback, timeout = 10000) {
    if (isShuttingDown) {
      return callback(new Error('Filesystem is shutting down'));
    }

    const uuid = v4();
    const [client] = ws.clients;

    if (!client || client.readyState !== WebSocket.OPEN) {
      return callback(new Error('No WebSocket client connected'));
    }

    activeOperations++;
    let timeoutHandle;
    let isResolved = false;

    const cleanup = () => {
      if (isResolved) return;
      isResolved = true;

      if (timeoutHandle) {
        clearTimeout(timeoutHandle);
      }
      pendingCallbacks.delete(uuid);
      activeOperations--;
    };

    const onNewMessage = function incoming(message) {
      try {
        let parsed = JSON.parse(message.toString('utf8'));

        // Ignore heartbeat messages
        if (parsed.heartbeat) return;
        if (parsed.id !== uuid) return;

        cleanup();

        if (parsed.error) {
          const error = new Error(parsed.error);
          return callback(error);
        }

        let parsedData = parsed.data;
        if (parsed.isBinary) {
          const uint8Array = new Int8Array(Object.values(parsedData));
          parsedData = Buffer.from(uint8Array);
        }

        callback(parsedData);
        client.off('message', onNewMessage);
      } catch (e) {
        cleanup();
        console.error('Error processing filesystem message:', e);
        callback(e);
        client.off('message', onNewMessage);
      }
    };

    // Setup timeout
    timeoutHandle = setTimeout(() => {
      cleanup();
      client.off('message', onNewMessage);
      callback(new Error(`Filesystem operation timeout after ${timeout}ms`));
    }, timeout);

    pendingCallbacks.set(uuid, { onNewMessage, cleanup });
    client.on('message', onNewMessage);

    try {
      const stringified = JSON.stringify({ data, id: uuid });
      client.send(stringified);
    } catch (e) {
      cleanup();
      client.off('message', onNewMessage);
      callback(new Error('Failed to send message: ' + e.message));
    }
  }

  // Use LRU cache instead of simple object
  const cachedFiles = new LRUCache(100, 50 * 1024 * 1024); // 50 MB cache

  const ops = {
    statfs: function (path, cb) {
      // Return realistic values instead of 1000000000
      cb(0, {
        bsize: 4096,
        frsize: 4096,
        blocks: 104857600, // 400 GB
        bfree: 52428800,   // 200 GB free
        bavail: 52428800,
        files: 10000000,
        ffree: 5000000,
        favail: 5000000,
        fsid: 1000000,
        flag: 0,
        namemax: 255
      })
    },
    readdir: function (path, cb) {
      send({ readdir: path }, (data) => {
        if (data instanceof Error) return cb(-1);
        if (data === null) return cb(Fuse.ENOENT);
        return cb(null, data.filter((d) => d.match(/^\./) === null));
      });
    },
    getattr: function (path, cb) {
      // Check cache first for metadata
      const cached = cachedFiles.get(path);
      if (cached && cached.stat && (Date.now() - cached.statTime < 5000)) {
        // Use cached stat if less than 5 seconds old
        return process.nextTick(cb, 0, cached.stat);
      }

      send({ getattr: path }, (data) => {
        if (data instanceof Error) return cb(-1);
        if (data === null) return cb(Fuse.ENOENT);

        const statFile = stat(data);

        // Update cache with new stat info
        const existing = cachedFiles.get(path) || { data: null };
        cachedFiles.set(path, {
          ...existing,
          stat: statFile,
          statTime: Date.now(),
          mtime: statFile.mtime
        });

        return process.nextTick(cb, 0, statFile);
      });
    },
    open: function (path, flags, cb) {
      //send({ open: path }, cb);
      return cb(0, 42)
    },
    release: function (path, fd, cb) {
      //send({ release: path }, cb);
      return cb(0)
    },
    read: function (path, fd, buffer, length, position, cb) {
      const cached = cachedFiles.get(path);

      // Check if we have valid cached data
      if (cached && cached.data && Buffer.isBuffer(cached.data)) {
        const cachedData = cached.data;

        // Check if the requested range is within cached data
        if (position + length <= cachedData.length) {
          const slice = cachedData.slice(position, position + length);
          slice.copy(buffer);
          return cb(slice.length);
        }
      }

      // Fetch from remote
      send({ read: path, position, length }, (data) => {
        if (data instanceof Error) return cb(-1);
        if (data === null) return cb(0);

        data.copy(buffer);

        // Update cache - store complete file data if small enough
        if (position === 0 && data.length < 10 * 1024 * 1024) { // Cache files < 10 MB
          const existing = cached || { stat: null, statTime: 0 };
          cachedFiles.set(path, {
            ...existing,
            data: data,
            cachedTime: Date.now()
          });
        }

        return cb(data.length);
      });
    },
    write: function (path, fd, buffer, length, position, cb) {
      const d = Array.from(buffer.slice(0, length));

      send({ write: path, position, length, data: d }, (data) => {
        if (data instanceof Error) return cb(-1);

        // Invalidate cache on write - file has changed
        cachedFiles.delete(path);

        cb(length);
      });
    },
    create(path, mode, cb) {
      send({ create: path, mode }, (data) => {
        if (data instanceof Error) return cb(-1);

        // Invalidate parent directory cache
        const parentPath = path.substring(0, path.lastIndexOf('/')) || '/';
        cachedFiles.delete(parentPath);

        process.nextTick(cb, 0, data);
      });
    },
    mkdir(path, mode, cb) {
      send({ mkdir: path }, (data) => {
        if (data instanceof Error) return cb(-1);

        // Invalidate parent directory cache
        const parentPath = path.substring(0, path.lastIndexOf('/')) || '/';
        cachedFiles.delete(parentPath);

        cb(0);
      });
    },
    rmdir(path, cb) {
      send({ rmdir: path }, (data) => {
        if (data instanceof Error) return cb(-1);

        // Invalidate caches
        cachedFiles.delete(path);
        const parentPath = path.substring(0, path.lastIndexOf('/')) || '/';
        cachedFiles.delete(parentPath);

        cb(0);
      });
    },
    unlink(path, cb) {
      send({ unlink: path }, (data) => {
        if (data instanceof Error) return cb(-1);

        // Invalidate caches
        cachedFiles.delete(path);
        const parentPath = path.substring(0, path.lastIndexOf('/')) || '/';
        cachedFiles.delete(parentPath);

        cb(0);
      });
    },
    rename(oldPath, newPath, cb) {
      send({ rename: oldPath, new: newPath } , (data) => {
        if (data instanceof Error) return cb(-1);

        // Invalidate all related caches
        cachedFiles.delete(oldPath);
        cachedFiles.delete(newPath);
        const oldParent = oldPath.substring(0, oldPath.lastIndexOf('/')) || '/';
        const newParent = newPath.substring(0, newPath.lastIndexOf('/')) || '/';
        cachedFiles.delete(oldParent);
        cachedFiles.delete(newParent);

        cb(0);
      });
    }
  }

  const fuse = new Fuse(path, ops, { debug: false, mkdir: true })
  await fuse.mount(console.error);

  // Graceful shutdown function
  return async () => {
    console.log(`Shutting down filesystem ${path}...`);
    isShuttingDown = true;

    // Wait for active operations to complete (max 5 seconds)
    const startTime = Date.now();
    const maxWait = 5000;

    while (activeOperations > 0 && (Date.now() - startTime) < maxWait) {
      console.log(`Waiting for ${activeOperations} active operations to complete...`);
      await new Promise(resolve => setTimeout(resolve, 100));
    }

    if (activeOperations > 0) {
      console.warn(`Forcing shutdown with ${activeOperations} operations still active`);

      // Clean up any pending callbacks
      pendingCallbacks.forEach(({ cleanup }) => {
        try {
          cleanup();
        } catch (e) {
          console.error('Error during callback cleanup:', e);
        }
      });
      pendingCallbacks.clear();
    }

    // Clear cache
    cachedFiles.clear();

    // Unmount FUSE
    try {
      console.log(`Unmounting ${path}...`);
      await execPromise(`fusermount -u ${path}`);
      console.log(`Unmounted ${path} successfully`);
    } catch (e) {
      console.error(`Error unmounting ${path}:`, e.message);
      // Try force unmount
      try {
        await execPromise(`fusermount -uz ${path}`);
      } catch (e2) {
        console.error(`Force unmount also failed:`, e2.message);
      }
    }

    // Clean up mount directory
    try {
      await execPromise(`rm -rf ${path}`);
    } catch (e) {
      console.error(`Error removing ${path}:`, e.message);
    }

    console.log(`Filesystem ${path} shutdown complete`);
  }
}

module.exports = async function (uuid, ws) {

    // run command
    const path = `/mnt/ftp-${uuid}`;
    // await execPromise(`mkdir ${path}`);
    const onClose = await createFS(ws, path);

   


    return {
      path,
      close() {
        return onClose();
      }
    };
}
