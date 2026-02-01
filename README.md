# powertui

A simple TUI for managing Linux CPU power profiles, built with [Ratatui](https://ratatui.rs).

![powertui screenshot](https://github.com/chbornman/powertui/assets/screenshot.png)

## Features

- View battery status, capacity, and health
- Switch between power profiles (Power Saver, Balanced, Performance)
- Vim-style navigation

## Installation

```bash
cargo install --path .
```

Or build from source:

```bash
git clone https://github.com/chbornman/powertui
cd powertui
cargo build --release
cp target/release/powertui ~/.local/bin/
```

## Usage

```bash
powertui
```

### Controls

| Key | Action |
|-----|--------|
| `j` / `Down` | Move down |
| `k` / `Up` | Move up |
| `Enter` / `Space` | Select profile |
| `r` | Refresh |
| `q` / `Esc` | Quit |

## Requirements

- Linux with `/sys/class/power_supply/` (for battery info)
- `cpupower` installed
- Passwordless sudo for `cpupower` (add to sudoers):

```
username ALL=(ALL) NOPASSWD: /usr/bin/cpupower
```

## License

MIT
