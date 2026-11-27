import { type Stats } from "node:fs";
export declare const ENVIRONMENT_FILE_FLAG: string;
export declare const MAX_ENVIRONMENT_FILE_BYTES: number;
/**
 * The only file names the daemon reads. The name is checked before the file is
 * opened, so a mistyped path to an SSH key or a credentials file is refused
 * without being read or deleted.
 */
export declare const ENVIRONMENT_FILE_NAME: RegExp;
/** Names outside this are skipped: Bash's exported functions (`BASH_FUNC_x%%`) and the like. */
export declare const PORTABLE_VARIABLE_NAME: RegExp;
/**
 * Read by Node, libuv or OpenSSL, wholly or in part, only while a process
 * starts. From the file they reach the daemon's children but not the daemon.
 */
export declare const STARTUP_VARIABLES: ReadonlySet<string>;
/** Why the daemon refused an environment file. The message never holds a value from the file. */
export declare class EnvironmentFileRefusedError extends Error {
    readonly name: string;
}
export interface TakeEnvironmentFileOptions {
    readonly platform?: NodeJS.Platform;
    /** The user the file must belong to; defaults to this process's user. */
    readonly uid?: number;
}
export interface EnvironmentFileContents {
    readonly variables: ReadonlyMap<string, string>;
    /** How many names outside {@link PORTABLE_VARIABLE_NAME} the file held. */
    readonly skippedNames: number;
}
export interface AppliedEnvironmentFile {
    readonly set: number;
    readonly skippedNames: number;
    /** The file's {@link STARTUP_VARIABLES}, which do not change the daemon itself. */
    readonly startupVariables: readonly string[];
}
/**
 * The path after `--environment-file`, or undefined without the flag. Refuses
 * the flag twice or without a path, so a second file can never replace the first.
 */
export declare function environmentFileFlag(argv: readonly string[]): string | undefined;
/**
 * Reads the environment file at `path`, deletes it, and returns its variables.
 *
 * The file is a JSON object of variable names to string values. It must be
 * named like {@link ENVIRONMENT_FILE_NAME}, and be a regular file owned by this
 * user with mode 0600 and one link. Any refusal leaves the file where it is,
 * for the launcher to delete: the daemon deletes only a file it has parsed.
 */
export declare function takeEnvironmentFile(path: string, options?: TakeEnvironmentFileOptions): EnvironmentFileContents;
/**
 * Opens the file `linkStats` describes and checks the open descriptor, so a
 * symlink, FIFO or other file swapped in after the `lstat` is refused. Returns
 * the descriptor; the caller closes it.
 */
export declare function openVerifiedEnvironmentFile(path: string, linkStats: Stats, uid: number): number;
/**
 * With `--environment-file` in `argv`, takes that file and makes its variables
 * the whole of `env`, as `spawn`'s `env` option does: every variable the
 * service manager set is removed first. Returns undefined without the flag.
 */
export declare function applyEnvironmentFile(argv: readonly string[], env: Record<string, string | undefined>, options?: TakeEnvironmentFileOptions): AppliedEnvironmentFile | undefined;
/**
 * Deletes the file while its descriptor is still open, then checks that no
 * name is left: a copy renamed away or linked after the checks would survive.
 */
export declare function deleteOpenFile(path: string, fd: number): void;
