# PRD: **Roto Pong** (Rust/WASM + WebGPU 2D Arcade Game)

## 1) Overview

**Roto Pong** is a **2D arcade game** (Pong/Breakout-inspired) set inside a **circular arena** with a **central black hole hazard**. The player controls a **curved paddle** that **orbits 360° on the inside of the arena, close to the black hole, defending it from incoming balls**. Blocks are positioned **near the outer perimeter** of the arena. The ball launches outward from the paddle toward the blocks, bounces off blocks and the outer wall, and returns toward the center. The player must intercept returning balls with the paddle to prevent them from falling into the black hole. If a ball falls into the black hole, the player loses a life (with multiball-specific rules), and the run ends at zero lives.

**Key spatial layout:**
- **Black hole:** Center of arena (loss zone)
- **Paddle:** Orbits close to black hole, defending it (~radius 55)
- **Blocks:** Near outer perimeter (~radius 280-340)
- **Outer wall:** Arena boundary (~radius 400)

The product is **web-first**, built in **Rust → WASM** with **WebGPU (wgpu)** acceleration and premium 2D shaders/VFX. It must be deployable as a static site to **GitHub Pages**, with strong **mobile browser support** (touch controls, responsive UI, safe areas).

Core pillars:
- **"One more wave" endless progression** with gentle difficulty ramp
- **Deterministic simulation + deterministic wave generation**
- **Run persistence** (LocalStorage "Continue Run") with robust corruption handling
- **Fairness + comfort** (pause menu + auto-pause on focus loss)
- **High-quality visuals** (black hole shader, particles, bloom/distortion) with performance presets

**Key update (restored + explicitly required):**
- **Between-wave "Breather Phase" is exactly 5.0 seconds** (deterministic, non-lethal).
- **Serve/Launch is required:** ball **starts attached to paddle** and launches on **click/tap** (or Space/Enter).

---

## 2) Problem Statement

Most browser arcade games are either:
- visually simple / non-accelerated,
- poorly tuned (difficulty spikes, unfair deaths),
- unreliable across sessions (no persistence, fragile saves),
- or weak on mobile (bad touch controls, non-responsive UI).

**Roto Pong** solves this by delivering a **high-performance WebGPU arcade game** that is **deterministic**, **fair**, **persistable**, and **mobile-friendly**, while providing premium lighting/shaders and satisfying "juice" without sacrificing gameplay readability.

---

## 3) Goals & Success Metrics

### Goals
1. Ship a **fully playable endless mode** with curated variety and smooth ramp.
2. Provide **Continue Run** via LocalStorage with safe **versioning, validation, migration, and corruption recovery**.
3. Guarantee **comfort & fairness**: pause menu + **auto-pause on blur/visibility loss** (no cheap deaths).
4. Achieve **premium 2D WebGPU visuals** with configurable graphics presets.
5. Deploy cleanly to **GitHub Pages** (static build, subpath-safe assets).
6. Ensure **determinism** for reproducibility (bugs, saves, fairness testing).
7. Make mobile feel first-class: **drag-anywhere touch** + large tap targets + safe-area support.
8. Ensure **Serve/Launch clarity**: ball starts "on" paddle, launches on click/tap, and is deterministic.

### Success Metrics (targets)
- **Performance (desktop):** stable **60 FPS** at 1080p on High.
- **Performance (mobile):** stable **45-60 FPS** on Medium on mid-range devices (Low preset must be playable).
- **Input latency:** paddle response **< 50ms** (input → visible motion).
- **Persistence reliability:** **> 99%** successful Continue loads for compatible saves; corrupted saves never crash and always provide a recovery path.
- **Auto-pause reliability:** **100%** pause on `visibilitychange` hidden and `window.blur` (QA gate).
- **Determinism:** same `(save + inputs)` yields the same **state hash** after 10 seconds (CI + browser E2E).
- **VFX budget:** configurable caps; High supports **~5k particles** without dropping below FPS targets (mobile lower).
- **Serve usability:** in playtests, ≥ **95%** of new players successfully launch within 10 seconds without confusion (qualitative + instrumentation if enabled).

---

## 4) Target Users

- **Casual arcade players** who enjoy short-to-medium sessions and "one more wave" loops.
- **Skill learners** who want predictable physics and fairness (learnable bounces).
- **Mobile web players** who expect touch-first controls and readable UI.
- **Web tech enthusiasts** interested in WebGPU visuals and performance.
- **Comfort/accessibility-minded players** needing reduced motion/flashing and high contrast options.

Platforms (**WebGPU required; no fallback renderer**):
- Desktop: Chrome/Edge/Firefox (supported WebGPU builds), Safari on macOS where WebGPU is available
- Mobile: iOS Safari (WebGPU-capable), Android Chrome (WebGPU-capable)

If WebGPU is unavailable, show a clear "Not Supported / How to enable" screen.

---

## 5) User Stories (3-5)

1. **As a player, I want** to rotate a curved paddle smoothly around the arena **so that** I can reliably return the ball and improve through practice.
2. **As a player, I want** the ball to start "on" the paddle and launch on click/tap (or Space/Enter) **so that** each wave/serve feels intentional and fair.
3. **As a player, I want** endless waves with a gentle difficulty ramp and varied block patterns **so that** each run stays fun without sudden unfair spikes.
4. **As a returning player, I want** my run saved locally and resumable **so that** I can continue after closing/reloading the tab.
5. **As a player, I want** the game to auto-pause when the tab loses focus **so that** I don't lose a life due to real-world interruptions.

---

## 6) Functional Requirements

### 6.1 Core Game Loop (Endless)
- Flow: **Boot → Main Menu → Start New Run / Continue → (Serve) → Active Wave → Wave Clear → (5s Breather Phase) → Next Wave → (Serve) → … → Life Loss → Respawn/Serve → … → Game Over**
- **Wave clear condition:** all blocks destroyed (`blocks.len() == 0`).

#### Wave Transition "Breather" (Updated Requirement)
- After wave clear, enter a deterministic **breather phase lasting exactly 5.0 seconds** (tunable only via `tuning.ron`, default and MVP target is **5s**).
- During the breather phase:
  - Simulation remains deterministic and continues ticking.
  - Gameplay is **non-lethal and non-interactive** to prevent cheap deaths and reduce cognitive load:
    - Balls are **despawned** or **frozen** (MVP recommendation: despawn all balls deterministically).
    - The black hole cannot consume a ball (because none are active).
    - Pickups on screen are cleared (optional; if kept, TTL continues deterministically).
  - Display a wave banner + brief stats (e.g., "Wave 12 Cleared") and a visible countdown ring.
- At the end of 5s:
  - Generate next wave deterministically.
  - Enter **Serve state** (ball attached to paddle; player must launch).

> Note: This replaces the earlier ~1.25s intermission. "Breather waves" as a *special lower-intensity generated wave* may still exist, but the mandatory between-wave **breather phase** is explicitly **5 seconds**.

#### Serve / Launch (Required)
At the start of a run, after respawn, and after each breather:
- Spawn exactly **one** ball and place it at a deterministic offset from the paddle centerline (e.g., on paddle surface slightly inward).
- Ball is in **Attached** state:
  - Position follows paddle rotation each tick.
  - Velocity is zero (or ignored) until launch.
  - Ball does not interact with blocks/walls/black hole while attached (or collision checks are skipped).
- Launch trigger:
  - Desktop: left click on playfield OR Space/Enter.
  - Mobile: tap on playfield OR dedicated "Launch" button (recommended to reduce ambiguity with drag).
- Launch behavior (deterministic):
  - On input event applied at tick boundary, ball transitions to **Free** state with deterministic initial velocity:
    - Direction: radially outward through paddle angle, plus optional small tangential component from paddle angular velocity (capped).
    - Speed: from tuning (`ball_start_speed(wave)`).

#### Lives
- Start with **3** (tunable), cap **5** (tunable).
- **Multiball life policy (required for multiball-enabled builds):** lose a life only when the **last active ball** is consumed by the black hole.
- Optional cadence: +1 life every N waves up to cap (tunable; defaults off unless playtesting supports it).

#### Respawn
- After life loss, delay ~**0.85s** (tunable), then enter **Serve**.

---

### 6.2 Controls (Desktop + Mobile)
Required input methods:
- **Mouse (desktop default):** paddle angle follows pointer angle around center: `atan2(y - cy, x - cx)` with smoothing to reduce jitter.
- **Keyboard:** A/D or ←/→ rotate; Esc pause; Space/Enter launch.
- **Touch (mobile required):** **drag anywhere** to set paddle angle using the same angle-from-center rule.
- **Launch:** tap playfield or press Launch button (mobile), click or Space/Enter (desktop).

**Input priority rules (required):**
- Pointer activity in last ~250ms takes priority; keyboard overrides while pressed.
- In menus/pause: gameplay inputs ignored except resume/confirm/back.

**Onboarding hint (required):**
- Show control hint (5s, dismissible, stored in settings) based on detected input method.
- Must mention **"Tap/Click to Launch"** and "Drag to rotate".

---

### 6.3 Physics & Collision (Curved World)
- Arena is circular with concentric zones (from center outward):
  - **Black hole** (center): `R_hole` ~40, `R_hole_loss` ~35 (ball consumed if `dist_to_center <= R_hole_loss`)
  - **Paddle zone** (inner): `R_paddle` ~55 (paddle orbits here, defending the black hole)
  - **Play zone** (middle): Open space for ball travel
  - **Block zone** (outer): `R_blocks` ~280-340 (blocks positioned near perimeter)
  - **Outer wall**: `R_outer` ~400 (arena boundary)
- Paddle and blocks are **thickened arc segments** (polar-space arcs).
- Ball is a circle with velocity; collisions:
  - Outer wall: reflect to keep ball inside arena.
  - Blocks: reflect unless ball is in **piercing** mode; apply damage rules.
  - Paddle: reflect with **position-based deflection** (hitting edges deflects ball at angle, center reflects straight back); optional "english" from paddle angular velocity.
  - **Paddle collision includes cooldown** to prevent ball sticking (6 ticks) and direction check (only reflects if ball moving toward paddle).

**Tunneling prevention (required):**
- Fixed-timestep simulation with **substepping / continuous collision handling** to avoid passing through thin arcs at high speed.

---

### 6.4 Deterministic Simulation (Hard Requirement)
- Use a **fixed timestep** (default **120Hz**) and stable iteration order (IDs ascending).
- All randomness uses a deterministic RNG; RNG state must be persisted in saves.
- Renderer/VFX must not influence simulation results.

Brief loop example (illustrative):
```rust
while accumulator >= dt && substeps < substep_max {
  let commands = input.drain_commands(); // deterministic input events
  sim.tick(dt, &commands);              // pure deterministic
  accumulator -= dt;
  substeps += 1;
}
```

---

### 6.5 Endless Progression & Tuning (Data-Driven)
- Difficulty ramps smoothly by wave index `w`.
- Tuning must be loaded from `assets/tuning.ron` (or similar) and include key parameters:
  - Ball start/max speed curves (with caps)
  - Paddle arc width progression (slow shrink to a minimum)
  - Block count range and ramp
  - Power-up drop chance + **pity timer**
  - **Breather phase duration** (required; default 5.0s)
  - Breather wave cadence (lower-intensity wave templates; optional but recommended)
  - Serve rules (attached offset, launch base direction/tangential cap, etc.)
  - VFX quality caps and preset defaults
- Tuning changes must be associated with a **tuning hash** to protect deterministic Continue.

**Required progression behaviors**
- **Gentle ramp:** no sudden jumps in speed or density.
- **Breather waves (required as pacing tool in v1):** every N waves (e.g., 10), reduce density and restrict certain specials.
- **Anti-spike constraints (required):**
  - Clamp per-wave deltas (e.g., block count and speed cannot jump beyond tuning thresholds).
  - Always ensure at least one viable "safe lane" (see generator).

---

### 6.6 Deterministic Wave/Block Generator (Fairness-Critical)

The wave generator must be deterministic, reproducible, and validated.

**Generator inputs (required):**
- `wave_index`
- deterministic RNG state (PCG32 or equivalent)
- tuning
- run context (e.g., previous template ID, "breather" flag, guarantee-due flags)

**Generator outputs (required):**
- list of curved arc blocks (polar geometry)
- wave metadata (template id, attempts, special counts, safe-lane span)
- updated RNG state

#### MVP 1 Simplified Generator (Explicit Requirement)
To reduce risk while retaining the "curved blocks" identity, MVP 1 may use a simplified deterministic generator:
- Generate **1-3 concentric rings** of arc blocks per wave (tunable).
- Each ring:
  - has deterministic radius and thickness,
  - is segmented into arc blocks (e.g., 8-24 segments based on wave),
  - obeys safe-lane and coverage constraints.
- The "safe lane" is implemented by reserving an angular corridor across relevant rings (no blocks spawned there).

This MVP generator must still:
- be deterministic (seeded, stable iteration),
- enforce constraints (below),
- provide bounded attempts and fallback.

#### Fairness constraints (hard)
- **Safe lane:** contiguous angular corridor of at least **0.40 rad** (tunable) clear of blocks in mid/outer bands.
- Band coverage caps to avoid "full ring walls" (tunable; enforced).
- No invalid placements: blocks never overlap illegal radii (too close to black hole or outside outer wall margins).
- Special caps per wave (tunable) enforced strictly (explosive, portal pairs, prism, etc.).
- If generator cannot find a valid wave within bounded attempts (e.g., 24), it must fall back to a simple valid pattern.

#### Templates (v1 requirement)
- Provide a small library of parametric patterns (e.g., ring slices, spiral ladder, gates/corridors, constellation/sparse, chords/crown). Exact list can be tuned, but must support variety and safe-lane guarantees.

#### Testing requirement
- Property/soak test generation across many seeds (e.g., 10k waves) must show **0** fairness violations.

---

### 6.7 Blocks (Curved) - Required Types

All blocks are arc segments with: `id`, `radius`, `thickness`, `theta_start/end`, `hp`, `kind`.

#### MVP 1 minimum
- **Glass:** HP 1, standard break.

#### Required block kinds and behaviors (v1)
- **Glass:** HP 1, standard break.
- **Armored:** HP 2 (later waves may reach 3 via tuning), sparks on hit.
- **Explosive:** on destroy, deals limited deterministic AoE damage; cap chain depth to avoid runaway clears.
- **Prism:** on destroy, spawns one extra ball (if under cap) OR a deterministic refract effect (choose one for v1; recommended: spawn ball).
- **Portal (paired):** teleports ball to paired portal with deterministic exit offset; per-ball cooldown to prevent loops; must validate "exit safety".
- **Pulse:** periodic or hit-based deterministic velocity tweak (subtle, variety without chaos).
- **Magnet:** applies subtle force; **must not** pull radially inward toward the black hole in v1 (tangential bias recommended).
- **Power-Up Capsule block:** HP 1; on destroy spawns a pickup.

---

### 6.8 Power-Ups (Pickups) - Required Set

Pickups spawn from capsules and are collected by intersecting paddle arc span. Pickups expire after a lifetime (e.g., 10s).

**MVP 1 stance (allowed):** power-ups may be disabled initially to stabilize physics + generator fairness.
**v1 required set (when enabled):**
1. **Multi-ball:** spawn +2 balls (respect deterministic spread and max ball cap).
2. **Slow:** multiply ball speeds by a factor (e.g., 0.65) for a duration; refresh duration on re-collect.
3. **Piercing:** ball passes through blocks (no reflection) for duration; still collides with paddle and outer wall (recommended).
4. **Widen Paddle:** increase paddle arc width multiplier for duration; refresh on re-collect.
5. **Shield:** prevents the next black hole consumption by reflecting the ball outward; consumes shield charge; duration or until used (no stacking beyond 1 charge in v1).

Rules (required):
- Display active effects (icons + timers) in HUD.
- Cap max simultaneous balls (e.g., 8) and max pickups on screen (tunable).
- Power-up selection uses deterministic weighted RNG from tuning.

---

### 6.9 Scoring & Run Stats
- Score awarded primarily on block destroys and wave clears (values tunable).
- Optional combo: increases with consecutive block destroys without losing a life (simple multiplier cap).
- Track run stats for game over screen: score, wave reached, time survived, blocks destroyed, highest combo, power-ups collected, balls lost to black hole.
- In dev/test builds, display seed and state hash (optional).

---

### 6.10 Persistence: Continue Run (LocalStorage) - **Required (Expanded: format/versioning/validation/corruption + tests)**

#### 6.10.1 Storage Keys (v1)
Use explicit, versioned LocalStorage keys to avoid collisions and enable parallel schema evolution:
- `roto.settings.v1` - settings JSON
- `roto.run.save.v1` - primary run save JSON (or JSON envelope containing base64 payload)
- `roto.run.bak.v1` - last known-good backup
- `roto.run.tmp.v1` - staged write (best-effort; may be deleted on success)
- `roto.run.meta.v1` - optional small metadata for menu display (wave, score, updated_at), duplicated for fast menu load

#### 6.10.2 Save Cadence (v1 decision)
- **Required:** save at **wave boundaries**, **pause events**, and **focus-loss auto-pause** (debounced).
- **Optional (not required for MVP):** mid-wave checkpoint saves (adds complexity; can be added post-MVP).

This cadence minimizes write frequency and corruption exposure while preserving user trust.

#### 6.10.3 Save Format: Versioned Envelope
- **Envelope is JSON** for debuggability and resilience.
- Simulation state payload is either:
  - **Option A (recommended for MVP):** direct JSON via `serde` (simple, larger).
  - **Option B (later optimization):** `postcard`/`bincode` bytes encoded as base64 inside JSON envelope.

**v1 (MVP) requires Option A** unless size becomes an issue in playtests.

**Canonical schema (v1):**
```json
{
  "schema_version": 1,
  "content": "run_state",
  "game_build": {
    "git_sha": "abcdef0",
    "build_time_utc": "2026-02-03T00:00:00Z"
  },
  "tuning": {
    "hash": "blake3:...",
    "name": "default"
  },
  "timestamps": {
    "created_at_ms": 1730000000000,
    "updated_at_ms": 1730000123456
  },
  "integrity": {
    "algo": "blake3-128",
    "digest": "base64...",
    "canonicalization": "serde_json_canonical"
  },
  "data": {
    "run_state": { "...": "..." }
  }
}
```

**Canonicalization requirement:** Integrity digest must be computed over a **canonical byte representation** to avoid JSON map-order instability.
- Recommended approach: use `serde_json_canonicalizer`-style deterministic ordering OR avoid canonicalization by hashing a stable binary encoding of the `run_state`.
- **MVP requirement:** Implement one of:
  1) `blake3(run_state_as_postcard_bytes)` stored in envelope, or
  2) canonical JSON (stable ordering + no whitespace).

#### 6.10.4 Versioning Rules
- `schema_version` changes when the envelope structure or semantics change.
- `run_state_version` (nested) may be introduced if `RunState` evolves frequently. For MVP, `schema_version` is sufficient, but code should be structured to easily add `run_state_version`.
- **Backwards compatibility:** must support loading at least **one prior schema** once v2 exists (migration path required).
- **Forwards compatibility:** unknown fields must be ignored where safe (`#[serde(default)]` + `Option<T>`). Unknown enum variants must map to a safe default (e.g., unknown block kind → `Glass`).

#### 6.10.5 Validation & Invariant Checking (Hard Requirement)
Load must be a two-stage process:
1) **Decode + integrity verification**
2) **Semantic validation** (invariants + clamping rules)

**Integrity checks (required):**
- Reject if:
  - `schema_version` unsupported
  - integrity digest mismatch
  - `tuning.hash` mismatch (unless migration explicitly allows)
- If primary fails: attempt backup.

**Semantic validation (required; reject or clamp as specified):**
- No NaN/Inf in any float fields (positions/velocities/theta). If found → reject save.
- Ranges/clamps (examples; exact numbers tunable but must exist):
  - `wave_index`: `0..=u32::MAX` (reject if absurdly large beyond configured max, e.g., > 1_000_000)
  - `lives`: clamp to `0..=lives_cap`
  - `score`: clamp to `0..=u64::MAX` (reject if negative in old schema)
  - `paddle.theta`: wrap into `[-pi, pi)`; `arc_width` clamp to `[min, max]`
  - `balls.len()`: clamp to `[1, max_balls]` by dropping extras deterministically (highest IDs first)
  - All entity IDs must be unique; if duplicates → reject
  - Portal pairing integrity: if invalid → convert portals to Glass (deterministic) or reject (choose one). **MVP requirement:** convert invalid portals to Glass to allow recovery.
- Deterministic ordering normalization:
  - Sort `balls`, `blocks`, `pickups` by `id` after load to restore stable iteration order.

**Validation result must be explicit** for UI messaging and QA:
```rust
enum SaveLoadError {
  NotFound,
  UnsupportedSchema { found: u32 },
  IntegrityMismatch,
  TuningMismatch { save: String, current: String },
  DecodeFailed,
  InvariantViolation { reason: String },
  QuotaExceeded,
}
```

#### 6.10.6 Corruption Handling & Recovery UX (Hard Requirement)
Behavior must never crash; it must be user-recoverable.

**Load flow:**
1. Try `roto.run.save.v1`
2. If fail, try `roto.run.bak.v1`
3. If fail:
   - Disable "Continue"
   - Show reason category:
     - "No save found"
     - "Save is from an incompatible version"
     - "Save appears corrupted"
     - "Save is incompatible with current tuning"
   - Provide action: **Delete Save** (clears save/bak/tmp/meta)

**Write flow (LocalStorage-safe "atomic" pattern):**
LocalStorage has atomic `setItem`, but no multi-key transaction. Implement best-effort staging + backup rotation:

1. Serialize envelope → `bytes`
2. Compute integrity digest and embed
3. `setItem(tmp_key, bytes)`
4. Read back `tmp_key` and verify digest (guards against partial writes / encoding issues)
5. Rotate backup:
   - Read current `save_key` (if exists) and write to `bak_key`
6. Commit:
   - Write `save_key = tmp_value`
7. Cleanup:
   - Remove `tmp_key` (best-effort)
8. Update `meta_key` (small, independent; failure here must not affect save)

If any step throws due to quota or browser restrictions:
- Keep existing `save_key` and `bak_key` intact.
- Surface non-blocking toast/log and continue gameplay; next pause/wave boundary retries.

#### 6.10.7 Migration Strategy (Required)
- Migration is implemented as a chain: `v0 -> v1 -> v2 ...` with unit tests.
- Migration rules must be deterministic and safe:
  - Missing fields: fill with defaults consistent with tuning.
  - Unknown block kinds: map to `Glass`.
  - Old physics params: clamp to current legal ranges.
- If migration cannot guarantee safety (e.g., missing critical RNG state), reject and require new run.

#### 6.10.8 Save Size & Quota Considerations
- Target: save envelope under ~200KB (soft target); must not exceed typical LocalStorage limits (5-10MB across origin).
- If save would exceed a threshold:
  - Drop non-critical fields first (e.g., cosmetic/VFX-only metadata must not be in RunState)
  - Prefer compact encodings (post-MVP Option B)

---

### 6.11 Pause & Auto-Pause (Comfort-Critical)
- **Manual pause:** Esc and on-screen pause button (mandatory on mobile).
- **Auto-pause triggers (required):**
  - `document.visibilitychange` to hidden
  - `window.blur`
- No auto-resume on refocus; player must explicitly resume.
- Save on pause trigger (debounced) without blocking UI.

---

### 6.12 UI Screens (Web)
Required screens/modals:
- Boot/loading (including WebGPU not supported message)
- Main menu: Start New Run, Continue (disabled with reason), Settings, How to Play, Delete Save (if present)
- In-game HUD: score, wave, lives, active power-ups, pause button, and **Serve "Launch" prompt**
- Pause overlay: resume, restart run, settings, main menu (with confirmations)
- Game over: stats + restart/menu actions

Mobile UI requirements:
- Responsive layout; landscape-optimized but portrait supported unless explicitly decided otherwise.
- Tap targets ≥ **44×44 CSS px**
- Respect safe areas via `env(safe-area-inset-*)`
- Prevent accidental zoom; include viewport meta.

---

### 6.13 VFX & Graphics (WebGPU) - Required
Visual identity is central, but must remain readable and performant.

Required elements:
- Animated **black hole**: event horizon ring + accretion swirl + infall particles (render-only).
- Block hit/break particles by block type.
- Post-processing options: **bloom** and **distortion/lensing** (quality dependent).
- Quality presets: **Low / Medium / High** with caps (particles, bloom strength, distortion on/off).
- Accessibility toggles (see NFR): reduced motion/flashing must clamp strong effects.

Important boundary: VFX must not affect determinism; particles can be GPU-simulated or CPU-simulated independently from gameplay sim.

---

### 6.14 Audio (v1)
- SFX for: wall/paddle hits, each block type hit/break, pickup collect, black hole consume, wave clear, **launch**.
- Optional music track (nice-to-have).
- Sliders: master/SFX/music; "mute when unfocused" default on.

---

### 6.15 Deployment (GitHub Pages) - Required
- Static build output: `index.html`, wasm, JS glue, assets.
- Must work when hosted from a **subpath** (GitHub Pages repo URL); all asset paths relative.
- Provide a GitHub Actions workflow to build and deploy.

---

## 7) Non-Functional Requirements

### Performance & Compatibility
- Desktop: 60 FPS @ 1080p on High (mid-range laptop baseline).
- Mobile: 45-60 FPS on Medium (mid-range devices), avoid overheating via preset defaults.
- Load size targets: initial wasm/js under ~5MB where feasible; total assets under ~15MB (tunable but tracked).
- **WebGPU required; no fallback renderer.** Show friendly unsupported UI otherwise.

### Security & Privacy
- No backend; no PII collection.
- All storage is LocalStorage only.

### Determinism & Testability
- Simulation must be pure deterministic: fixed dt, stable ordering, deterministic RNG.
- Provide a **state hash** function for CI validation (quantize floats before hashing).
- Headless sim tests must run in Rust without WebGPU.

### Accessibility & Comfort
- **Reduced Motion:** disable shake, reduce distortion, reduce particle intensity significantly.
- **Reduced Flashing:** clamp bloom/shockwave intensity; avoid rapid full-screen flashes.
- **High Contrast:** stronger outlines and HUD readability.
- **Keyboard navigable menus** with visible focus state.
- UI scale options (e.g., 80-150%) without breaking layout.

### Reliability (Persistence-specific)
- Save/load must be **crash-free** under all malformed input conditions.
- Corrupted saves must never block "New Run".
- Save writes must be resilient to quota exceptions; failures must not destroy existing valid save/bak.

---

## 8) Technical Architecture

### High-Level Design
**Two deterministic layers + one non-deterministic layer:**
1. **Simulation (deterministic):** entities, collisions, serve/launch, power-ups, scoring, wave progression
2. **Generator (deterministic):** wave templates + fairness validation + bounded retries + fallback
3. **Renderer/UI (non-deterministic allowed):** WebGPU rendering, particles, bloom; DOM UI overlay

### Recommended Module Breakdown (Rust)
- `platform/` browser glue: time, input, visibility/blur events, LocalStorage
- `sim/` deterministic core: state, tick, collisions, serve/launch, power-ups, scoring
- `sim/generator/` deterministic wave generator + validators + templates
- `tuning/` load/validate `tuning.ron`, compute tuning hash
- `persistence/` save/load/migrate/validate + integrity digests + tmp/bak rotation
- `renderer/` wgpu pipelines, instancing, post-processing, particle rendering
- `ui/` menu/HUD/pause/settings (recommended: DOM overlay for accessibility + responsive layout)

### Key Technical Choices (Locked)
- Rust → WASM, WebGPU via `wgpu`
- **WebGPU-only (no WebGL2 fallback)**
- Deterministic RNG (PCG32 recommended); persist RNG state
- Fixed timestep simulation (120Hz default) + substep cap
- Static GitHub Pages deployment

---

## 9) Data Model

### Core Simulation Entities
- **RunState**
  - `seed: u64` (for repro/debug; seed set at new run start)
  - `rng_state: Pcg32State` (authoritative for determinism)
  - `wave_index: u32`, `lives: u8`, `score: u64`, `combo: u32`, `time_ticks: u64`
  - `paddle: PaddleState { theta, arc_width, omega? }`
  - `balls: Vec<Ball>`
  - `blocks: Vec<BlockArc>`
  - `pickups: Vec<Pickup>`
  - `active_effects: ActiveEffects` (slow/piercing/widen/shield timers + shield charged)
  - `phase: Phase` (required): `Serve | Playing | Breather | Paused | GameOver`
  - `last_template_id: Option<String>`

- **Ball**
  - `id`, `pos`, `vel`, `radius`, `mode: Normal|Piercing`
  - `state: Attached { paddle_offset: f32 } | Free`

- **BlockArc**
  - `id`, `kind`, `hp`
  - `radius`, `thickness`, `theta_start`, `theta_end`
  - for portals: `pair_id` or pairing reference

- **Pickup**
  - `id`, `kind`, `pos`, `vel`, `ttl_ticks`

### Persistence Envelopes
- **SaveEnvelope (schema v1)**
  - `schema_version: u32`
  - `content: String` (e.g., `"run_state"`)
  - `game_build: { git_sha, build_time_utc }`
  - `tuning: { hash, name }`
  - `timestamps: { created_at_ms, updated_at_ms }`
  - `integrity: { algo, digest, canonicalization }`
  - `data: { run_state: RunState }`

- **Settings**
  - graphics: quality preset, bloom/distortion toggles, particle intensity, reduced motion/flashing
  - controls: scheme preference, sensitivity, invert, keyboard rate
  - accessibility: high contrast, UI scale, palette
  - audio: volumes, mute when unfocused
  - onboarding: hints dismissed

Relationships:
- RunState owns all entities; entities are referenced by IDs and stable-ordered for determinism.
- Portal blocks must be paired consistently (integrity validated on load and generation).

---

## 10) API Endpoints (if applicable)

No backend APIs (web-only offline).
Optional **test hooks** may be exposed in dev/test mode via `window.rotoTest` for CI E2E (e.g., get state hash, step ticks). These must not ship gameplay advantages.

---

## 11) UI/UX Considerations

- **Readability first:** ball must remain visible against effects (outline/halo required); bloom must not wash out black hole boundary.
- **Touch-first constraints:** drag-anywhere control avoids finger occlusion; pause button always visible on mobile.
- **Serve clarity:** explicit "Tap/Click to Launch" prompt; optional HUD Launch button on mobile.
- **Clear Continue messaging:** if Continue disabled, show *why* (no save / incompatible / corrupted / tuning mismatch).
- **Pause transparency:** overlay should indicate whether pause was manual or "Focus Lost."
- **Wave transition breather (5s):** countdown + "Next Wave" preview; ensure it feels like a reward/rest, not dead time.
- **Settings apply immediately** where possible, but must not change deterministic sim results (except input mapping).

---

## 12) Dependencies & Integrations

### Rust/WASM/WebGPU
- `wgpu`, `wasm-bindgen`, `web-sys`
- `glam` (math), `bytemuck` (GPU buffers)
- `serde` + `serde_json` (saves/settings), `ron` (tuning)
- `crc32fast` or `blake3` (checksums/hashes; **blake3 recommended** for save integrity + determinism digest)
- `log` + `console_log` (debug)
- Testing (recommended):
  - `proptest` (property tests)
  - `wasm-bindgen-test` (WASM unit/integration tests)

### Tooling / Build / Deploy
- Build: `trunk` (recommended) or `wasm-pack` + bundler
- Optimization: `wasm-opt` in release
- CI: GitHub Actions
- Deploy: GitHub Pages (artifact deploy)
- E2E testing: Playwright (desktop + mobile emulation)

No external services are required.

---

## 13) Timeline & Milestones (Suggested)

### Phase 0 - Technical Spikes (1-2 weeks)
- Prototype arc collision + reflection stability
- Fixed timestep loop + deterministic RNG integration
- Minimal wgpu render of arena + paddle + ball
- Persistence spike: implement envelope + integrity digest + basic validation

### Phase 1 - MVP Endless (2-3 weeks)
- Endless wave loop with basic generator + 2 block types (Glass/Armored)
- HUD + main menu + pause + auto-pause
- Implement **Serve/Launch** (ball attached; click/tap/Space to launch)
- Implement **5-second between-wave breather phase**
- Persistence v1: save/load + tmp/bak rotation + corruption UX

### Phase 2 - Content & Fairness (2-4 weeks)
- Implement remaining block types + power-ups
- Deterministic wave templates + fairness validation + fallback wave
- Add tuning system (`tuning.ron` + hash), breather waves, pity timer

### Phase 3 - Mobile & UX Polish (2-3 weeks)
- Touch controls + safe areas + responsive layout
- Performance presets and mobile defaults
- How to play + onboarding hints + settings UX

### Phase 4 - VFX + Accessibility + Test Hardening (2-4 weeks)
- Black hole shader, particles, bloom/distortion
- Reduced motion/flashing, high contrast, UI scale
- Determinism hash tests + generator property tests + **save/load corruption + migration test suite**

### Phase 5 - Release (1 week)
- Cross-browser/device QA
- GitHub Pages deployment + final smoke tests

---

## 14) Risks & Mitigations

1. **Unfair or "impossible" waves**
   - Mitigation: enforce safe lane + coverage caps + bounded retries + fallback pattern; property tests across many seeds.

2. **Non-determinism creeping in (breaks saves/repro)**
   - Mitigation: strict separation of sim vs renderer; fixed dt; stable ordering; deterministic RNG only; state-hash CI checks.

3. **Mobile performance issues with WebGPU/VFX**
   - Mitigation: default to Medium/Low on mobile; particle caps; distortion off on Low; profiling on physical devices early.

4. **Touch control inconsistency across iOS/Android**
   - Mitigation: keep gestures simple (drag-anywhere); test iOS Safari and Android Chrome repeatedly; avoid exotic pointer edge cases.

5. **Save corruption / incompatible versions / tuning mismatch**
   - Mitigation: tmp/bak staging + integrity digest + strict validation + deterministic migration; clear UI recovery path (Delete Save); load fallback to backup.

6. **5-second breather feels too long for some players**
   - Mitigation: include a Settings option to reduce UI animation intensity; keep breather phase visually engaging (countdown, wave preview). (Duration remains 5s for MVP per requirement; revisit post-MVP only if explicitly approved.)

7. **Serve/Launch confusion**
   - Mitigation: strong prompt + optional explicit Launch button on mobile; ensure drag does not accidentally launch.

---

## 15) Future Considerations

- Gamepad support (left stick radial aim)
- Daily seed mode (offline)
- Multiple save slots; export/import save strings
- More power-ups and block variants (careful with fairness caps)
- Optional curated "level set" mode (fixed seeds/patterns)
- Lightweight offline leaderboards (still local-only) or optional online leaderboard (would add backend and privacy review)
- GPU-compute particle simulation for higher density on desktop

> Explicitly **not planned**: a WebGPU fallback renderer (WebGL2). The product stance is WebGPU-only.

---

---

## 16) Success Criteria (Definition of Done)

### Product/UX
- New player can start a run within **≤ 10 seconds** from load (no confusing gating).
- Controls feel responsive on:
  - Desktop mouse/trackpad
  - Mobile touch (iOS Safari, Android Chrome)
- Serve/Launch is clear:
  - Ball visibly "attached" to paddle before launch
  - Prompt displayed; input works reliably
- "Continue" behavior is unambiguous:
  - Enabled only when a valid save exists
  - If disabled, UI explains why (no save / incompatible / corrupted / tuning mismatch)
- Readability holds across all presets:
  - Ball remains visible at all times (outline/halo)
  - Black hole boundary remains clearly defined
- Between-wave **breather phase is 5 seconds** and does not allow accidental death.

### Technical
- Deterministic sim invariants hold:
  - Same seed + same input stream ⇒ identical state hash at fixed checkpoints
- Stable performance targets (minimum):
  - Desktop: 60 FPS on mid-range GPU with Medium preset
  - Mobile: 45-60 FPS on recent devices with Low/Medium preset
- Save system is robust:
  - Staged write (tmp) + backup rotation + integrity validation
  - Corrupt save never crashes the game; recovery path exists
  - Migration tests pass for supported prior versions

---

## 17) Quality Assurance & Test Plan

### 17.1 Determinism Test Suite (CI)
- **Golden replay tests**
  - Record input stream (timestamped input events mapped to fixed-tick frames)
  - Re-run simulation headless for N ticks
  - Assert exact match on:
    - Final state hash
    - Key scalar values (score, wave index, ball count, RNG counter)
- **Cross-build determinism**
  - Run golden replay on:
    - Native (Rust) test harness
    - WASM (node or headless browser) harness
  - Hash comparison must match for supported platforms/browsers

### 17.2 Procedural Generator Property Tests
Generate many seeds and validate constraints:
- At least one safe lane at spawn
- Coverage and density caps satisfied
- No unavoidable immediate-loss patterns within first X seconds (as defined by the fairness model)
- Ensure fallback wave triggers when retries exceed limit

### 17.3 Save/Load Validation, Migration, and Corruption Tests (Expanded)

#### Unit Tests (native Rust)
1. **Roundtrip:**
   - Given a representative `RunState`, `serialize -> deserialize` returns identical state (or semantically identical after normalization sort-by-id).
2. **Integrity:**
   - Modifying any byte/field in `data.run_state` causes `IntegrityMismatch`.
3. **Invariant enforcement:**
   - NaN in any float → `InvariantViolation`
   - Duplicate IDs → `InvariantViolation`
   - `lives` above cap clamps (or rejects-must match spec); verify exact behavior
   - Invalid portal pairing maps to Glass (verify deterministic mapping)
4. **Tuning mismatch:**
   - Save with tuning hash A, load with tuning hash B → `TuningMismatch`.
5. **Unsupported schema:**
   - `schema_version` > current → `UnsupportedSchema`.
6. **Normalization:**
   - Unsorted vectors load and become sorted by `id`; confirm stable order.
7. **Serve/Launch determinism:**
   - Given identical tick-quantized launch input, ball initial velocity and position after N ticks match expected hash.

#### Migration Tests
- Maintain fixtures as JSON files under `tests/fixtures/saves/`:
  - `save_schema_v1_example.json`
  - `save_schema_v0_legacy.json` (once v2 exists)
- For each fixture:
  - Load with migration chain
  - Assert expected fields present and within legal ranges
  - Assert determinism-critical fields retained (seed, RNG state)

#### Corruption / Fuzz Tests
- **Truncation:** remove last N bytes → must fail gracefully and attempt backup.
- **Random bit flips:** flip K random characters/bytes in JSON → must fail gracefully.
- **Type confusion:** replace numbers with strings, objects with arrays → decode fails but no panic.
- **Oversized payload:** extremely large arrays → loader must cap or reject without OOM (use explicit length checks).
- **LocalStorage quota exception simulation:** mock storage throwing on setItem; ensure old save remains valid.

**Recommended tooling:**
- `proptest` for structured mutation of save strings.
- `cargo fuzz` (optional) for decode/validate entry points.

#### Browser/WASM Integration Tests
Using `wasm-bindgen-test` (headless) or Playwright:
- Save on pause → reload page → Continue works.
- Corrupt `roto.run.save.v1` but keep `bak` valid → load falls back to backup automatically.
- Corrupt both → Continue disabled + Delete Save works; New Run works.

### 17.4 Rendering & UX Regression (Playwright)
- Smoke checks:
  - Load → New Game → Serve prompt visible → Launch works
  - Clear wave → verify **5-second breather** banner + countdown → next wave spawns → Serve again
  - Pause overlay visible and resumes correctly
  - Auto-pause on tab blur/visibility loss
  - Touch drag moves paddle (mobile emulation)
- Visual sanity snapshots (non-pixel-perfect; structural):
  - HUD elements inside safe area
  - Pause overlay legible
  - Breather countdown legible and not blocked by notch/home bar

### 17.5 Manual Device Matrix (Minimum)
- iOS Safari (latest, and one prior if feasible)
- Android Chrome (latest)
- Desktop Chrome, Firefox, Safari
- Low-power laptop / integrated GPU sanity run (Low preset)

---

## 18) Launch / Release Checklist

### Build & Packaging
- `wasm-opt` enabled in release builds
- Asset compression (textures/audio) verified
- Cache headers appropriate for GitHub Pages (hashed filenames preferred)

### Compatibility
- WebGPU availability checks:
  - If WebGPU unavailable, show clear message (no fallback)
- Pointer/touch:
  - No scroll/zoom conflicts during gameplay area interactions
  - Safe-area padding applied on iOS notch devices
  - Tap-to-launch does not trigger unexpectedly while dragging

### Stability
- No console errors in release build
- Save migration behavior verified for at least one older version (if any exist)
- Accessibility toggles tested:
  - Reduced motion reduces screen shake and particle velocity/intensity
  - Flashing reduction clamps bursts and bloom spikes
  - UI scale affects HUD/layout without overlap
- Save corruption handling verified:
  - Primary corrupt → backup loads
  - Both corrupt → Continue disabled, Delete Save clears, New Run works
- Wave transitions verified:
  - Breather is exactly **5 seconds** (± one simulation tick tolerance) and non-lethal.
- Serve/Launch verified:
  - Desktop click + Space/Enter
  - Mobile tap + optional Launch button

### Documentation
- "How to Play" updated to reflect final controls, Serve/Launch, and power-ups
- Changelog included in repo releases (if using GitHub Releases)

---

## 19) Open Questions (To Resolve Before MVP Lock)

1. **Audio scope**
   - Minimal SFX only, or music track(s)? (Affects loading, UX, tuning.)
2. **Exact definition of "breather wave" cadence**
   - Fixed interval (e.g., every 5 or 10 waves) vs adaptive (after near-death).
3. **Continue semantics**
   - Save on every wave boundary only, or mid-wave as well? (**This PRD sets v1 to boundary+pause+focus-loss.**)
4. **Input recording**
   - Needed only for tests, or exposed as a user-facing "share seed + replay" feature later?
5. **Mobile Launch UX**
   - Tap anywhere vs dedicated Launch button vs both? (Recommended: both; allow disabling button in settings.)

> Resolved: **WebGPU-only; no fallback renderer**.
> Resolved: **Between-wave breather phase is exactly 5.0 seconds**.

---

## 20) Appendix

### 20.1 Save Data Envelope (Proposed, updated)
```json
{
  "schema_version": 1,
  "content": "run_state",
  "game_build": { "git_sha": "abcdef0", "build_time_utc": "2026-02-03T00:00:00Z" },
  "tuning": { "hash": "blake3:…", "name": "default" },
  "timestamps": { "created_at_ms": 1730000000000, "updated_at_ms": 1730000123456 },
  "integrity": { "algo": "blake3-128", "digest": "base64…", "canonicalization": "run_state_postcard" },
  "data": {
    "run_state": {
      "seed": 123456789,
      "wave_index": 17,
      "score": 42000,
      "lives": 1,
      "rng_state": { "…": "…" },
      "sim_state": { "…": "…" }
    }
  }
}
```

**Rules**
- Reject if:
  - `schema_version` unsupported
  - integrity digest invalid
  - `tuning.hash` mismatch (unless a migration explicitly allows)
  - invariants fail (NaN/Inf, duplicate IDs, invalid geometry, etc.)
- On load failure:
  - fallback to `.bak`
  - if both fail: disable Continue + offer Delete Save

### 20.2 Determinism Hash (State Digest)
- Compute at fixed checkpoints (e.g., end-of-wave, every 10 seconds, or every 600 ticks):
  - Quantize floats to stable representation (e.g., fixed-point or IEEE bytes after clamping)
  - Serialize fields in stable order
  - Hash using `blake3` (fast + stable)
- Include:
  - Ball positions/velocities
  - Paddle position
  - Block states + HP
  - Active power-ups + timers
  - RNG state/counter
  - Score/wave counters
  - Phase state (Serve/Playing/Breather)

### 20.3 Tuning File (`tuning.ron`) Guidelines
- All gameplay-impacting constants live here:
  - Ball speed ramps, max speed, paddle size curve
  - Serve attached offset and launch vector tuning (tangential cap, etc.)
  - Block HP tables, spawn weights, special effects magnitudes
  - Power-up durations and caps
  - Fairness constraints (coverage caps, retry limits)
  - **`breather_phase_seconds = 5.0`** (new required tuning field; default 5.0)

- Any change updates `tuning_hash`, invalidating old saves unless migrated.

### 20.4 Fairness Constraints (Reference)
- **Safe lane rule:** guarantee one contiguous angular segment free of lethal hazards at spawn time.
- **Density cap:** max fraction of "high-threat" blocks per ring/sector.
- **Unavoidable combo ban list:** disallow specific adjacency patterns known to cause trap states (maintained as patterns).
- **Retry loop:** generator attempts up to N times, then emits fallback wave.

### 20.5 Accessibility Toggles (Minimum Set)
- Reduced Motion:
  - Disable/attenuate screen shake
  - Lower particle counts and velocities
  - Reduce distortion intensity
- Reduced Flashing:
  - Clamp bloom spikes
  - Limit rapid full-screen flashes
- High Contrast:
  - Strengthen ball outline and hazard boundaries
  - Increase HUD contrast
- UI Scale:
  - 80% / 100% / 120% / 140% presets

---

## 21) Final Acceptance Criteria (MVP)

- Can start Endless run, complete multiple waves, and reach game over without errors.
- **Serve/Launch** exists and is clear:
  - Ball starts attached to paddle
  - Launch on click/tap (or Space/Enter)
- Pause works (manual + focus lost) and returns to identical sim state.
- Continue works across reloads with validated save; corruption handling behaves as specified.
- At least:
  - 2 block types (Glass/Armored) and 1 hazard (Black Hole) in MVP scope
  - If Armored is delayed, Glass-only is acceptable for MVP 1 as long as waves progress and difficulty ramps via density/speed.
- Determinism tests pass in CI with golden replays.
- Mobile touch controls usable; HUD respects safe areas.
- Performance preset "Low" is playable on mobile; "Medium" is playable on desktop.

---

## 22) Post-MVP Backlog (Not Required for MVP)

### 22.1 Gameplay Expansion
- Additional block types:
  - **Splitter** (spawns 2 weaker balls on break)
  - **Shielded** (requires hit from behind / angle condition)
  - **Regenerator** (restores neighboring block HP slowly)
- Additional hazards:
  - **Laser Sweep** (telegraphed arc sweep)
  - **Mine** (delayed detonation with clear fuse)
- Power-ups (expanded set):
  - Multi-ball (capped, with fairness safeguards)
  - Magnet paddle (temporary catch + release)
  - Piercing ball (limited hits)
  - Time slow (bounded; must not break determinism)
- Meta progression (optional):
  - Unlock cosmetic palettes only (avoid gameplay-affecting upgrades for deterministic fairness)

### 22.2 Modes & Content
- Daily seed runs (same seed for all users, local leaderboard later)
- Wave "biomes" (visual + rule variations; must still obey fairness constraints)
- Challenge modifiers (opt-in):
  - Low paddle size
  - Increased hazard density
  - Limited continues

### 22.3 UX / Social
- Replay export/import:
  - Share **seed + input stream + tuning hash**
  - Viewer mode with speed controls + scrub checkpoints
- Ghost playback (rendered from replay)
- Photo mode / GIF capture (non-essential)

### 22.4 Tech / Platform
- Service Worker offline support (cache assets + wasm)
- iOS Safari optimization pass (audio unlock, memory, touch latency)

---

## 23) Non-Goals (Explicit)

- No multiplayer/co-op for MVP.
- No networked leaderboards for MVP.
- No real-money monetization or ads.
- No server-side authoritative simulation (client-only).
- No procedural "infinite art" system beyond minimal palette/FX variations.
- No accessibility certification claim; only the minimum toggles described above.
- **No fallback renderer (WebGL2) now or for MVP**. WebGPU is required.

---

## 24) Risks & Mitigations

### 24.1 Determinism Drift
**Risk:** Floating-point variance, platform timing differences, RNG misuse.
**Mitigation:**
- Fixed timestep only; no "delta time" in gameplay code.
- Deterministic RNG with explicit counter/state in save/replay.
- Hash checkpoints + golden replay tests in CI.
- Avoid non-deterministic GPU compute affecting gameplay (GPU for rendering only).

### 24.2 Generator Fairness / Unwinnable Waves
**Risk:** Procedural content creates trap states or sudden difficulty spikes.
**Mitigation:**
- Enforce constraints (safe lane, density caps, ban patterns).
- Retry loop + fallback wave.
- Telemetry (local dev) for failure rates per wave archetype.
- Playtest scripts that brute-run seeds and flag spikes.

### 24.3 Mobile Performance / Thermal Throttling
**Risk:** Particle-heavy FX and high resolution drop below target FPS.
**Mitigation:**
- Aggressive presets; cap particles; dynamic resolution.
- Budgeted post-processing (or off by default on Low).
- Short sessions by design (breather phases + pause/resume).

### 24.4 Web Audio Restrictions
**Risk:** Audio cannot start until user gesture; inconsistent on iOS.
**Mitigation:**
- "Tap to Start" gate.
- Lazy-load audio assets; provide silent fallback.
- Centralized audio unlock + retries on resume.

### 24.5 Save Corruption / Partial Writes / Quota Failures
**Risk:** Sudden tab close during save write; LocalStorage quota; bad migrations.
**Mitigation:**
- Temp staging + backup rotation; integrity digest verification.
- Invariant validation + deterministic migration chain with fixtures.
- Clear recovery UX: Continue disabled with reason + Delete Save.

### 24.6 Input Complexity (Touch + Mouse + Keyboard)
**Risk:** Control schemes feel inconsistent or "floaty."
**Mitigation:**
- Single abstraction layer for "desired paddle angle."
- Sensible deadzones and smoothing tuned per input type.
- Touch direct positioning option; optionally "relative drag" toggle post-MVP.
- Separate tap-to-launch from drag-to-aim (cooldown window / threshold / explicit button).

---

## 25) MVP Milestones (Suggested)

> Adjust dates to team size; below assumes 1-2 engineers + 1 designer part-time.

### M0 - Foundations (Week 1-2)
- Fixed timestep loop + pause/focus handling
- Input abstraction (mouse/keyboard/touch) + tick-quantization
- Basic arena + paddle + ball physics (deterministic)
- Serve/Launch prototype (attached ball; launch events)
- Minimal UI shell (title, start, pause, game over)
- Save envelope + integrity digest scaffolding

### M1 - Core Loop (Week 3-4)
- Wave system + simplified generator (rings/layers) with fairness constraints
- 1-2 block types (Glass required; Armored optional if schedule allows)
- Black hole hazard tuned + telegraph boundary clarity
- Scoring + lives + respawn to Serve
- Implement **5-second between-wave breather phase**
- Save/load v1 + backup + Continue UI

### M2 - Persistence & Determinism (Week 5-6)
- Save validation + corruption fallback behaviors complete
- Fixture-based load tests + corruption fuzz tests in CI
- Replay format + golden replays + CI determinism tests
- Tuning file pipeline + tuning hash enforcement

### M3 - Rendering/FX + Accessibility (Week 7-8)
- Basic juice pass (particles, shake, bloom) behind toggles
- Reduced motion/flashing/high contrast/UI scale implemented
- Performance presets (Low/Medium/High) + baseline profiling

### M4 - Polish & Release Hardening (Week 9-10)
- Bugfix + tuning
- Tutorial/How to Play update (must include Serve/Launch)
- Release checklist completion + smoke tests across browsers/devices

---

## 26) Metrics (MVP Validation)

### 26.1 Functional
- Crash-free session rate (local QA): target 100% across smoke test matrix
- Determinism: 100% pass rate for golden replays on CI platforms
- Save reliability:
  - Compatible save load success ≥ 99%
  - Corrupt save recovery success (via backup) ≥ 95% in simulated corruption tests (where backup remains intact)
- Serve/Launch:
  - ≥ 95% successful launches in tutorial/onboarding sessions without assistance

### 26.2 Performance
- Median FPS:
  - Mobile Low: ≥ 45 FPS typical waves; worst-case ≥ 30 FPS
  - Desktop Medium: ≥ 60 FPS typical; worst-case ≥ 50 FPS
- Frame-time spikes: < 2 spikes > 50ms per 5-minute run (desktop baseline)

### 26.3 Gameplay Feel (Qualitative + Lightweight Quantitative)
- Average session length: 3-10 minutes typical for first-time users
- Early churn: tutorial completion rate target ≥ 70% in playtests
- "Unfair death" reports: declining trend over tuning iterations

---

## 27) Test Matrix (Minimum)

### Browsers
- Chrome (latest stable) - Windows + Android
- Firefox (latest stable) - Windows
- Safari (latest stable) - macOS + iOS (if supported target)
- Edge (latest stable) - Windows

### Devices (Representative)
- Desktop:
  - Integrated GPU laptop
  - Mid-range discrete GPU
- Mobile:
  - Recent Android mid-range
  - Older Android device (performance floor)
  - iPhone baseline target (if iOS supported)

### Scenarios
- Start → Serve → launch → play 10 waves → verify each wave-clear produces a **5-second breather** → next wave Serve → launch again → pause → resume → game over
- Save on wave boundary → reload tab → continue
- Corrupt save → backup loads
- Corrupt both → Continue disabled + Delete Save + New Run works
- Toggle accessibility options mid-run
- Offline (if no network dependency) start + play

---

## 28) Glossary

- **Wave:** A single procedurally generated layout + hazard configuration that ends when all blocks are cleared (or another defined completion condition).
- **Breather Phase:** The **mandatory 5-second** between-wave rest period that is non-lethal and shows the transition UI.
- **Breather Wave:** A lower-intensity procedurally generated wave used for pacing (separate concept from the breather phase).
- **Serve:** Pre-play state with the ball attached to the paddle awaiting launch input.
- **Determinism:** Given identical seed + inputs + tuning, the simulation produces identical outcomes across runs.
- **Golden Replay:** A recorded input stream with expected determinism hashes at checkpoints used in CI.
- **Tuning Hash:** Hash of gameplay constants; used to invalidate incompatible saves/replays.
- **Safe Lane Rule:** Generator guarantee that an initial survivable path exists at spawn.
- **Threat Density:** Proportion of high-danger elements in a wave (hazards + high-HP blocks, etc.).

---

## 29) Decision Log (To Fill During Development)

- Renderer decision (WebGPU-only vs fallback): **Decided: WebGPU-only; no fallback**
- Audio scope (SFX only vs include music): **TBD**
- Breather cadence rule (fixed vs adaptive): **TBD**
- Breather phase duration: **Decided for MVP: 5.0 seconds**
- Serve/Launch: **Decided: required**
- Save cadence (wave boundary only vs mid-wave): **Decided for MVP:** boundary + pause + focus-loss; mid-wave optional post-MVP
- Replay exposure (internal only vs share feature): **TBD**

---

## 30) Appendix Addendum (Formats)

### 30.1 Replay File (Proposed)
```json
{
  "version": 1,
  "tuning_hash": "blake3:…",
  "seed": 123456789,
  "inputs": [
    { "tick": 0, "type": "move", "x": 0.12 },
    { "tick": 10, "type": "launch", "pressed": true },
    { "tick": 15, "type": "move", "x": 0.18 },
    { "tick": 120, "type": "pause", "state": true }
  ],
  "checkpoints": [
    { "tick": 600, "digest": "blake3:…" },
    { "tick": 1200, "digest": "blake3:…" }
  ]
}
```

**Notes**
- Inputs are sparse events; simulation samples "current desired angle" each tick.
- Launch is a tick-quantized event.
- Checkpoints must match CI-computed digests exactly.

### 30.2 Settings Storage (Proposed)
- `settings.json` (or `localStorage` equivalent on web) includes:
  - accessibility toggles
  - volume levels
  - performance preset
  - UI scale
- Settings must not affect determinism hashes (render-only).

---

**End of PRD (MVP)**

---

# Post-MVP Addendum (v1.1+)

## 31) Post-MVP Roadmap (Candidates)

> Not committed for MVP. Prioritize based on MVP metrics + playtest feedback.

### 31.1 Gameplay & Content
- **Adaptive breather cadence**
  - Dynamic breather insertion based on recent damage taken, near-miss rate, or difficulty slope.
- **New block archetypes**
  - Shielded blocks (directional resistance)
  - Split blocks (spawned smaller blocks)
  - "Objective" blocks (must be cleared last / protected)
- **New hazards / modifiers**
  - Wind/drag zones
  - Temporary low-gravity wave modifier
  - "Dark" waves (reduced visibility; accessibility-safe toggle)
- **Meta progression (lightweight)**
  - Unlockable cosmetics or optional challenge mutators
  - Avoid power creep that invalidates baseline tuning
- **Daily seed / challenge runs**
  - Fixed daily seed with leaderboard hooks (if online is added)

### 31.2 UX & Accessibility
- **Full remapping** (keyboard + gamepad), including deadzone tuning and axis inversion
- **Expanded colorblind support**
  - Additional palettes + patterns/textures on critical elements
- **Advanced tutorial**
  - Optional training room for mechanics + hazard practice

### 31.3 Tech / Platform
- **Replay sharing**
  - Export/import replay JSON + deterministic validation on load
- **Cloud save (optional)**
  - If accounts are added; otherwise remain local-only
- **Performance deep dives**
  - Automated perf regression tests; per-wave perf telemetry in dev builds

---

## 32) Non-Goals (Explicit)

- Multiplayer / co-op
- Real-money monetization
- Account system (unless required for a later leaderboard feature)
- Procedural generation that requires network connectivity
- Mid-wave save as a requirement for MVP (allowed as later enhancement)
- **WebGL2 fallback renderer** (product is WebGPU-only unless this PRD is explicitly revised)

---

## 33) Risks & Mitigations

### 33.1 Determinism Risk (Cross-browser)
- **Risk:** Floating-point variance or timing differences break replays.
- **Mitigations:**
  - Fixed-tick simulation; never use real-time delta for gameplay
  - Avoid non-deterministic APIs (e.g., `Math.random()` without seeded PRNG)
  - CI golden replay checks on at least 2 engines (Chromium + Firefox)

### 33.2 Performance Risk (Mobile)
- **Risk:** Fill-rate and overdraw spikes on older devices.
- **Mitigations:**
  - Strict particle budget + pooled systems
  - Low preset disables expensive post effects
  - Simple fallback materials; cap dynamic lights (if any)

### 33.3 Save Corruption / Compatibility
- **Risk:** Players lose runs after updates or storage issues.
- **Mitigations:**
  - Versioned saves + tuning hash gates
  - Always keep last-known-good backup
  - "Recover from backup" UI path and "Delete save" escape hatch

### 33.4 Generator Fairness / "Unfair Death"
- **Risk:** Procgen produces unavoidable spawns or sudden spikes.
- **Mitigations:**
  - Safe Lane Rule + spawn-protection window
  - Threat density clamps + spike smoothing across waves
  - Automated seed sweeps in CI with failure heuristics (e.g., damage within first N seconds)

### 33.5 Scope Creep
- **Risk:** Adding content/features delays release hardening.
- **Mitigations:**
  - Milestone gates: M4 only bugfix/tuning/docs
  - Post-MVP bucket (Section 31) is the only place new feature requests go by default

---

## 34) Release Readiness (Definition of Done)

### 34.1 Must-Pass
- All scenarios in Test Matrix (Section 27) pass on each target browser/device class
- Golden replay determinism passes on CI for all supported browsers
- No P0/P1 bugs open (define severity in tracker)
- Tutorial completion flow has no blockers; first run ends with clear next action (continue/new run)

### 34.2 Should-Pass
- Performance meets or exceeds targets in Section 26.2 on representative hardware
- Accessibility toggles verified with at least one external user or structured internal review

---

## 35) Document Change Log
- v0.1 - Initial MVP PRD drafted
- v0.2 - Added determinism + save recovery requirements
- v0.3 - Added metrics + test matrix + file format proposals
- v1.0 - MVP PRD finalized for implementation kickoff
- v1.1 - Updated: **5-second between-wave breather phase**; decided **WebGPU-only (no fallback)**
- v1.2 - Updated: **Serve/Launch required** (ball attached to paddle; click/tap to launch)

---

**End of Document (Complete)**

---

## 36) Appendices

### 36.1 Glossary
- **Wave:** A discrete segment of play with defined spawn pattern(s), duration, and transition rules.
- **Seed:** 32-bit (or 64-bit) value that fully determines procgen output for a run.
- **Fixed-tick simulation:** Game updates in constant time steps (e.g., 60 ticks/sec) independent of rendering frame rate.
- **Golden replay:** A recorded input stream + initial state used to verify determinism and regression stability across builds/browsers.
- **Threat density:** Normalized measure of on-screen danger (projectiles, collision surfaces, hazard zones) used to clamp procgen spikes.
- **Near-miss:** Player passes within a defined distance of a hazard without taking damage; used for difficulty pacing/telemetry.
- **Breathers:** Low-intensity interval to reset cognitive load; may include reduced spawns or slower patterns.
- **Tuning hash:** Hash of critical balance parameters embedded into save/replay metadata to prevent mismatched playback.

### 36.2 Severity Definitions (Bug Tracker)
- **P0 (Blocker):** Crashes, save loss, non-launching, or determinism broken on supported platforms.
- **P1 (Critical):** Major gameplay break (unavoidable deaths from generator, input failures, softlocks, tutorial blockers).
- **P2 (Major):** Noticeable defects affecting experience but with workaround (UI layout issues, rare spawn anomalies).
- **P3 (Minor):** Cosmetic or low-impact issues (minor visual glitches, copy tweaks).
- **P4 (Trivial):** Nice-to-have polish, extremely low priority.

### 36.3 Target Browsers / Platforms (MVP Support Statement)
- **Desktop:** Latest stable Chrome/Edge, Firefox, Safari (macOS) where WebGPU is supported/enabled.
- **Mobile:** iOS Safari (recent iOS) and Chrome on Android (recent Android) where WebGPU is supported/enabled.
- **Not guaranteed:** Browsers/devices without WebGPU, embedded webviews, older Safari versions, legacy Android browsers.
- **Controllers:** XInput-compatible on desktop; mobile controller support best-effort if browser provides standard Gamepad API.

### 36.4 Accessibility Checklist (MVP)
- **Input**
  - Fully remappable controls (KB + gamepad) *(post-MVP if too large; MVP must at least support KB navigation in menus)*
  - Toggle hold vs. tap (where applicable)
  - Adjustable deadzones + axis inversion
- **Visual**
  - Colorblind palettes + patterns on critical affordances
  - Reduced motion toggle (limits screen shake, high-frequency flashes)
  - Visibility aids (contrast slider, optional outlines)
- **Audio**
  - Separate sliders (SFX/Music/UI)
  - Critical cues have visual equivalents
- **Cognitive**
  - Breathers present in early/mid loops
  - Clear telegraphs; avoid overlapping identical cues without distinction
  - Tutorial skippable, but accessible via menu at any time

---

## 37) Data & Format Specifications (MVP)

### 37.1 Save File (Local) - JSON (Versioned)
**Filename:** `save_v{n}.json` (stored in localStorage/IndexedDB per implementation)

**Top-level fields (example)**
```json
{
  "version": 1,
  "createdAt": 1730000000,
  "lastPlayedAt": 1730001111,
  "profile": {
    "settings": {
      "audio": { "master": 0.9, "sfx": 0.9, "music": 0.7, "ui": 0.8 },
      "accessibility": { "reducedMotion": false, "colorblindMode": "deuteranopia" },
      "input": { "kb": {}, "gp": {}, "deadzone": 0.18, "invertY": false }
    }
  },
  "progress": {
    "bestScore": 123456,
    "bestWave": 42,
    "unlocks": { "cosmetics": [], "mutators": [] }
  },
  "integrity": {
    "tuningHash": "abc123",
    "checksum": "optional"
  }
}
```

**Rules**
- Must tolerate unknown fields (forward compatibility).
- If `tuningHash` mismatch after update: keep file but warn and disable incompatible "continue" states.
- Maintain `backup` copy automatically on each successful write.

### 37.2 Run Snapshot (Optional, Not Required for MVP)
A "continue run" snapshot is allowed but not required (per Non-Goals). If implemented later:
- Store minimal deterministic state only (tick index, RNG state, player state, wave generator state, active hazards).
- Never store raw floating-point transforms without quantization rules.

### 37.3 Replay Format - JSON (Deterministic Input Log)
**Purpose:** Debugging + shareable "proof" runs.

**Example**
```json
{
  "version": 1,
  "gameVersion": "1.0.0",
  "tuningHash": "abc123",
  "seed": 987654321,
  "startTick": 0,
  "endTick": 184200,
  "inputs": [
    { "t": 0, "a": "move", "x": 0, "y": 1 },
    { "t": 10, "a": "launch", "v": 1 },
    { "t": 15, "a": "launch", "v": 0 }
  ],
  "meta": {
    "platform": "web",
    "browser": "Chromium",
    "notes": ""
  }
}
```

**Constraints**
- Input events are tick-indexed (`t`), not timestamped.
- Inputs are canonicalized (e.g., stick values quantized to a fixed step).
- On load: validate `tuningHash` and reject playback if mismatch (or offer "best effort" debug mode in dev builds only).

---

## 38) Telemetry / Metrics Appendix (Dev + Optional Release)

### 38.1 Event Dictionary (Minimal)
If telemetry is included (offline local logs OK), events must be:
- **Anonymous** (no PII), **opt-out**, and **rate-limited**.

**Proposed events**
- `run_start` - seed, difficulty, build, device class
- `run_end` - duration, waves cleared, score, cause_of_death
- `serve_launch` - time-to-launch, input method (coarse)
- `damage_taken` - tick, amount, source archetype
- `near_miss` - tick, hazard type, distance bucket
- `settings_changed` - category only (no raw keybinds if privacy-sensitive)
- `tutorial_step_complete` - step id, time-to-complete

### 38.2 Aggregations (What We Actually Use)
- Wave completion distribution
- Death causes by wave band (1-10, 11-20, etc.)
- Average FPS by device class and preset
- Input device usage (% keyboard vs. touch)
- Serve launch success rate and time-to-launch (if enabled)
- Tutorial completion funnel + drop-off step

---

## 39) Determinism Engineering Notes (Implementation Guidance)

### 39.1 RNG Requirements
- Single authoritative PRNG implementation (seeded).
- No usage of native `Math.random()` in gameplay code.
- PRNG state must be:
  - Serializable
  - Advanced only in deterministic order (no conditional "extra draws" tied to rendering)

### 39.2 Physics / Collision Guidance
- Prefer discrete collision checks with integer/quantized positions where possible.
- If floats are required:
  - Quantize key state at fixed points (e.g., after integration per tick).
  - Avoid trig-heavy drift without normalization.
- All collision resolution must be stable given identical inputs and initial state.

### 39.3 Time & Scheduling
- One simulation clock: `tick`.
- Rendering can interpolate between ticks but must never feed back into simulation.
- Avoid browser timing callbacks that mutate simulation out of order.

---

## 40) Open Questions (To Resolve During Implementation Kickoff)
- **Mobile thermal strategy:** Do we enforce 30 FPS mode on sustained thermal throttling?
- **Leaderboard stance:** Fully offline MVP; define whether daily seed includes "share code" only.
- **Audio pipeline:** WebAudio-only vs. hybrid HTMLAudio fallback for Safari edge cases.
- **Controller UX:** Do we ship an in-game button glyph system for Xbox/PlayStation/Switch layouts?

---

## 41) Final Sign-Off Checklist (Kickoff Gate)
- [ ] MVP feature list (Section 5/6) locked
- [ ] Non-goals reaffirmed (Section 32)
- [ ] Performance budgets approved (Section 26)
- [ ] Determinism approach approved + golden replay plan in CI (Section 33/39)
- [ ] Accessibility baseline accepted (Section 31.2 / 36.4)
- [ ] Test matrix agreed + owners assigned (Section 27)
- [ ] Release DoD accepted (Section 34)

---

**End of Document (Complete)**

---

## 42) Glossary / Definitions

- **Tick** - Fixed simulation step (e.g., 60 ticks/sec) that advances gameplay state deterministically.
- **Run** - One complete play session from start to death/exit; defined by a seed + tuningHash + input stream.
- **Seed** - Integer used to initialize the PRNG for wave generation and any randomized gameplay systems.
- **tuningHash** - Hash of all gameplay-affecting parameters (enemy stats, wave tables, physics constants). Used to ensure replays/snapshots remain comparable.
- **Golden replay** - A canonical input log used in CI to detect determinism regressions.
- **Near miss** - A metric event indicating a hazard passed within a defined proximity threshold.
- **Device class** - Coarse categorization (desktop/laptop/tablet/phone) used for performance aggregation without collecting PII.
- **Preset** - A user-selectable configuration bundle (graphics/audio/accessibility) with known performance characteristics.
- **Best effort replay** - Dev-only mode allowing playback with mismatched tuningHash for debugging only; not valid for proofs/leaderboards.

---

## 43) Reference Data (Non-Normative)

### 43.1 Recommended Quantization Defaults
These are defaults; final values to be validated in tuning/playtests.
- Stick axis quantization step: `0.05` (i.e., values snapped to increments of 0.05)
- Angle quantization (if using angles): `1°` steps
- Position quantization (if floats are unavoidable): `1/1024` world units (or equivalent fixed-point)
- Velocity quantization: `1/1024` world units per tick
- Replay input compaction: delta-encode ticks; store only changes (optional)

### 43.2 Suggested Determinism "No-Fly" List
Avoid in gameplay simulation:
- Non-deterministic iteration order over hash maps/objects
- Time-based APIs (`Date.now`, `performance.now`) affecting simulation
- Frame-time dependent integrations
- Platform-dependent floating point behaviors without quantization
- Asynchronous callbacks mutating authoritative state out of tick order

---

## 44) Document Control

### 44.1 Ownership
- **Product Owner:** TBD
- **Design:** TBD
- **Engineering Lead:** TBD
- **QA Lead:** TBD
- **Audio:** TBD
- **Accessibility Reviewer:** TBD

### 44.2 Change Log
- **v0.1** - Initial PRD draft
- **v0.2** - Added determinism, replay format, save schema guidance
- **v0.3** - Added telemetry appendix + sign-off checklist
- **v0.4** - Added 5s breather requirement
- **v0.5** - Added Serve/Launch requirement

### 44.3 Dependencies / External References (Optional)
- WCAG 2.1 AA baseline (accessibility targets)
- Platform controller mapping references (standard gamepad layouts)
- WebAudio implementation notes per target browsers (if web build)

---

**End of Document (Complete)**

---

## 45) Telemetry Event Dictionary (Normative)

> Purpose: enable performance + balance tuning while maintaining privacy. Telemetry must follow constraints in Sections 36.1 (privacy) and 36.2 (opt-out).

### 45.1 Common Envelope (All Events)
All telemetry events share:
- `eventName` (string)
- `tsClient` (number) - client timestamp (ms). Must not be used for simulation.
- `buildId` (string) - build/version identifier
- `platform` (enum) - `web`, `win`, `mac`, `linux`, `ios`, `android` (as applicable)
- `deviceClass` (enum) - per Glossary
- `sessionId` (string) - random per app launch; not stable across reinstalls
- `runId` (string) - random per run
- `seed` (int) - run seed
- `tuningHash` (string) - see Glossary
- `optInTelemetry` (bool)

**Prohibited in payloads:**
- IP address storage (server logs exempt but must not be joined)
- Full user agent strings (use coarse platform + deviceClass)
- Precise location, contacts, advertising identifiers
- Freeform text

### 45.2 Core Events (MVP)
#### `app_start`
- When: on app launch / first scene ready
- Fields: `coldStart` (bool), `locale` (string, coarse e.g., `en-US`), `graphicsPreset` (string), `audioPreset` (string)

#### `run_start`
- When: run begins (first tick of gameplay after launch)
- Fields: `difficulty` (enum/string), `mode` (string), `startingLoadout` (string id)

#### `serve_launch`
- When: player launches from Serve
- Fields: `waveIndex` (int), `timeInServeTicks` (int), `inputMethod` (enum: `mouse`, `keyboard`, `touch`, `unknown`)

#### `run_end`
- When: run ends (death/quit/abort)
- Fields:
  - `endReason` (enum) - `death`, `quit_to_menu`, `app_backgrounded_timeout`, `crash_recovered`
  - `durationTicks` (int)
  - `score` (int)
  - `waveReached` (int)
  - `kills` (int)
  - `damageTaken` (int)
  - `nearMissCount` (int)
  - `avgFpsBucket` (enum) - `>=60`, `45-59`, `30-44`, `<30`
  - `thermalDownclocked` (bool, if detectable)
  - `determinismFlag` (enum) - `ok`, `desync_detected`, `not_checked`

#### `wave_start`
- When: wave begins (after breather + serve launch)
- Fields: `waveIndex` (int), `enemySetId` (string), `budget` (int)

#### `wave_complete`
- When: wave ends successfully
- Fields: `waveIndex` (int), `timeTicks` (int), `damageTakenThisWave` (int)

#### `upgrade_chosen`
- When: player selects an upgrade/perk (post-MVP if applicable)
- Fields: `waveIndex` (int), `upgradeId` (string), `choiceIndex` (int), `offeredUpgradeIds` (string[])

#### `perf_sample`
- When: periodic (e.g., every 10 seconds) during gameplay; may be disabled when opt-out
- Fields: `fpsAvg` (number), `fpsP95` (number), `frameTimeP95Ms` (number), `gcPauseP95Ms` (number if measurable), `simTickOverrunRate` (number 0..1)

### 45.3 Error / Reliability Events
#### `error`
- When: caught exceptions that would impact UX
- Fields: `errorCode` (string), `scene` (string), `recoverable` (bool)

#### `desync_report` (Dev/QA builds only unless approved)
- When: replay validation fails
- Fields: `expectedHash` (string), `actualHash` (string), `tick` (int), `replayId` (string)

---

## 46) Replay File Format (Normative)

> Goal: compact, deterministic playback + CI validation.

### 46.1 Replay Identity
- `replayId`: `sha256(seed + tuningHash + inputStreamDigest)` (or equivalent)
- Replays are only comparable if `tuningHash` matches (except "best effort replay" dev-only).

### 46.2 Container (Recommended)
A single binary or JSON+binary blob with:
- Header (fixed)
- Input stream (delta-compressed)
- Optional checkpoints (for fast seek; not required MVP)

### 46.3 Header Fields
- `version` (int) - replay schema version
- `createdAt` (ms) - informational only
- `gameBuildId` (string)
- `seed` (int)
- `tuningHash` (string)
- `tickRate` (int) - e.g., 60 or 120
- `inputQuantization` (object) - e.g., axis step, button encoding
- `initialStateHash` (string) - hash of initial sim state after seeding/spawn
- `finalStateHash` (string) - computed at end of run (optional until completion)
- `durationTicks` (int)

### 46.4 Input Stream Encoding
Per tick, store only changes:
- `dt` (varint) - ticks since previous input event (delta)
- `mask` (bitfield) - which controls changed
- Values:
  - Buttons: bitset
  - Axes: quantized integers (e.g., `-20..20` for step 0.05)

**Rules:**
- If no input changes for long spans, `dt` increases; simulation still advances every tick.
- Inputs are applied at tick boundaries only (see determinism requirements).

### 46.5 Validation Hashing (Golden Replay)
- At minimum: hash authoritative state every `N` ticks (e.g., 60) and compare to expected sequence in CI.
- Hash must include only deterministic fields (no pointers, no allocation ids, no timestamps).

---

## 47) Save Data Schema (Normative)

> Saves must be forward-compatible and resilient to partial writes.

### 47.1 Save Slots
- `profile` (single): settings, accessibility, unlocks, best scores
- `runs` (optional, bounded): last N run summaries + last run replay pointer (if stored)

### 47.2 Schema Requirements
- `schemaVersion` (int)
- `lastMigratedFrom` (int, optional)
- Use atomic write strategy:
  - Write to temp key/file
  - Validate JSON/schema
  - Swap/commit
- If corrupted: fall back to defaults and emit `error` telemetry with `errorCode=SAVE_CORRUPT` (if opted-in).

### 47.3 Profile Fields (MVP)
- `settings`:
  - `graphicsPreset`
  - `audio`: `master`, `music`, `sfx`, `muteWhenUnfocused`
  - `controls`: bindings + sensitivity
  - `accessibility`: colorblind mode, reduced motion, text size, etc.
- `progress`:
  - `unlocks` (string[])
  - `bestScore` (int)
  - `bestWave` (int)
  - `stats` (aggregates): `runsPlayed`, `totalKills`, `totalTimeTicks`

### 47.4 Migration Policy
- Migrations must be deterministic and pure (no network calls).
- Keep migration steps for the last 3 schema versions minimum.

---

## 48) Determinism & Golden Replay Operations (Non-Normative)

### 48.1 Golden Replay Lifecycle
- Store golden replays in repo (or artifact store) with:
  - `replayId`
  - `tuningHash`
  - Expected hash chain (tick checkpoints)
- When gameplay-affecting changes occur:
  - Bump `tuningHash`
  - Regenerate golden replays intentionally via a documented command
  - Require PR approval from engineering lead + design

### 48.2 Debugging a Desync
Recommended workflow:
1. Re-run the same replay locally with verbose logging at the first mismatch tick.
2. Dump state diff for key systems (RNG, positions, velocities, health, spawns, Serve/Launch events).
3. Verify:
   - iteration order stability
   - float quantization points
   - PRNG call counts and ordering
4. Add a minimal new golden replay covering the failing edge case.

---

## 49) Rollout / Milestones (Non-Normative)

### 49.1 Milestone 0 - Foundations (1-2 weeks)
- Deterministic sim loop + tick scheduler
- Input capture + quantization
- PRNG + seeding + tuningHash plumbing
- Basic replay record/playback
- Serve/Launch event wiring

### 49.2 Milestone 1 - MVP Gameplay (3-6 weeks)
- Core movement + collisions
- Ring-based wave spawning
- Scoring + death/end conditions
- UI shell (start, pause, results)
- Accessibility baseline implemented

### 49.3 Milestone 2 - Performance & Polish (2-4 weeks)
- Performance passes to hit budgets (Section 26)
- Graphics presets, reduced motion options
- Audio pass + mix
- Telemetry integration + dashboards (if enabled)

### 49.4 Milestone 3 - Release Hardening (2-3 weeks)
- Full test matrix run (Section 27)
- Golden replay stability in CI
- Crash handling + save corruption recovery
- Store/hosting readiness (GitHub Pages)

---

## 50) Legal / Compliance Notes (Non-Normative)

- Telemetry opt-in/out must be accessible from Settings at any time.
- If targeting regions with consent requirements, default telemetry state must follow legal guidance (TBD).
- If user-generated "share codes" are implemented:
  - Ensure codes contain no PII
  - Document what is encoded (seed + tuningHash + optional run summary)

---

**End of Document (Complete)**