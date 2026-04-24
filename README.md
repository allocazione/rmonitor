# rmonitor

rmonitor is a high-performance, cross-platform System, Docker, and Security Monitor running entirely in your terminal (TUI). Built in Rust with `ratatui` and `tokio`, it provides a 60 FPS real-time dashboard of your machine's critical metrics, Docker containers, and active remote access sessions.

## Features

- **System Metrics:** Real-time per-core CPU usage gauges, memory history sparkline, and overall disk utilization.
- **Network Stats:** Live tracking of inbound (RX) and outbound (TX) traffic rates across all interfaces.
- **Docker Monitoring:** Live container table showing name, image, status, CPU%, memory usage, and per-container network I/O. Automatically detects Docker availability.
- **Cross-Platform Security Monitoring:**
  - **Linux/OpenBSD/FreeBSD:** Non-blocking tailing and regex-parsing of `/var/log/auth.log`, `/var/log/secure`, or `/var/log/authlog` to capture SSH login/logoff events.
  - **Windows:** Subscribes to the Windows Security Event Log for Event IDs 4624 (Logon) and 4634 (Logoff), capturing RDP and Network logins.
  - **WSL:** Automatically detected and indicated with a `[WSL]` badge in the header.
- **Smart Analytics:** Automatically looks up remote login IPs against a GeoIP service (with an LRU cache) to map connections to physical locations.
- **In-App Settings:** Edit all configuration (colors, refresh rates, URLs, paths) directly in the TUI. Press `Ctrl+S` to save to disk.
- **Floating Alerts:** Important login events and settings saves spawn dynamic floating toast notifications.

---

## Prerequisites

Before building `rmonitor`, ensure you have the following dependencies installed:

### All Platforms
- [Rust & Cargo](https://www.rust-lang.org/tools/install) (1.75+)

### Linux (Ubuntu/Debian)
```bash
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev clang lld
```

### Linux (Fedora/RHEL)
```bash
sudo dnf groupinstall "Development Tools"
sudo dnf install pkg-config openssl-devel clang lld
```

### macOS
```bash
brew install pkg-config openssl llvm
```

## Installation & Setup

Ensure you have [Rust](https://www.rust-lang.org/tools/install) installed.

### 1. Build the application

```bash
cargo build --release
```

Or use the provided build scripts which output to `release/<os>/`:

```bash
# Windows (PowerShell)
powershell -ExecutionPolicy Bypass -File .\build.ps1

# Linux / macOS / OpenBSD
./build.sh
```

### 2. Run the application

#### On Linux:

```bash
sudo ./target/release/rmonitor
```

#### On Windows:

Run from an **Administrator** terminal for full Security Event Log access:

```powershell
.\target\release\rmonitor.exe
```

#### Docker Monitoring

Docker monitoring works automatically — just have Docker Desktop or the Docker daemon running. If Docker is not available, the Docker tab will display a friendly message and keep retrying.

---

## Keyboard Controls

| Key | Action |
|---|---|
| `Tab` | Cycle through tabs (Dashboard → Docker → Settings) |
| `1`, `2`, `3` | Jump directly to Dashboard, Docker, or Settings |
| `↑` / `↓` | Navigate lists (Docker containers, Settings fields) |
| `PgUp` / `PgDn` | Scroll settings by 10 fields |
| `Enter` | Edit selected settings field |
| `Esc` | Cancel edit / Quit |
| `Ctrl+S` | Save settings to disk (in Settings tab) |
| `q` | Quit |

---

## Configuration

rmonitor loads settings from:

- **Linux:** `~/.config/rmonitor/config.toml`
- **Windows:** `%APPDATA%\rmonitor\config.toml`

You can edit all settings **live** from the Settings tab (press `2` or `Tab`). Changes are applied immediately and can be saved to disk with `Ctrl+S`.

### Example `config.toml`

```toml
[general]
ui_fps = 60
refresh_rate_ms = 1000
alert_duration_secs = 5

[network]
geoip_url_template = "http://ip-api.com/json/{ip}?fields=status,country,city"
public_ip_url = "https://api.ipify.org"

[colors]
header_bg = "#1a1b26"
header_fg = "#c0caf5"
gauge_low = "#9ece6a"
gauge_high = "#f7768e"
border = "#565f89"
```

---

## Architecture

rmonitor uses an MVC pattern optimized for the terminal:

1. **Model (`AppState`):** A shared `Arc<RwLock<AppState>>` holding all system data.
2. **Providers (Background Tasks):** Independent `tokio` tasks polling CPU, memory, disk, network, Docker, and security logs. They acquire short write locks to push data.
3. **View (`ratatui` UI):** A 60 FPS render loop using non-blocking read snapshots — the UI never stalls.

## Graceful Error Handling

- **Panic hooks** restore the terminal even if the app crashes.
- **Mutex poisoning** is recovered from automatically.
- **Docker unavailable** — shows a friendly message, keeps retrying.
- **Insufficient permissions** — displays a warning banner instead of crashing.
- **Terminal too small** — shows a resize message instead of panicking.
