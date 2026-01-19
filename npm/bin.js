#!/usr/bin/env node
const { spawn } = require("child_process");
const path = require("path");

const binary = path.join(__dirname, "mete" + (process.platform === "win32" ? ".exe" : ""));
const child = spawn(binary, process.argv.slice(2), { stdio: "inherit" });

child.on("close", (code) => process.exit(code));
