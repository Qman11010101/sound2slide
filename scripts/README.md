# Build scripts

Run from the project root:

```powershell
./scripts/build.ps1
```

The build creates `dist/sound2slide.dll`, `dist/sound2slide.ini`, and
`dist/third-party-library.txt`. Distribute the license notices with the plugin.
License generation failures stop the build before the DLL is copied to `dist`.

`cargo-about` is installed automatically if unavailable, using version 0.9.2
with its `cli` feature. Installation and license gathering can require network
access. Dependency versions are read from `Cargo.lock` without updating it.

To regenerate only the notices:

```powershell
./scripts/generate-licenses.ps1
```

`about.toml` specifies the Windows target and accepted license identifiers.
The output includes transitive Rust dependencies, build dependencies, vendored
C++ licenses, bundled font notices, and versioned crate source download links.

To package exactly the DLL, INI, and notices into the release ZIP:

```powershell
./scripts/package-release.ps1
```

The release profile uses `opt-level = "s"`, fat LTO, one code generation unit,
and symbol stripping. Static MSVC CRT linking is enabled. Panic unwinding stays
enabled because the plugin catches panics at the host ABI boundary and restores
host windows after errors.

To test with the release profile used by Windows CI:

```powershell
./scripts/test.ps1 -Release
```
