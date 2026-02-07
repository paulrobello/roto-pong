# Roto Pong 🎮

[![Play Now](https://img.shields.io/badge/Play%20Now-roto--pong.pardev.net-brightgreen)](https://roto-pong.pardev.net)
![WebGPU](https://img.shields.io/badge/WebGPU-required-blue)
![Rust](https://img.shields.io/badge/built%20with-Rust%20%2B%20WASM-orange)
![License](https://img.shields.io/badge/license-MIT-green)

A **circular arena arcade game** (Pong/Breakout-inspired) built with Rust, WebAssembly, and WebGPU. Defend the black hole with your orbiting paddle!

![Roto Pong Menu](https://raw.githubusercontent.com/paulrobello/roto-pong/main/screenshot-menu.png)

![Roto Pong Gameplay](https://raw.githubusercontent.com/paulrobello/roto-pong/main/screenshot.png)

[!["Buy Me A Coffee"](https://www.buymeacoffee.com/assets/img/custom_images/orange_img.png)](https://buymeacoffee.com/probello3)

## 🎮 Play Now

**[https://roto-pong.pardev.net](https://roto-pong.pardev.net)**

No installation required - runs in any WebGPU-capable browser!

## ✨ Features

### Gameplay
- 🔄 **360° Rotating Paddle** - Orbit around the arena to defend the black hole
- 🕳️ **Black Hole Hazard** - Central gravity well pulls the ball in
- 💎 **10 Block Types** - Glass, Armored, Explosive, Jello, Crystal, Electric, Magnet, Ghost, Portal, Invincible
- 💊 **5 Power-ups** - MultiBall, Slow Motion, Piercing, Widen Paddle, Shield
- 🏆 **Endless Waves** - Progressive difficulty with variety
- 🎯 **Combo System** - Chain hits for score multipliers

### Visuals
- 🎨 **WebGPU-Powered** - Premium SDF shaders and effects
- 🌌 **M87-Style Black Hole** - Swirling accretion disk
- ⚡ **Electric Arcs** - Lightning between electric blocks
- ✨ **Particle Effects** - Block breaks, celebrations, sparks
- 🌀 **Screen Shake & Flash** - Satisfying feedback

### Audio
- 🔊 **Procedural Sound Effects** - 16 unique sounds, no external files
- 🎛️ **Volume Controls** - Master and SFX sliders
- 🔇 **Mute on Blur** - Auto-mute when tab loses focus
- ⌨️ **Quick Toggle** - Press `M` to mute/unmute

### Quality of Life
- 💾 **Auto-Save** - Continue your run after closing the tab
- ⏸️ **Pause Menu** - Press `Escape` anytime
- 📊 **High Score Leaderboard** - Track your best runs
- ⚙️ **Settings** - Quality presets, visual effects toggles
- 📱 **Mobile Support** - Touch controls

## 🎹 Controls

| Input | Action |
|-------|--------|
| **Mouse** | Move paddle (pointer lock) |
| **Touch** | Drag anywhere to move paddle |
| **Click / Tap / Space / Enter** | Launch ball |
| **Escape** | Pause / Resume |
| **M** | Toggle sound on/off |

## 🖥️ Browser Support

**WebGPU required** (no fallback renderer):

| Browser | Status |
|---------|--------|
| Chrome / Edge | ✅ Supported |
| Firefox | ✅ With WebGPU flag enabled |
| Safari | ✅ macOS Sonoma+ |
| Mobile Chrome | ✅ Android with WebGPU |
| Mobile Safari | ✅ iOS 17+ |

## 🛠️ Development

### Prerequisites

- [Rust](https://rustup.rs/) (latest stable)
- [trunk](https://trunkrs.dev/): `cargo install trunk`
- `wasm32-unknown-unknown` target: `rustup target add wasm32-unknown-unknown`

### Run Locally

```bash
# Clone the repo
git clone https://github.com/paulrobello/roto-pong.git
cd roto-pong

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

## 📁 Architecture

```
src/
├── audio.rs       # Procedural Web Audio sound effects
├── sim/           # Deterministic simulation (physics, collisions, state)
│   ├── arc.rs     # Curved arc segment geometry
│   ├── collision.rs # Ball-arc collision detection
│   ├── state.rs   # Game state (balls, blocks, paddle, etc.)
│   └── tick.rs    # Fixed timestep game loop
├── renderer/      # WebGPU rendering pipeline
│   ├── sdf_shader.wgsl # SDF-based fragment shader
│   └── sdf_pipeline.rs # Render state and uniforms
├── persistence/   # LocalStorage save/load with integrity verification
├── settings.rs    # User preferences (quality, audio, effects)
├── highscores.rs  # Local leaderboard
└── ui/            # Menu/HUD (DOM overlay)
```

## 🎯 Design Principles

- **Deterministic Simulation** - Fixed 120Hz timestep, seeded RNG for reproducibility
- **Fair & Comfortable** - Auto-pause on blur, no cheap deaths
- **Mobile-First Touch** - Drag anywhere to control, large tap targets
- **Performance Presets** - Low/Medium/High quality for all devices
- **No External Assets** - Procedural audio, SDF rendering

## 📝 License

MIT License - see [LICENSE](LICENSE) for details.

## 🙏 Acknowledgments

- Inspired by classic Breakout and circular arena games
- Built with [wgpu](https://wgpu.rs/) for WebGPU rendering
- Bundled with [Trunk](https://trunkrs.dev/) for WASM deployment

---

Made with ❤️ and 🦀 by [Paul Robello](https://github.com/paulrobello)
