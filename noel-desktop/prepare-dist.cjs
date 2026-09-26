// 把圣诞树页面（../html）复制到 dist，供 Tauri 打包进客户端。
// 不复制备份、本地服务脚本和启动器；three.js、手势模型等 vendor 文件原样保留。
const fs = require('fs');
const path = require('path');

const src = path.resolve(__dirname, '..', 'html');
const dist = path.join(__dirname, 'dist');
if (!fs.existsSync(path.join(src, 'index.html'))) throw new Error('没有找到 ../html/index.html');

const skip = rel => {
  const top = rel.split(path.sep)[0];
  return top.startsWith('_backup') || /^(serve\.mjs|.*\.bat|我的网页开发\.html)$/.test(top);
};
fs.rmSync(dist, { recursive: true, force: true });
fs.cpSync(src, dist, { recursive: true, filter: p => { const rel = path.relative(src, p); return !rel || !skip(rel); } });
let files = 0, bytes = 0;
(function walk(dir) { for (const e of fs.readdirSync(dir, { withFileTypes: true })) { const p = path.join(dir, e.name); if (e.isDirectory()) walk(p); else { files++; bytes += fs.statSync(p).size; } } })(dist);
console.log(`dist: ${files} 个文件，${(bytes / 1048576).toFixed(1)} MB`);
