# ⚡ etop (Ecosystem Top)

**Deterministic macOS Developer Ecosystem & Package Top**

[![Rust](https://img.shields.io/badge/rust-2021_edition-orange.svg)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/platform-macOS-lightgrey.svg)](https://apple.com/macos)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#-license)

`etop` is a blazing-fast, deterministic CLI and interactive TUI monitor for macOS developers who work with multiple programming languages, package managers, runtimes, build caches, and container layers.

Inspired by classic Unix monitors (`htop`, `btop`, `ctop`), `etop` audits your entire developer workstation to discover installed software, resolve dependency topologies (unreferenced orphans vs explicit top-level tools), inspect associated filesystem artifacts (configs, data directories, logs, caches, and environment variables), and generate reversible, zero-accident cleanup scripts.

```
┌─ ⚡ etop | Items: 879 | Orphans: 219 (778 MB) | Marked: 0 | Sort: Disk Size ─────────────────────┐
│ Mark  Source         Name                 Category               Size       Status        Last Used│
│ [✓]   🧹 cache:build UV Python Wheels     Build Artifacts        1.47 GB    Cache         Today    │
│ [✓]   🧹 cache:build Homebrew Bottles     Build Artifacts        1.75 GB    Cache         Today    │
│ [ ]   🍺 brew:formula postgresql@16       Databases & Storage    68.7 MB    Dep (7 refs)  Today    │
│ [ ]   ⚡ mise:runtime ruby@4.0.6          Languages & Runtimes   482.1 MB   Active Runtime 4d ago  │
│ [ ]   🦀 cargo:bin   tokio-console        CLI Developer Tools    18.4 MB    Top-Level     12d ago  │
│ [ ]   🍎 macos:app   Ghostty              GUI Apps & Media       42.1 MB    Top-Level     1d ago   │
└────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## ✨ Key Capabilities

- **Immediate Screen Painting ($< 1\text{ ms}$ Startup)**:
  Renders the complete UI frame instantly without blocking terminal startup, asynchronously streaming live discoveries from background workers with smooth 30 FPS Braille loading animations.
- **Polyglot Ecosystem Coverage**:
  Parallel, read-only discovery across **10 toolchains**:
  - 🍺 **Homebrew Formulae & Casks** (`brew info --json=v2`)
  - ⚡ **Mise Runtimes** (`mise ls --json` / active configs)
  - 🦀 **Cargo / Rust Binaries** (`~/.cargo/.crates2.json`)
  - 💎 **Ruby Gems** (audited across all active runtimes)
  - 🌐 **NPM / Bun / Yarn Global Packages**
  - 🐍 **Python / UV / Pip Isolated Tools**
  - 🐹 **Go Binaries** (`$GOPATH/bin` + `go version -m`)
  - 🍎 **macOS Applications** (`/Applications` + `mdls` access timestamps)
  - 🧹 **Compiler Build Caches** (Cargo, Go, UV, Bundler, Homebrew)
  - 🐳 **Docker Dangling Images & Container Layers**
- **Filesystem & Artifact Archaeology**:
  Dynamically locates all associated configuration files, application support state, log files, caches, and active toolchain environment variables for any installed tool.
- **Configuration & Environment Preservation Invariant**:
  User configuration files (`~/.config/...`, `.toml`, `.conf`, `.plist`, `.rc`) and active environment variables are **preserved and never deleted**.
- **Surgical Residual Log Purging**:
  Cleanup plans specifically isolate and purge `.log` files, crash dumps, and caches — including **logs nested inside config directories** (e.g. `~/.config/<name>/*.log` or `logs/`) — without touching configuration state.
- **Post-Removal Package Tombstones & Forensic Archive**:
  Complete package snapshots (paths, configs, data, logs, active environment variables, and reinstall commands) are archived to `~/.local/share/etop/tombstones/` and an append-only `history.jsonl` Write-Ahead Log upon cleanup generation.
- **Zero-Accident Safety Protocol**:
  Never deletes packages silently. Cleanup operations default to `--dry-run` and generate human-auditable bash scripts with inverse rollback/reinstall scripts.

---

## 🚀 Installation

### Option 1: Homebrew (Recommended)
```bash
brew tap vikks/tap
brew install etop
```

### Option 2: One-Line Universal Installer (Pre-compiled Binary)
```bash
curl -fsSL https://raw.githubusercontent.com/vikks/etop/main/install.sh | sh
```

### Option 3: Install via Cargo
```bash
cargo install --git https://github.com/vikks/etop.git
```

### Option 4: Build from Source
```bash
git clone https://github.com/vikks/etop.git
cd etop
cargo build --release
cargo install --path .
```

---

## 🗑️ Uninstallation

### Via Homebrew:
```bash
brew uninstall etop
```

### Via Script:
```bash
curl -fsSL https://raw.githubusercontent.com/vikks/etop/main/uninstall.sh | sh
```
*(To also purge tombstone history data from `~/.local/share/etop/`, pass `--purge`: `curl -fsSL .../uninstall.sh | sh -s -- --purge`)*

---

## 🎮 Interactive TUI Dashboard

Launch the fullscreen immediate-mode TUI:
```bash
etop
# or
etop tui
```

## 💻 CLI Subcommand Reference

### 1. Inspect Package Details & Associated Files (`etop info`)
Inspect multiple versions, install paths, configurations, data directories, log paths, caches, active environment variables, and reinstall commands:
```bash
# Inspect specific package or runtime
etop info postgresql
etop info ruby
etop info ripgrep

# Output structured JSON for automation pipelines
etop info postgresql --json
```

### 2. Full Audited Software Inventory Table (`etop scan`)
```bash
# View complete table sorted by disk footprint
etop scan

# View top 15 largest items
etop scan --top 15

# Sort by inactivity (longest unused first)
etop scan --sort inactivity

# Output raw JSON
etop scan --json
```

### 3. Filter by Objective Criteria (`etop filter`)
```bash
# Filter by ecosystem / toolchain
etop filter --ecosystem ruby
etop filter --ecosystem rust
etop filter --ecosystem brew

# Show only unreferenced orphan dependencies
etop filter --orphans

# Show only build artifact caches and dangling Docker layers
etop filter --caches

# Filter packages inactive for more than 90 days
etop filter --older-than 90

# Combine multi-criteria filters
etop filter -e python --orphans --older-than 60
```

### 4. High-Level Inventory Summary (`etop summary` & `etop categories`)
```bash
# High-level toolchain, orphan, and cache breakdown
etop summary

# Disk usage grouped by categorized domains
etop categories
```

### 5. Historical Tombstone Store (`etop history`)
```bash
# List all past uninstalled packages and preserved configs
etop history

# Inspect full forensic snapshot of an uninstalled package
etop history --inspect postgresql
```

### 6. Generate Deterministic Cleanup & Rollback Scripts (`etop prune`)
```bash
# Generate cleanup plan for orphan dependencies
etop prune --orphans

# Generate cleanup plan for build caches
etop prune --caches

# Output scripts to a specific directory
etop prune --orphans --caches --out ./cleanup_plans
```

---

## 🛡️ Zero-Accident Safety & Forensic Deletion Protocol

When `etop` generates a cleanup plan (e.g. `cleanup_20260819_160920.sh`), it enforces a strict dual-tier safety structure:

1. **Phase 1: Native Package Uninstalls**:
   Executes native package manager commands (`brew uninstall`, `cargo uninstall`, `gem uninstall`, etc.).
2. **Phase 2: Targeted Log & Cache Purge**:
   Deterministically deletes only `.log` files, crash dumps, and caches — specifically including logs nested inside config directories (`~/.config/<name>/*.log` or `logs/`) — **while keeping all `.conf`, `.toml`, `.plist`, and `.rc` user configuration files completely intact**.
3. **Forensic Tombstone Archive**:
   A companion manifest (`cleanup_manifest_*.json`) is saved alongside the scripts, and every removed package's paths, configs, logs, and environment variables are permanently archived to `~/.local/share/etop/tombstones/` and `history.jsonl`.
4. **Reversible Rollback Script**:
   A matching `rollback_reinstall_*.sh` script is generated with inverse install commands and documentation of all preserved configuration paths and environment settings.

---

## 🤝 Contributing Guidelines

We welcome contributions from systems programmers, toolsmiths, and developers across all language ecosystems!

### Development Workflow
1. **Fork & Clone**:
   ```bash
   git clone https://github.com/your-username/etop.git
   cd etop
   ```
2. **Make Changes**:
   Implement your collector, TUI feature, or engine enhancement.
3. **Verify Zero Warnings & Invariants**:
   ```bash
   cargo check
   cargo test
   cargo clippy -- -D warnings
   ```
4. **Submit a Pull Request**.

### Core Architecture Invariants
- **Strict Determinism**: Never rely on external network calls or non-deterministic heuristics at runtime. All inspection must come from deterministic CLI tools and filesystem metadata.
- **Read-Only Scans**: Audits and discovery queries must never mutate the host filesystem.
- **Config & Env Preservation**: Never write deletion routines that destroy `~/.config` files or user environment configurations without explicit, isolated safeguards.

---

## 📜 License

- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
