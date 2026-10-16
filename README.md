# E220 Controller

Desktop controller for the EBYTE E220 LoRa module, built with [GPUI Kit](https://github.com/longbridge/gpui-kit).
It started as a rewrite of [0-terarin-0/app-e220-controller](https://github.com/0-terarin-0/app-e220-controller),
replacing the Next.js / Tauri frontend with a native window. Serial I/O uses the `serialport` crate directly.

## What it does

- Connect to a serial port, with optional auto-reconnect
- Normal mode: send and receive text or hex, with optional CRLF
- Settings mode: read and write E220 registers (address, channel, UART, air rate, power, WOR, crypto)
- WOR receive and WOR send pages for the matching M0 / M1 wiring
- Japanese and English UI (`locales/*.toml`)
- Automatic updates from GitHub Releases when installed with the Velopack installer

Settings mode still requires M0 and M1 HIGH. In that mode the module's UART is fixed at 9600 bps.

## Install

Download the installer for your OS from [Releases](https://github.com/wsp-space/e220-controller/releases/latest):

| OS      | File                              |
| ------- | --------------------------------- |
| Windows | `E220Controller-win-Setup.exe`    |
| macOS   | `E220Controller-osx-Setup.pkg`    |
| Linux   | `E220Controller.AppImage`         |

Installed copies check for a newer release on startup, download it in the background,
and apply it the next time the app starts.

## Develop

```bash
cargo run
cargo test
```

## Release

Releases are built by [`.github/workflows/release.yml`](.github/workflows/release.yml)
and packaged with [Velopack](https://velopack.io).

1. Bump `version` in `Cargo.toml` and merge to `main`.
2. Tag the commit on `main` and push the tag:

   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```

The workflow refuses tags that do not match `Cargo.toml` or that point outside `main`.
Running the workflow manually builds and packs all three platforms without publishing.

## License

[MIT](LICENSE)
