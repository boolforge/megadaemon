import { type AuthUrlEnvFilter } from "@anysphere/cursor-config/auth";
export declare const BACKEND_URL_FLAG: string;
/** The variable every part of the daemon reads its backend from. */
export declare const BACKEND_ENDPOINT_VARIABLE: string;
/** Why the daemon refused its backend. The message names at most a host, never a whole URL. */
export declare class BackendEndpointRefusedError extends Error {
    readonly name: string;
}
export interface ApplyBackendEndpointOptions {
    /** Whether this process runs from the packaged daemon bundle. */
    readonly built: boolean;
}
export interface AppliedBackendEndpoint {
    /**
     * Where the backend came from: the launcher's flag, the environment's
     * variable, or neither, which leaves the daemon on its default.
     */
    readonly source: "flag" | "environment" | "default";
    /** The backend the daemon talks to. */
    readonly endpoint: string;
}
/**
 * The URL after `--backend-url`, or undefined without the flag. Refuses the
 * flag twice or without a URL, so a second backend can never replace the first.
 */
export declare function backendUrlFlag(argv: readonly string[]): string | undefined;
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
export declare function applyBackendEndpoint(argv: readonly string[], env: Record<string, string | undefined>, options: ApplyBackendEndpointOptions): AppliedBackendEndpoint;
/**
 * The filter a packaged daemon on `endpoint` sets for the auth URLs the
 * shared auth code reads from the environment. The API URL takes an API key,
 * so it is held to the shared credential rule, as the backend variable is.
 * The website URL is where the user signs in, so it is held to the shared
 * sign-in rule. Either may also name the daemon's own backend. A source run
 * sets no filter.
 */
export declare function authUrlEnvFilterFor(endpoint: string, options: ApplyBackendEndpointOptions): AuthUrlEnvFilter | undefined;
