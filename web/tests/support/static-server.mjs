// Minimal loopback-only static file server for the SCN-13 suite's
// Playwright webServer. Uses the same Node runtime that runs the tests
// (no external interpreter to locate/spawn), and binds 127.0.0.1 only.
//
// Usage: node static-server.mjs <root-dir> <port>
//
// TRACE: SCN-13

import http from "node:http";
import fs from "node:fs";
import path from "node:path";

const root = path.resolve(process.argv[2] || ".");
const port = Number(process.argv[3] || 4319);
const HOST = "127.0.0.1";

const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".png": "image/png",
  ".svg": "image/svg+xml",
  ".map": "application/json; charset=utf-8",
  ".md": "text/markdown; charset=utf-8",
  // WebAssembly.instantiateStreaming rejects any other MIME; without this
  // the wasm-bindgen glue falls back to arrayBuffer() and logs a warning.
  ".wasm": "application/wasm",
};

const server = http.createServer((req, res) => {
  try {
    const url = new URL(req.url, `http://${HOST}:${port}`);
    let pathname = decodeURIComponent(url.pathname);
    if (pathname.endsWith("/")) pathname += "index.html";

    // Contain every request under root — reject path traversal.
    const resolved = path.normalize(path.join(root, pathname));
    if (resolved !== root && !resolved.startsWith(root + path.sep)) {
      res.writeHead(403).end("Forbidden");
      return;
    }
    if (!fs.existsSync(resolved) || !fs.statSync(resolved).isFile()) {
      res.writeHead(404).end("Not Found");
      return;
    }
    const type = TYPES[path.extname(resolved).toLowerCase()] || "application/octet-stream";
    res.writeHead(200, { "content-type": type });
    fs.createReadStream(resolved).pipe(res);
  } catch (err) {
    res.writeHead(500).end(String(err));
  }
});

server.listen(port, HOST, () => {
  // eslint-disable-next-line no-console
  console.log(`static-server: serving ${root} at http://${HOST}:${port}`);
});
