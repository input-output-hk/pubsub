#!/usr/bin/env node
// Local preview of the site in web/, including *.draft.html pages that the
// deploy leaves out. Reloads the browser whenever a file under web/ changes.
// No dependencies: needs only Node 18+.
//
//   npm run slides          # opens the year-end draft deck
//   npm run web             # opens the site's home page
//   PORT=9000 npm run web   # another port

import { createServer } from 'node:http';
import { readFile, stat } from 'node:fs/promises';
import { watch } from 'node:fs';
import { extname, join, normalize, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(fileURLToPath(new URL('../web', import.meta.url)));
const port = Number(process.env.PORT) || 8000;
const openPath = process.argv[2] || '/';

const types = {
  '.html': 'text/html; charset=utf-8', '.css': 'text/css', '.js': 'text/javascript',
  '.mjs': 'text/javascript', '.json': 'application/json', '.svg': 'image/svg+xml',
  '.png': 'image/png', '.jpg': 'image/jpeg', '.ico': 'image/x-icon',
  '.txt': 'text/plain; charset=utf-8', '.xml': 'application/xml', '.pdf': 'application/pdf',
};

// Injected before </body> of every HTML page: reload when the server says so.
const reloadSnippet = `<script>new EventSource('/__reload').onmessage = () => location.reload();</script>`;
const clients = new Set();

const server = createServer(async (req, res) => {
  const url = new URL(req.url, 'http://localhost');
  if (url.pathname === '/__reload') {
    res.writeHead(200, { 'Content-Type': 'text/event-stream', 'Cache-Control': 'no-cache' });
    res.write(': connected\n\n');
    clients.add(res);
    req.on('close', () => clients.delete(res));
    return;
  }
  let file = normalize(join(root, decodeURIComponent(url.pathname)));
  if (file !== root && !file.startsWith(root + sep)) { res.writeHead(403).end(); return; }
  try {
    if ((await stat(file)).isDirectory()) file = join(file, 'index.html');
    let body = await readFile(file);
    const type = types[extname(file)] || 'application/octet-stream';
    if (type.startsWith('text/html')) body = body.toString().replace('</body>', reloadSnippet + '</body>');
    res.writeHead(200, { 'Content-Type': type, 'Cache-Control': 'no-store' }).end(body);
  } catch {
    res.writeHead(404, { 'Content-Type': 'text/plain' }).end('Not found');
  }
});

let pending;
watch(root, { recursive: true }, () => {
  clearTimeout(pending);
  pending = setTimeout(() => clients.forEach((c) => c.write('data: reload\n\n')), 100);
});

server.listen(port, () => {
  console.log(`Serving web/ at http://localhost:${port}${openPath}`);
  console.log('Edits under web/ reload the page. Ctrl+C to stop.');
});
