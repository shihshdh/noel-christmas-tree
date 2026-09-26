// 本地静态服务：让页面跑在 http://localhost 上。
// 浏览器只在 localhost / https 下稳定地允许摄像头，Worker 和本地模块也只能在 http 下加载。
// 用法：双击「启动圣诞树.bat」，或 node serve.mjs [端口]
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { exec } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const ROOT = path.dirname(fileURLToPath(import.meta.url));
const TYPES = {
  '.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8', '.mjs': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8', '.json': 'application/json', '.wasm': 'application/wasm', '.task': 'application/octet-stream',
  '.png': 'image/png', '.jpg': 'image/jpeg', '.jpeg': 'image/jpeg', '.svg': 'image/svg+xml', '.mp3': 'audio/mpeg', '.ico': 'image/x-icon'
};

const server = http.createServer((req, res) => {
  let rel = decodeURIComponent(new URL(req.url, 'http://x').pathname);
  if (rel.endsWith('/')) rel += 'index.html';
  const file = path.join(ROOT, rel);
  if (!file.startsWith(ROOT)) { res.writeHead(403).end(); return; }
  fs.stat(file, (err, st) => {
    if (err || !st.isFile()) { res.writeHead(404, { 'Content-Type': 'text/plain; charset=utf-8' }).end('没有这个文件'); return; }
    const ext = path.extname(file).toLowerCase();
    res.writeHead(200, {
      'Content-Type': TYPES[ext] || 'application/octet-stream',
      'Content-Length': st.size,
      // 大文件（模型、wasm、three）长期缓存，页面本身每次取最新
      'Cache-Control': rel.startsWith('/vendor/') ? 'public, max-age=604800' : 'no-cache'
    });
    fs.createReadStream(file).pipe(res);
  });
});

let port = +(process.argv[2] || 5288);
server.on('error', err => {
  if (err.code === 'EADDRINUSE' && port < 5300) { port++; server.listen(port, '127.0.0.1'); }
  else { console.error(err); process.exit(1); }
});
// 用独立的 Chrome 窗口打开，并强制走高性能显卡（独显）。
// 单独的配置目录保证启动参数生效：已开着的 Chrome 会忽略新参数。
function findChrome() {
  const bases = [process.env['PROGRAMFILES'], process.env['PROGRAMFILES(X86)'], process.env.LOCALAPPDATA].filter(Boolean);
  for (const b of bases) {
    const p = path.join(b, 'Google', 'Chrome', 'Application', 'chrome.exe');
    if (fs.existsSync(p)) return p;
  }
  const edge = path.join(process.env['PROGRAMFILES(X86)'] || '', 'Microsoft', 'Edge', 'Application', 'msedge.exe');
  return fs.existsSync(edge) ? edge : null;
}
server.on('listening', () => {
  const url = `http://localhost:${port}/`;
  console.log(`\n  圣诞树已启动：${url}\n  关闭这个窗口即停止。\n`);
  if (process.env.NO_OPEN) return;
  const browser = process.platform === 'win32' ? findChrome() : null;
  if (!browser) { exec(process.platform === 'win32' ? `start "" "${url}"` : `open "${url}"`); return; }
  const profile = path.join(process.env.LOCALAPPDATA || ROOT, 'NoelTree', 'BrowserProfile');
  const flags = ['--force_high_performance_gpu', '--ignore-gpu-blocklist', '--enable-gpu-rasterization', '--no-first-run', '--no-default-browser-check', '--start-maximized', `--user-data-dir="${profile}"`, `--app=${url}`];
  exec(`start "" "${browser}" ${flags.join(' ')}`);
  console.log(`  已用高性能显卡模式打开：${path.basename(browser)}\n`);
});
server.listen(port, '127.0.0.1');
