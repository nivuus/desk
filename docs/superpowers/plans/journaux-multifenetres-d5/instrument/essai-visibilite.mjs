#!/usr/bin/env node
// Épreuve isolée : l'override de `document.hidden` posé par
// `Page.addScriptToEvaluateOnNewDocument` prend-il sur une page d'arrière-plan
// d'un Chrome sans interface ? Et l'événement `visibilitychange` s'y
// déclenche-t-il de lui-même ?
import { spawn, spawnSync } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const dodo = (ms) => new Promise((r) => setTimeout(r, ms));
const AMORCE = `
  window.__marque = 'amorce-passee';
  window.__cachee = false;
  Object.defineProperty(document, 'hidden', { configurable: true, get: () => window.__cachee });
  Object.defineProperty(document, 'visibilityState', {
    configurable: true, get: () => (window.__cachee ? 'hidden' : 'visible') });
  window.__evts = [];
  document.addEventListener('visibilitychange', () => window.__evts.push('vis:' + document.hidden));
  window.addEventListener('focus', () => window.__evts.push('focus'));
  window.addEventListener('blur', () => window.__evts.push('blur'));
`;

class Cdp {
    constructor(wsUrl) {
        this.ws = new WebSocket(wsUrl); this.nextId = 1; this.pending = new Map(); this.handlers = [];
        this.ready = new Promise((r) => this.ws.addEventListener('open', () => r(), { once: true }));
        this.ws.addEventListener('message', (e) => {
            const m = JSON.parse(String(e.data));
            if (m.id !== undefined && this.pending.has(m.id)) {
                const { resolve, reject } = this.pending.get(m.id); this.pending.delete(m.id);
                m.error ? reject(new Error(JSON.stringify(m.error))) : resolve(m.result);
            } else if (m.method) for (const h of this.handlers) h(m);
        });
    }
    on(h) { this.handlers.push(h); }
    async send(method, params = {}, sessionId) {
        await this.ready; const id = this.nextId++;
        const msg = { id, method, params }; if (sessionId) msg.sessionId = sessionId;
        return new Promise((res, rej) => { this.pending.set(id, { resolve: res, reject: rej }); this.ws.send(JSON.stringify(msg)); });
    }
    async ev(sid, expr) {
        const r = await this.send('Runtime.evaluate', { expression: expr, returnByValue: true }, sid);
        return r.exceptionDetails ? { __ex: JSON.stringify(r.exceptionDetails).slice(0, 300) } : r.result.value;
    }
}

const port = 9995;
const udd = await mkdtemp(join(tmpdir(), 'chrome-essai-'));
const chrome = spawn('google-chrome', ['--headless=new', `--remote-debugging-port=${port}`,
    '--remote-allow-origins=*', `--user-data-dir=${udd}`, '--no-sandbox', '--disable-gpu',
    '--disable-popup-blocking', '--window-size=800,600', 'about:blank'], { stdio: 'ignore' });
try {
    let v;
    for (let i = 0; i < 60; i++) { try { const r = await fetch(`http://127.0.0.1:${port}/json/version`); if (r.ok) { v = await r.json(); break; } } catch { } await dodo(250); }
    const cdp = new Cdp(v.webSocketDebuggerUrl);
    const pages = new Map();
    cdp.on(async (m) => {
        if (m.method === 'Target.attachedToTarget') {
            const { sessionId, targetInfo } = m.params;
            if (targetInfo.type !== 'page') { await cdp.send('Runtime.runIfWaitingForDebugger', {}, sessionId).catch(() => { }); return; }
            pages.set(sessionId, targetInfo.url);
            await cdp.send('Page.enable', {}, sessionId).catch(() => { });
            await cdp.send('Runtime.enable', {}, sessionId).catch(() => { });
            await cdp.send('Page.addScriptToEvaluateOnNewDocument', { source: AMORCE }, sessionId).catch((e) => console.log('!! addScript', String(e).slice(0, 200)));
            await cdp.send('Runtime.runIfWaitingForDebugger', {}, sessionId).catch(() => { });
        } else if (m.method === 'Target.targetInfoChanged') {
            for (const [sid] of pages) if (m.params.targetInfo.targetId && pages.has(sid)) pages.set(sid, m.params.targetInfo.url);
        }
    });
    await cdp.send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: true, flatten: true });
    await cdp.send('Target.setDiscoverTargets', { discover: true });

    // Deux façons d'ouvrir : Target.createTarget (comme la shell ne le fait
    // pas) et window.open depuis une page (comme la shell le fait).
    await cdp.send('Target.createTarget', { url: 'data:text/html,<title>A</title>A' });
    await dodo(1500);
    const sidA = [...pages].find(([, u]) => u.includes('A'))?.[0] ?? [...pages.keys()][0];
    await cdp.ev(sidA, `(() => { window.open('data:text/html,<title>B</title>B', '_blank', 'width=400,height=300'); return 'ouvert'; })()`);
    await dodo(1500);

    for (const [sid, url] of pages) {
        const r = await cdp.ev(sid, `JSON.stringify({
            marque: window.__marque ?? null, hidden: document.hidden, vs: document.visibilityState,
            focus: document.hasFocus(), evts: window.__evts ?? null,
            proprietaire: Object.getOwnPropertyDescriptor(document, 'hidden') ? 'document' : 'prototype' })`);
        console.log(`page ${sid.slice(0, 6)} url=${String(url).slice(0, 40)} → ${r}`);
    }

    // Puis : la bascule et l'événement, sur la page d'arrière-plan.
    const sidFond = [...pages.keys()][pages.size - 2];
    console.log('bascule sur', sidFond.slice(0, 6),
        await cdp.ev(sidFond, `(() => { window.__cachee = true; document.dispatchEvent(new Event('visibilitychange'));
             return JSON.stringify({ hidden: document.hidden, evts: window.__evts }); })()`));
} finally {
    chrome.kill('SIGKILL');
    await rm(udd, { recursive: true, force: true }).catch(() => { });
}
