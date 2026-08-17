const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');

const configPath = require.resolve('../next.config.js');

function loadConfig(nodeEnv) {
  process.env.NODE_ENV = nodeEnv;
  delete require.cache[configPath];
  return require(configPath);
}

test('development and production use separate Next.js build directories', () => {
  assert.equal(loadConfig('development').distDir, '.next-dev');
  assert.equal(loadConfig('production').distDir, '.next');
});

test('Tauri waits on the warmed frontend gate', () => {
  const config = JSON.parse(
    fs.readFileSync(path.join(__dirname, '../src-tauri/tauri.conf.json')),
  );
  assert.equal(config.build.devUrl, 'http://localhost:3119');
  assert.match(config.app.security.devCsp['connect-src'], /ws:\/\/localhost:3119/);
});
