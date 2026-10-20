# v1.0.0 release evidence

**Status: released.** [CodeForge Engine v1.0.0](https://github.com/styayur/codeforge-engine/releases/tag/v1.0.0)
was published from tag commit `51073219b6982ad2072dd2b34e90c71a71e2423f`.

## Release governance

- Release PR: [#27](https://github.com/styayur/codeforge-engine/pull/27), squash merged as `5107321`.
- PR and post-merge CI: Windows/Linux, Rust 1.90 MSRV, CodeForge Regression Gate,
  cargo-audit, frontend audit and secret scan all passed.
- Non-publishing candidate build: [run 39284094980](https://github.com/styayur/codeforge-engine/actions/runs/39284094980).
- Publishing tag workflow: [run 39289895575](https://github.com/styayur/codeforge-engine/actions/runs/39289895575).
- The tag workflow validated tag/version metadata, reran format, Clippy and tests,
  rebuilt both platforms, published normalized filenames, generated checksums,
  downloaded the published assets again, and verified their bytes.

## Platform acceptance

### Windows x64

- CLI archive: version output `codeforge 1.0.0`; `scan`, JSON `review`, SARIF
  export, preview-only `apply`, confirmed `apply`, `history` and `undo` passed.
  The workspace was restored byte-for-byte to its pre-apply source.
- MSI: installed per-user with `ALLUSERS=2 MSIINSTALLPERUSER=1`, started with
  window title `CodeForge Engine`, and uninstalled without registry or directory
  residue. A default per-machine silent install requires administrator rights.
- NSIS: silent install, GUI startup, and silent uninstall passed without registry
  or directory residue.
- Host: Windows 11 build 26200, x64.

### Linux x64

- CLI archive: version output, review, apply and undo passed; source restoration
  passed.
- Debian package: installed as `code-forge-engine 1.0.0`, desktop process started
  under WSLg, and removal left no binary or installed package.
- AppImage: extracted successfully and desktop process started under WSLg.
- Host: WSL2 Ubuntu x86_64 with WSLg. Mesa emitted software-rendering warnings,
  but both desktop processes remained alive during the startup window.
- No native physical Linux machine was available; this is recorded rather than
  described as bare-metal acceptance.

## Published assets

The final release assets differ in bytes from the pre-tag candidate rebuild; the
published copies were downloaded and functionally tested after publication.

```text
64ea02dcabae08ea7259a7317b326e3be90166d7fbbf5a04c94a2abdb511a30c  CodeForge.Engine_1.0.0_amd64.AppImage
b46282dd7168528351e1028442ad7e18550c014b2b6c88bb1b90f761df1858d9  CodeForge.Engine_1.0.0_amd64.deb
8505e6f1b2a5821c56e0461372f82a07db3beaf499175dbfdcd436696e0f3c0c  CodeForge.Engine_1.0.0_x64-setup.exe
0ea9a369ba85fad6cfe936aca3f8f2fe8116d21cf201d4f66da85690a613d99f  CodeForge.Engine_1.0.0_x64_en-US.msi
9629a6e60d2b889dae9b528548c90b91f993c2b120a753631dd556bedd59a7b0  codeforge-cli-v1.0.0-x86_64-pc-windows-msvc.zip
5b50c57add5e9d5d1fc40091d34a8ff6cc01b1e9805a4a2b742011938d38e41c  codeforge-cli-v1.0.0-x86_64-unknown-linux-gnu.tar.gz
```

## Surviving boundaries

Context extraction, heuristic impact/test suggestions, local SARIF path import and
optional CodeQL execution remain Experimental. Fake-process tests do not certify a
real CodeQL installation. Name-based relations do not provide complete scope/type
resolution. Missing required tools remain execution gaps, not passing evidence.
