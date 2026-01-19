#!/usr/bin/env node
const https = require("https");
const fs = require("fs");
const path = require("path");

const REPO = "dutch-casa/mete.rs";
const VERSION = require("./package.json").version;

function getPlatformBinary() {
  const platform = process.platform;
  const arch = process.arch;

  if (platform === "darwin" && arch === "arm64") return "mete-darwin-arm64";
  if (platform === "darwin" && arch === "x64") return "mete-darwin-x64";
  if (platform === "linux" && arch === "x64") return "mete-linux-x64";
  if (platform === "win32" && arch === "x64") return "mete-windows-x64.exe";

  throw new Error(`Unsupported platform: ${platform}-${arch}`);
}

function download(url, dest) {
  return new Promise((resolve, reject) => {
    const file = fs.createWriteStream(dest);
    https
      .get(url, (response) => {
        if (response.statusCode === 302 || response.statusCode === 301) {
          download(response.headers.location, dest).then(resolve).catch(reject);
          return;
        }
        if (response.statusCode !== 200) {
          reject(new Error(`Failed to download: ${response.statusCode}`));
          return;
        }
        response.pipe(file);
        file.on("finish", () => {
          file.close();
          resolve();
        });
      })
      .on("error", reject);
  });
}

async function main() {
  const binaryName = getPlatformBinary();
  const url = `https://github.com/${REPO}/releases/download/v${VERSION}/${binaryName}`;
  const dest = path.join(__dirname, "mete" + (process.platform === "win32" ? ".exe" : ""));

  console.log(`Downloading mete v${VERSION} for ${process.platform}-${process.arch}...`);

  try {
    await download(url, dest);
    if (process.platform !== "win32") {
      fs.chmodSync(dest, 0o755);
    }
    console.log("mete installed successfully!");
  } catch (err) {
    console.error(`Failed to download mete: ${err.message}`);
    console.error(`URL: ${url}`);
    console.error("\nYou can install from source with: cargo install mete --features mcp");
    process.exit(1);
  }
}

main();
