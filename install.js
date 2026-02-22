'use strict';

var fs = require('fs');
var path = require('path');
var https = require('https');
var stream = require('stream');
var util = require('util');

var pipe = util.promisify(stream.pipeline);

var VERSION = process.env.npm_package_version;
var REPO = new URL(process.env.npm_package_repository_url).pathname.slice(1);
var isWin = process.platform === 'win32';

var TARGETS = {
  'darwin-arm64': 'push-guard-aarch64-apple-darwin',
  'darwin-x64':   'push-guard-x86_64-apple-darwin',
  'linux-arm64':  'push-guard-aarch64-unknown-linux-gnu',
  'linux-x64':    'push-guard-x86_64-unknown-linux-gnu',
  'win32-x64':    'push-guard-x86_64-pc-windows-msvc',
};

var key = process.platform + '-' + process.arch;
var target = TARGETS[key];

if (!target) {
  console.error('push-guard: unsupported platform ' + key);
  process.exit(1);
}

var ext = isWin ? '.zip' : '.tar.xz';
var url = 'https://github.com/' + REPO + '/releases/download/v' + VERSION + '/' + target + ext;
var dest = path.join(__dirname, 'push-guard');

function fetch(url) {
  return new Promise(function (resolve, reject) {
    https.get(url, function (res) {
      if (res.statusCode === 301 || res.statusCode === 302) {
        fetch(res.headers.location).then(resolve, reject);
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
  .catch(function (err) {
    console.error('push-guard install failed:', err.message);
    process.exit(1);
  });
