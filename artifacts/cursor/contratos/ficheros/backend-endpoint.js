import { checkCursorCredentialOrigin, checkCursorSignInOrigin, } from "@anysphere/cursor-config/auth";
export const BACKEND_URL_FLAG = "--backend-url";
/** The variable every part of the daemon reads its backend from. */
export const BACKEND_ENDPOINT_VARIABLE = "CURSOR_API_ENDPOINT";
/** The backend a daemon with none named talks to. */
const DEFAULT_BACKEND_ENDPOINT = "https://api2.cursor.sh";
/** Why the daemon refused its backend. The message names at most a host, never a whole URL. */
export class BackendEndpointRefusedError extends Error {
    constructor() {
        super(...arguments);
        this.name = "BackendEndpointRefusedError";
    }
}
/**
 * The URL after `--backend-url`, or undefined without the flag. Refuses the
 * flag twice or without a URL, so a second backend can never replace the first.
 */
export function backendUrlFlag(argv) {
    let url;
    for (let i = 0; i < argv.length; i += 1) {
        if (argv[i] !== BACKEND_URL_FLAG) {
            continue;
        }
        const value = argv[i + 1];
        if (value === undefined || value.startsWith("--") || value.trim().length === 0) {
            throw new BackendEndpointRefusedError(`${BACKEND_URL_FLAG} needs a URL`);
        }
        if (url !== undefined) {
            throw new BackendEndpointRefusedError(`${BACKEND_URL_FLAG} was given more than once`);
        }
        url = value;
        i += 1;
    }
    return url;
}
/** The host of a URL that passed the origin check's parse, for a message. */
function hostOf(url) {
    return new URL(url).host;
}
function refusal(named, url, reason) {
    switch (reason) {
        case "unparseable":
            return new BackendEndpointRefusedError(`${named} is not an http or https URL`);
        case "embedded_credentials":
            return new BackendEndpointRefusedError(`${named} carries a user name or a password`);
        case "insecure_scheme":
            return new BackendEndpointRefusedError(`${named} names the Cursor API host ${hostOf(url)} without https`);
        case "not_cursor_origin":
            return new BackendEndpointRefusedError(`${named} names ${hostOf(url)}, which is not one of Cursor's API hosts; a packaged daemon takes any other backend only from ${BACKEND_URL_FLAG} on its command line`);
        default: {
            const unhandled = reason;
            return new BackendEndpointRefusedError(`${named} was refused: ${String(unhandled)}`);
        }
    }
}
/**
 * Settles the backend the daemon talks to, before anything reads it.
 *
 * `--backend-url` in `argv` is the launcher's choice: it replaces the
 * variable in `env`, so every reader sees it. Without the flag, a packaged
 * daemon takes the variable only when it names an origin Cursor's shared
 * credential rule allows with nothing configured, one of Cursor's API hosts
 * over https, and refuses any other value: the variable can come from a shell
 * profile or a service manager rather than from the launcher. A source run
 * takes the variable as it is. Either way a blank variable is removed and a
 * padded one is trimmed, so every reader sees the backend this settled on.
 */
export function applyBackendEndpoint(argv, env, options) {
    const flag = backendUrlFlag(argv)?.trim();
    if (flag !== undefined) {
        // Configured with itself, the flag passes on any host and either scheme.
        const verdict = checkCursorCredentialOrigin(flag, [flag]);
        if (!verdict.allowed) {
            throw refusal(BACKEND_URL_FLAG, flag, verdict.reason);
        }
        env[BACKEND_ENDPOINT_VARIABLE] = flag;
        return { source: "flag", endpoint: flag };
    }
    const fromEnvironment = (env[BACKEND_ENDPOINT_VARIABLE] ?? "").trim();
    if (fromEnvironment.length === 0) {
        // Some readers take any non-empty value as the backend, so a blank one is removed.
        delete env[BACKEND_ENDPOINT_VARIABLE];
        return { source: "default", endpoint: DEFAULT_BACKEND_ENDPOINT };
    }
    if (options.built) {
        const verdict = checkCursorCredentialOrigin(fromEnvironment);
        if (!verdict.allowed) {
            throw refusal(BACKEND_ENDPOINT_VARIABLE, fromEnvironment, verdict.reason);
        }
    }
    // Readers see the value that was checked, without the padding around it.
    env[BACKEND_ENDPOINT_VARIABLE] = fromEnvironment;
    return { source: "environment", endpoint: fromEnvironment };
}
function checkAuthUrlFromEnvironment(variable, url, endpoint) {
    switch (variable) {
        case "CURSOR_API_BASE_URL":
            return checkCursorCredentialOrigin(url, [endpoint]);
        case "CURSOR_WEBSITE_URL":
            return checkCursorSignInOrigin(url, [endpoint]);
        default: {
            const unhandled = variable;
            throw new Error(`Unhandled auth URL variable: ${String(unhandled)}`);
        }
    }
}
/**
 * The filter a packaged daemon on `endpoint` sets for the auth URLs the
 * shared auth code reads from the environment. The API URL takes an API key,
 * so it is held to the shared credential rule, as the backend variable is.
 * The website URL is where the user signs in, so it is held to the shared
 * sign-in rule. Either may also name the daemon's own backend. A source run
 * sets no filter.
 */
export function authUrlEnvFilterFor(endpoint, options) {
    if (!options.built) {
        return undefined;
    }
    return (variable, url) => checkAuthUrlFromEnvironment(variable, url, endpoint).allowed;
}
