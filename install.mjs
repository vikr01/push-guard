import { createWriteStream, chmodSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { pipeline } from 'node:stream/promises';
import { get } from 'node:https';

const VERSION = process.env.npm_package_version;
const REPO = new URL(process.env.npm_package_repository_url).pathname.slice(1);
const dir = dirname(fileURLToPath(import.meta.url));

const TARGETS = {
  'darwin-arm64':  'push-guard-aarch64-apple-darwin',
  'darwin-x64':    'push-guard-x86_64-apple-darwin',
  'linux-arm64':   'push-guard-aarch64-unknown-linux-gnu',
  'linux-x64':     'push-guard-x86_64-unknown-linux-gnu',
  'win32-x64':     'push-guard-x86_64-pc-windows-msvc',
};

const key = `${process.platform}-${process.arch}`;
const target = TARGETS[key];

if (!target) {
  console.error(`push-guard: unsupported platform ${key}`);
  process.exit(1);
}

const isWin = process.platform === 'win32';
const ext = isWin ? '.zip' : '.tar.xz';
const binName = isWin ? 'push-guard.exe' : 'push-guard';
const url = `https://github.com/${REPO}/releases/download/v${VERSION}/${target}${ext}`;
// Always written as `push-guard` (no .exe) — npm's bin wiring handles Windows execution
const dest = join(dir, 'push-guard');

function fetch(url) {
  return new Promise((resolve, reject) => {
    get(url, res => {
      if (res.statusCode === 301 || res.statusCode === 302) {
        fetch(res.headers.location).then(resolve).catch(reject);
      } else if (res.statusCode !== 200) {
        reject(new Error(`HTTP ${res.statusCode}`));
      } else {
        resolve(res);
      }
    }).on('error', reject);
  });
}

// tar.xz extraction tracked in TODOS.md — needs a proper tar library
const res = await fetch(url);
await pipeline(res, createWriteStream(dest));
if (!isWin) chmodSync(dest, 0o755);
