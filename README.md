# Roto Pong 🎮

A **circular arena arcade game** (Pong/Breakout-inspired) built with Rust, WebAssembly, and WebGPU.

![Status](https://img.shields.io/badge/status-in%20development-yellow)

## Features

- 🔄 **360° rotating paddle** around a circular arena
- 🕳️ **Central black hole hazard** - don't let the ball fall in!
- 🎨 **WebGPU-powered graphics** with premium shaders and effects
- 📱 **Mobile-friendly** with touch controls
- 💾 **Run persistence** - continue your game after closing the tab
- 🎯 **Deterministic simulation** for fair, reproducible gameplay

## Development

### Prerequisites

- [Rust](https://rustup.rs/) (latest stable)
- [trunk](https://trunkrs.dev/) for WASM builds: `cargo install trunk`

### Run Locally

```bash
# Development server with hot reload
trunk serve

# Open http://127.0.0.1:8080 in a WebGPU-capable browser
```

### Build for Release

```bash
trunk build --release
# Output in dist/
```

### Run Tests

```bash
cargo test
```

## Architecture

```
src/
├── sim/           # Deterministic simulation (physics, collisions, state)
│   ├── arc.rs     # Curved arc segment geometry
│   ├── collision.rs # Ball-arc collision detection
│   └── state.rs   # Game state (balls, blocks, paddle, etc.)
├── renderer/      # WebGPU rendering pipeline
├── platform/      # Browser/native abstraction
├── persistence/   # Save/load with integrity verification
├── tuning/        # Data-driven game balance
└── ui/            # Menu/HUD (DOM overlay)
```

## Browser Support

**WebGPU required** (no fallback renderer):

- ✅ Chrome/Edge (latest)
- ✅ Firefox (with WebGPU enabled)
- ✅ Safari (macOS, where WebGPU is available)
- ✅ iOS Safari / Android Chrome (WebGPU-capable devices)

## License

MIT
