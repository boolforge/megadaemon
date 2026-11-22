# Go 1.23.2 fixture matrix: observed results

All four fixtures were cross-built with the same installed Go 1.23.2 toolchain (`-trimpath`, `CGO_ENABLED=0` in `scripts/build-fixtures.sh`) and passed the independent Python structural sanity checker. These are not Rust parser test results.

| Target | nfunc | Names resolved | minLC | Structural errors |
|---|---:|---:|---:|---:|
| Linux/amd64 | 1,889 | 1,889 | 1 | 0 |
| Linux/arm64 | 1,887 | 1,887 | 4 | 0 |
| Windows/amd64 | 1,939 | 1,939 | 1 | 0 |
| Darwin/arm64 | 1,946 | 1,946 | 4 | 0 |

Detailed reports are the adjacent `fixture-go1.23.2-*.json` files. `validation/go1.23.2-linux-amd64.json` is the separate baseline executable built with the host's default `CGO_ENABLED` setting; it reported 1,889/1,889 names.

The checker verifies only the modern `pcHeader`/`functab`/`_func.nameOff` structure and string boundaries. It does not verify all `_func` arrays, `pctab`, `moduledata`, types, `.go.type`, itabs, runtime execution on foreign OS/architectures, or Rust code compilation.
