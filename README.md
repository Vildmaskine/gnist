# Gnist

A minimal, modern serial terminal built with Tauri 2, Svelte, and Rust.

![Screenshot placeholder](docs/screenshot.png)

## Features

- Auto-detects serial ports (`ttyUSB*`, `ttyACM*`, `ttyAMA*`)
- Configurable baud rate: 9600 / 19200 / 38400 / 57600 / 115200
- Full xterm.js terminal with scrollback and keyboard input
- Ports refreshed automatically every 2 seconds
- Disconnects gracefully if a device is unplugged
- Ships as a Flatpak

## Requirements

- [Node.js](https://nodejs.org/) 18+
- [Rust](https://rustup.rs/) 1.70+
- Linux: `libudev-dev` (`sudo apt install libudev-dev`)
- Linux: WebKitGTK 4.1 (`sudo apt install libwebkit2gtk-4.1-dev`)

## Development

```bash
npm install
npm run tauri dev
```

## Build

```bash
npm run tauri build
```

Produces a native binary and bundles (`.deb`, `.rpm`, `.AppImage`) in `src-tauri/target/release/bundle/`.

## Flatpak

Requires `org.gnome.Platform//48` and `flatpak-builder`.

```bash
cd src-tauri
flatpak-builder --force-clean --repo=repo build-dir gnist.flatpak.yml
flatpak build-bundle repo gnist.flatpak com.gnist.app
flatpak install --user gnist.flatpak
flatpak run com.gnist.app
```

## License

MIT
