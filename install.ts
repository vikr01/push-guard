import fs from 'fs';
import path from 'path';
import https from 'https';
import stream from 'stream';
import util from 'util';
import { IncomingMessage } from 'http';

const pipe = util.promisify(stream.pipeline);

const VERSION: string = process.env.npm_package_version!;
const REPO: string = new URL(process.env.npm_package_repository_url!).pathname.slice(1);
const isWin: boolean = process.platform === 'win32';

const TARGETS: Record<string, string> = {
  'darwin-arm64': 'push-guard-aarch64-apple-darwin',
  'darwin-x64':   'push-guard-x86_64-apple-darwin',
  'linux-arm64':  'push-guard-aarch64-unknown-linux-gnu',
  'linux-x64':    'push-guard-x86_64-unknown-linux-gnu',
  'win32-x64':    'push-guard-x86_64-pc-windows-msvc',
};

const key: string = process.platform + '-' + process.arch;
const target: string | undefined = TARGETS[key];

if (!target) {
  console.error('push-guard: unsupported platform ' + key);
  process.exit(1);
}

const ext: string = isWin ? '.zip' : '.tar.xz';
const url: string = 'https://github.com/' + REPO + '/releases/download/v' + VERSION + '/' + target + ext;
const dest: string = path.join(__dirname, 'push-guard');

function fetch(url: string): Promise<IncomingMessage> {
  return new Promise(function (resolve, reject) {
    https.get(url, function (res) {
      if (res.statusCode === 301 || res.statusCode === 302) {
        fetch(res.headers.location!).then(resolve, reject);
      } else if (res.statusCode !== 200) {
        reject(new Error('HTTP ' + res.statusCode));
      } else {
        resolve(res);
      }
    }).on('error', reject);
  });
}

fetch(url)
  .then(function (res) {
    return pipe(res, fs.createWriteStream(dest));
  })
  .then(function () {
    if (!isWin) fs.chmodSync(dest, 0o755);
  })
  .catch(function (err: Error) {
    console.error('push-guard install failed:', err.message);
    process.exit(1);
  });
