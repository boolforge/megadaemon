# Validation record

Environment: Go 1.23.2 linux/amd64. Rust/Cargo were not installed.

## Executed runtime fixtures

Baseline executable:

```text
hello Fido 2 6 *main.Dog {4 6}
```

Plugin host loading the separate Go plugin:

```text
fixture-plugin
```

## Build metadata

`go version -m artifacts/fixture-go1.23.2-linux-amd64` identifies `go1.23.2`, module `example.com/go-retool-fixture`, `GOARCH=amd64`, `GOOS=linux`, `-trimpath`, and buildmode exe.

## Structural checks

`matrix-summary.md` records the cross-target matrix. The independent Python checker found:

- Linux/amd64: 1,889 functions; 1,889 names resolved; 0 structural errors.
- Linux/arm64: 1,887 functions; 1,887 names resolved; 0 structural errors.
- Windows/amd64: 1,939 functions; 1,939 names resolved; 0 structural errors.
- Darwin/arm64: 1,946 functions; 1,946 names resolved; 0 structural errors.

These are Python checker results, not Rust parser results. Rust/Cargo were absent, so no claim of `cargo test` success is made.
