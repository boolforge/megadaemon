import { isUtf8 } from "node:buffer";
import { closeSync, constants, fstatSync, lstatSync, openSync, readSync, unlinkSync, } from "node:fs";
import { basename, isAbsolute } from "node:path";
export const ENVIRONMENT_FILE_FLAG = "--environment-file";
// Far above any real environment, small enough that a wrong path cannot fill memory
export const MAX_ENVIRONMENT_FILE_BYTES = 256 * 1024;
/**
 * The only file names the daemon reads. The name is checked before the file is
 * opened, so a mistyped path to an SSH key or a credentials file is refused
 * without being read or deleted.
 */
export const ENVIRONMENT_FILE_NAME = /^agent-host-daemon-environment-[A-Za-z0-9_-]+\.json$/;
/** Names outside this are skipped: Bash's exported functions (`BASH_FUNC_x%%`) and the like. */
export const PORTABLE_VARIABLE_NAME = /^[A-Za-z_][A-Za-z0-9_]*$/;
/**
 * Read by Node, libuv or OpenSSL, wholly or in part, only while a process
 * starts. From the file they reach the daemon's children but not the daemon.
 */
export const STARTUP_VARIABLES = new Set([
    "ELECTRON_RUN_AS_NODE",
    "HTTPS_PROXY",
    "HTTP_PROXY",
    "NODE_COMPILE_CACHE",
    "NODE_EXTRA_CA_CERTS",
    "NODE_OPTIONS",
    "NODE_USE_ENV_PROXY",
    "NODE_USE_SYSTEM_CA",
    "NO_PROXY",
    "SSL_CERT_DIR",
    "SSL_CERT_FILE",
    "UV_THREADPOOL_SIZE",
    "http_proxy",
    "https_proxy",
    "no_proxy",
]);
/** Why the daemon refused an environment file. The message never holds a value from the file. */
export class EnvironmentFileRefusedError extends Error {
    constructor() {
        super(...arguments);
        this.name = "EnvironmentFileRefusedError";
    }
}
/**
 * The path after `--environment-file`, or undefined without the flag. Refuses
 * the flag twice or without a path, so a second file can never replace the first.
 */
export function environmentFileFlag(argv) {
    let path;
    for (let i = 0; i < argv.length; i += 1) {
        if (argv[i] !== ENVIRONMENT_FILE_FLAG) {
            continue;
        }
        const value = argv[i + 1];
        if (value === undefined || value.startsWith("--")) {
            throw new EnvironmentFileRefusedError(`${ENVIRONMENT_FILE_FLAG} needs a path`);
        }
        if (path !== undefined) {
            throw new EnvironmentFileRefusedError(`${ENVIRONMENT_FILE_FLAG} was given more than once`);
        }
        path = value;
        i += 1;
    }
    return path;
}
/**
 * Reads the environment file at `path`, deletes it, and returns its variables.
 *
 * The file is a JSON object of variable names to string values. It must be
 * named like {@link ENVIRONMENT_FILE_NAME}, and be a regular file owned by this
 * user with mode 0600 and one link. Any refusal leaves the file where it is,
 * for the launcher to delete: the daemon deletes only a file it has parsed.
 */
export function takeEnvironmentFile(path, options = {}) {
    const platform = options.platform ?? process.platform;
    if (platform === "win32") {
        // Windows has no owner-only file mode; an ACL decides access, and Node cannot read one.
        throw new EnvironmentFileRefusedError(`${ENVIRONMENT_FILE_FLAG} is not supported on Windows: there is no file mode to check, and this check cannot read the file's ACL`);
    }
    if (!isAbsolute(path)) {
        throw new EnvironmentFileRefusedError(`${ENVIRONMENT_FILE_FLAG} must be an absolute path`);
    }
    if (!ENVIRONMENT_FILE_NAME.test(basename(path))) {
        throw refused(path, "is not named agent-host-daemon-environment-<id>.json, so it was not opened");
    }
    const uid = options.uid ?? currentUid();
    const linkStats = lstatOrRefuse(path);
    if (linkStats.isSymbolicLink()) {
        throw refused(path, "is a symlink");
    }
    if (!linkStats.isFile()) {
        throw refused(path, "is not a regular file");
    }
    const fd = openVerifiedEnvironmentFile(path, linkStats, uid);
    try {
        const contents = readAtMost(fd, MAX_ENVIRONMENT_FILE_BYTES + 1);
        if (contents.byteLength > MAX_ENVIRONMENT_FILE_BYTES) {
            throw refused(path, `is larger than ${String(MAX_ENVIRONMENT_FILE_BYTES)} bytes`);
        }
        const parsed = parseEnvironment(contents, path);
        deleteOpenFile(path, fd);
        return parsed;
    }
    finally {
        closeSync(fd);
    }
}
/**
 * Opens the file `linkStats` describes and checks the open descriptor, so a
 * symlink, FIFO or other file swapped in after the `lstat` is refused. Returns
 * the descriptor; the caller closes it.
 */
export function openVerifiedEnvironmentFile(path, linkStats, uid) {
    // O_NONBLOCK keeps a FIFO swapped in after the lstat from blocking the open
    const flags = constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK;
    let fd;
    try {
        fd = openSync(path, flags);
    }
    catch (error) {
        if (errorCode(error) === "ELOOP") {
            throw refused(path, "is a symlink", error);
        }
        if (errorCode(error) === "ENOENT") {
            throw refused(path, "does not exist", error);
        }
        throw refused(path, `could not be opened (${errorCode(error)})`, error);
    }
    let verified = false;
    try {
        const stats = fstatSync(fd);
        if (stats.dev !== linkStats.dev || stats.ino !== linkStats.ino || !stats.isFile()) {
            throw refused(path, "changed while it was being opened");
        }
        if (stats.uid !== uid) {
            throw refused(path, `is owned by uid ${String(stats.uid)}, not this user (${String(uid)})`);
        }
        const mode = stats.mode & 0o7777;
        if (mode !== 0o600) {
            throw refused(path, `has mode ${mode.toString(8).padStart(4, "0")}, not 0600`);
        }
        if (stats.nlink !== 1) {
            throw refused(path, `has ${String(stats.nlink)} links; deleting it would leave a copy`);
        }
        verified = true;
        return fd;
    }
    finally {
        if (!verified) {
            closeSync(fd);
        }
    }
}
/**
 * With `--environment-file` in `argv`, takes that file and makes its variables
 * the whole of `env`, as `spawn`'s `env` option does: every variable the
 * service manager set is removed first. Returns undefined without the flag.
 */
export function applyEnvironmentFile(argv, env, options = {}) {
    const path = environmentFileFlag(argv);
    if (path === undefined) {
        return undefined;
    }
    const { variables, skippedNames } = takeEnvironmentFile(path, options);
    for (const name of Object.keys(env)) {
        delete env[name];
    }
    for (const [name, value] of variables) {
        env[name] = value;
    }
    return {
        set: variables.size,
        skippedNames,
        startupVariables: [...variables.keys()].filter((name) => STARTUP_VARIABLES.has(name)).sort(),
    };
}
function parseEnvironment(contents, path) {
    if (!isUtf8(contents)) {
        throw refused(path, "is not UTF-8");
    }
    let parsed;
    try {
        parsed = JSON.parse(contents.toString("utf8"));
    }
    catch (error) {
        // V8's JSON error quotes the text around the problem, which can be a value
        throw refused(path, "is not valid JSON", { name: errorName(error) });
    }
    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
        throw refused(path, "is not a JSON object of variable names to strings");
    }
    const variables = new Map();
    let skippedNames = 0;
    for (const [name, value] of Object.entries(parsed)) {
        const portable = PORTABLE_VARIABLE_NAME.test(name);
        const which = portable ? name : "a variable";
        if (typeof value !== "string") {
            throw refused(path, `has a value for ${which} that is not a string`);
        }
        if (value.includes("\0")) {
            throw refused(path, `has a value for ${which} with a NUL character`);
        }
        if (!portable) {
            skippedNames += 1;
            continue;
        }
        variables.set(name, value);
    }
    return { variables, skippedNames };
}
/** Reads up to `limit` bytes, so a file that grew after its checks is never read whole. */
function readAtMost(fd, limit) {
    const buffer = Buffer.alloc(limit);
    let length = 0;
    while (length < limit) {
        const read = readSync(fd, buffer, length, limit - length, null);
        if (read === 0) {
            break;
        }
        length += read;
    }
    return buffer.subarray(0, length);
}
/**
 * Deletes the file while its descriptor is still open, then checks that no
 * name is left: a copy renamed away or linked after the checks would survive.
 */
export function deleteOpenFile(path, fd) {
    try {
        unlinkSync(path);
    }
    catch (error) {
        throw refused(path, `could not be deleted after reading (${errorCode(error)})`, error);
    }
    if (fstatSync(fd).nlink !== 0) {
        throw refused(path, "still has another name after it was deleted, so a copy survives; on a network filesystem such as NFS a deleted open file keeps a name, so put the file on a local one");
    }
}
function lstatOrRefuse(path) {
    try {
        return lstatSync(path);
    }
    catch (error) {
        if (errorCode(error) === "ENOENT") {
            throw refused(path, "does not exist", error);
        }
        throw refused(path, `could not be read (${errorCode(error)})`, error);
    }
}
function currentUid() {
    if (process.getuid === undefined) {
        throw new EnvironmentFileRefusedError(`${ENVIRONMENT_FILE_FLAG} needs a platform with user ids to check the file's owner`);
    }
    return process.getuid();
}
function refused(path, reason, cause) {
    return new EnvironmentFileRefusedError(`${ENVIRONMENT_FILE_FLAG} ${path} ${reason}`, cause === undefined ? undefined : { cause });
}
function errorCode(error) {
    if (typeof error === "object" &&
        error !== null &&
        "code" in error &&
        typeof error.code === "string") {
        return error.code;
    }
    return "unknown error";
}
function errorName(error) {
    return error instanceof Error ? error.name : "unknown error";
}
