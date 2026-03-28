#!/usr/bin/env node

const { execSync } = require("child_process");
const fs = require("fs");
const path = require("path");
const https = require("https");
const http = require("http");

const REPO = "Akram012388/cc-dm-stream";
const BINARY = "cc-dm-stream";

const PLATFORM_MAP = {
  darwin: {
    arm64: "aarch64-apple-darwin",
    x64: "x86_64-apple-darwin",
  },
  linux: {
    arm64: "aarch64-unknown-linux-gnu",
    x64: "x86_64-unknown-linux-gnu",
  },
};

function getTarget() {
  const platform = PLATFORM_MAP[process.platform];
  if (!platform) {
    throw new Error(`Unsupported platform: ${process.platform}`);
  }
  const target = platform[process.arch];
  if (!target) {
    throw new Error(
      `Unsupported architecture: ${process.arch} on ${process.platform}`
    );
  }
  return target;
}

function getVersion() {
  const pkg = require("./package.json");
  return pkg.version;
}

function download(url) {
  return new Promise((resolve, reject) => {
    const client = url.startsWith("https") ? https : http;
    client
      .get(url, { headers: { "User-Agent": "cc-dm-stream-npm" } }, (res) => {
        if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
          return download(res.headers.location).then(resolve).catch(reject);
        }
        if (res.statusCode !== 200) {
          return reject(new Error(`HTTP ${res.statusCode} for ${url}`));
        }
        const chunks = [];
        res.on("data", (chunk) => chunks.push(chunk));
        res.on("end", () => resolve(Buffer.concat(chunks)));
        res.on("error", reject);
      })
      .on("error", reject);
  });
}

async function main() {
  const target = getTarget();
  const version = getVersion();
  const asset = `${BINARY}-${target}`;
  const url = `https://github.com/${REPO}/releases/download/v${version}/${asset}`;

  console.log(`Downloading ${BINARY} v${version} (${target})...`);

  const binDir = path.join(__dirname, "bin");
  fs.mkdirSync(binDir, { recursive: true });

  const binPath = path.join(binDir, BINARY);
  const data = await download(url);
  fs.writeFileSync(binPath, data);
  fs.chmodSync(binPath, 0o755);

  console.log(`Installed ${BINARY} to ${binPath}`);
}

main().catch((err) => {
  console.error(`Failed to install ${BINARY}: ${err.message}`);
  process.exit(1);
});
