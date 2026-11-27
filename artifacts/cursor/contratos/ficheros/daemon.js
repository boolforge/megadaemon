#!/usr/bin/env node
import "./stdio-errors-at-start.js";
import "./environment-file-at-start.js";
import "./backend-endpoint-at-start.js";
import "./orbit-build-guard.js";
import "./data-sink-at-start.js";
import "./client-machine-id-at-start.js";
import { runDaemonProcess } from "../process/run-process.js";
// The spawner sets NODE_COMPILE_CACHE so Node can compile this main module
// from cache. Node reads that variable at startup, before this file runs.
// Keep that folder for the sandbox policy (shell-exec's node-compile-cache.ts
// keeps it read-only), remove the variable before the process starts children
// so they do not inherit it, and turn the cache off for them.
const startupCompileCache = process.env.NODE_COMPILE_CACHE;
const startupCompileCacheKey = Symbol.for("cursor.startupNodeCompileCache");
if (startupCompileCache !== undefined && !Object.hasOwn(globalThis, startupCompileCacheKey)) {
    Object.defineProperty(globalThis, startupCompileCacheKey, {
        value: startupCompileCache,
        configurable: true,
        writable: true,
    });
}
delete process.env.NODE_COMPILE_CACHE;
process.env.NODE_DISABLE_COMPILE_CACHE = "1";
const enteredAtMs = performance.now();
process.stderr.write(`agent-host-daemon entered at ${enteredAtMs.toFixed(1)}ms\n`);
void runDaemonProcess(process.argv.slice(2)).then((exitCode) => {
    if (exitCode !== 0) {
        process.exit(exitCode);
    }
});
