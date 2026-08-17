#!/usr/bin/env node

const { spawn } = require('node:child_process');
const http = require('node:http');
const net = require('node:net');

const frontendUrl = 'http://localhost:3118';
const frontendPort = 3118;
const readinessPort = 3119;
const next = spawn(
  process.execPath,
  [require.resolve('next/dist/bin/next'), 'dev', '-p', '3118'],
  { stdio: 'inherit' },
);

let nextExited = false;
next.once('exit', (code) => {
  nextExited = true;
  process.exit(code ?? 1);
});

async function waitForFrontend() {
  while (!nextExited) {
    try {
      const response = await fetch(frontendUrl);
      if (response.ok) {
        await response.text();
        return;
      }
    } catch {}
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error('Next.js exited before the frontend was ready');
}

async function main() {
  await waitForFrontend();
  const server = http.createServer((request, response) => {
    const proxyRequest = http.request(
      {
        hostname: '127.0.0.1',
        port: frontendPort,
        path: request.url,
        method: request.method,
        headers: { ...request.headers, host: `localhost:${frontendPort}` },
      },
      (proxyResponse) => {
        response.writeHead(proxyResponse.statusCode ?? 502, proxyResponse.headers);
        proxyResponse.pipe(response);
      },
    );
    proxyRequest.on('error', () => response.destroy());
    request.pipe(proxyRequest);
  });

  server.on('upgrade', (request, socket, head) => {
    const upstream = net.connect(frontendPort, '127.0.0.1', () => {
      const headers = request.rawHeaders
        .reduce((lines, value, index, values) => {
          if (index % 2 === 0) {
            lines.push(`${value}: ${value.toLowerCase() === 'host' ? `localhost:${frontendPort}` : values[index + 1]}`);
          }
          return lines;
        }, [])
        .join('\r\n');
      upstream.write(
        `${request.method} ${request.url} HTTP/${request.httpVersion}\r\n${headers}\r\n\r\n`,
      );
      upstream.write(head);
      socket.pipe(upstream).pipe(socket);
    });
    upstream.on('error', () => socket.destroy());
  });

  server.listen(readinessPort);
}

main().catch((error) => {
  console.error(error.message);
  next.kill();
  process.exitCode = 1;
});
