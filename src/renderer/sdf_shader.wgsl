// SDF-based renderer for Roto Pong
// Everything rendered in fragment shader using signed distance fields

// ============================================================================
// UNIFORMS - Fixed sizes for WebGPU compatibility
// ============================================================================

const MAX_BALLS: u32 = 8u;
const MAX_BLOCKS: u32 = 256u;
const MAX_TRAIL: u32 = 32u;
const MAX_PARTICLES: u32 = 256u;

struct Globals {
    resolution: vec2<f32>,   // offset 0
    time: f32,               // offset 8
    arena_radius: f32,       // offset 12
    black_hole_radius: f32,  // offset 16
    ball_count: u32,         // offset 20
    block_count: u32,        // offset 24
    trail_count: u32,        // offset 28
    particle_count: u32,     // offset 32
    _pad1: u32,              // offset 36
    camera_pos: vec2<f32>,   // offset 40 (8-byte aligned)
    camera_zoom: f32,        // offset 48
    _pad2: u32,              // offset 52
}

struct Paddle {
    theta: f32,
    arc_width: f32,
    radius: f32,
    thickness: f32,
}

struct Ball {
    pos: vec2<f32>,
    radius: f32,
    speed: f32,
}

struct Block {
    theta_start: f32,
    theta_end: f32,
    radius: f32,
    thickness: f32,
    kind: u32,
    wobble: f32,
    _pad2: f32,
    _pad3: f32,
}

struct TrailPoint {
    pos: vec2<f32>,
    speed: f32,
    alpha: f32,
}

struct Particle {
    pos: vec2<f32>,
    size: f32,
    life: f32,
    color_u: u32,
    _p1: u32,
    _p2: u32,
    _p3: u32,
}

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var<uniform> paddle: Paddle;
@group(0) @binding(2) var<storage, read> balls: array<Ball, MAX_BALLS>;
@group(0) @binding(3) var<storage, read> blocks: array<Block, MAX_BLOCKS>;
@group(0) @binding(4) var<storage, read> trail: array<TrailPoint, MAX_TRAIL>;
@group(0) @binding(5) var<storage, read> particles: array<Particle, MAX_PARTICLES>;

// ============================================================================
// SDF PRIMITIVES
// ============================================================================

const PI: f32 = 3.14159265359;
const TAU: f32 = 6.28318530718;

fn sdCircle(p: vec2<f32>, r: f32) -> f32 {
    return length(p) - r;
}

fn sdRing(p: vec2<f32>, inner: f32, outer: f32) -> f32 {
    let d = length(p);
    return max(inner - d, d - outer);
}

// Arc SDF - simplified for performance
fn sdArc(p: vec2<f32>, theta_start: f32, theta_end: f32, radius: f32, thickness: f32) -> f32 {
    let r = length(p);
    let r_dist = abs(r - radius) - thickness * 0.5;
    
    // Quick radial reject
    if (r_dist > 20.0) { return r_dist; }
    
    let angle = atan2(p.y, p.x);
    
    // Simple angle-in-arc check
    var a = angle - theta_start;
    a = a - round(a / TAU) * TAU;
    if (a < 0.0) { a += TAU; }
    
    var span = theta_end - theta_start;
    span = span - round(span / TAU) * TAU;
    if (span <= 0.0) { span += TAU; }
    
    if (a <= span) {
        return r_dist;
    }
    
    // Distance to nearest endpoint
    let p1 = vec2<f32>(cos(theta_start), sin(theta_start)) * radius;
    let p2 = vec2<f32>(cos(theta_end), sin(theta_end)) * radius;
    return min(length(p - p1), length(p - p2)) - thickness * 0.5;
}

// ============================================================================
// NOISE & SWIRL HELPERS
// ============================================================================

// Simple hash for noise
fn hash(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453);
}

// 2D noise
fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    
    return mix(
        mix(hash(i + vec2<f32>(0.0, 0.0)), hash(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash(i + vec2<f32>(0.0, 1.0)), hash(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y
    );
}

// Fractal brownian motion
fn fbm(p: vec2<f32>) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var freq = 1.0;
    var pos = p;
    
    for (var i = 0; i < 4; i++) {
        value += amplitude * noise(pos * freq);
        freq *= 2.0;
        amplitude *= 0.5;
    }
    return value;
}

// Swirling black hole effect - simplified
fn blackHoleSwirl(p: vec2<f32>, hole_radius: f32) -> vec3<f32> {
    let r = length(p);
    let disk_inner = hole_radius;
    let disk_outer = hole_radius * 3.0;
    
    // Early exit for most pixels
    if (r < disk_inner || r > disk_outer) {
        return vec3<f32>(0.0, 0.0, 0.0);
    }
    
    let angle = atan2(p.y, p.x);
    let disk_t = (r - disk_inner) / (disk_outer - disk_inner);
    
    // Simple spiral
    let twist = (1.0 - disk_t) * 6.0;
    let spiral_angle = angle * 2.0 - twist - globals.time * 0.5;
    let arm = smoothstep(-0.3, 0.3, sin(spiral_angle));
    
    // Colors
    let hot = vec3<f32>(0.6, 0.35, 0.08);
    let cool = vec3<f32>(0.3, 0.1, 0.5);
    let arm_color = mix(cool, hot, arm);
    
    // Cubic falloff
    let falloff = (1.0 - disk_t) * (1.0 - disk_t) * (1.0 - disk_t);
    return arm_color * falloff * 0.6;
}

// ============================================================================
// COLOR HELPERS
// ============================================================================

fn velocityColor(speed: f32) -> vec3<f32> {
    let t = clamp((speed - 150.0) / 250.0, 0.0, 1.0);
    // Blue -> Cyan -> Green -> Yellow -> Red
    if (t < 0.25) {
        return mix(vec3<f32>(0.2, 0.5, 1.0), vec3<f32>(0.2, 0.9, 0.9), t * 4.0);
    } else if (t < 0.5) {
        return mix(vec3<f32>(0.2, 0.9, 0.9), vec3<f32>(0.2, 0.9, 0.3), (t - 0.25) * 4.0);
    } else if (t < 0.75) {
        return mix(vec3<f32>(0.2, 0.9, 0.3), vec3<f32>(1.0, 0.9, 0.2), (t - 0.5) * 4.0);
    } else {
        return mix(vec3<f32>(1.0, 0.9, 0.2), vec3<f32>(1.0, 0.3, 0.1), (t - 0.75) * 4.0);
    }
}

// ============================================================================
// VERTEX & FRAGMENT SHADERS
// ============================================================================

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    // Fullscreen triangle
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0)
    );
    
    var out: VertexOutput;
    out.position = vec4<f32>(pos[vi], 0.0, 1.0);
    out.uv = pos[vi];
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Convert UV to game coordinates with camera
    let aspect = globals.resolution.x / globals.resolution.y;
    var p = in.uv * globals.arena_radius * 1.1 * globals.camera_zoom;
    if (aspect > 1.0) {
        p.x *= aspect;
    } else {
        p.y /= aspect;
    }
    
    // Apply camera offset (add to center view on camera position)
    p = p + globals.camera_pos;
    
    // p_dist is the camera-transformed position for rendering
    let p_dist = p;
    
    // Start with dark background
    var color = vec3<f32>(0.01, 0.01, 0.03);
    let aa = 2.5; // Anti-aliasing
    
    // Starfield backdrop - 2 independent random layers
    let backdrop_uv = p / 500.0;
    let drift_dir = vec2<f32>(1.0, 0.3);
    
    // Layer 1: far stars (unique offset for different pattern)
    let star_uv1 = backdrop_uv + drift_dir * globals.time * 0.004;
    let star_grid1 = floor(star_uv1 * 150.0);
    let star1 = hash(star_grid1 + vec2<f32>(0.0, 0.0)); // Base pattern
    
    // Layer 2: near stars (different grid size + large offset = completely different pattern)
    let star_uv2 = backdrop_uv * 1.5 + drift_dir * globals.time * 0.01;
    let star_grid2 = floor(star_uv2 * 100.0);
    let star2 = hash(star_grid2 + vec2<f32>(1337.0, 7919.0)); // Offset for different pattern
    
    let star_bright1 = step(0.985, star1) * 0.15;
    let star_bright2 = step(0.98, star2) * 0.1;
    
    // Simple twinkle
    let twinkle1 = sin(globals.time * 1.5 + star1 * 100.0) * 0.15 + 0.85;
    let twinkle2 = sin(globals.time * 1.8 + star2 * 60.0) * 0.12 + 0.88;
    
    color += vec3<f32>(0.9, 0.95, 1.0) * star_bright1 * twinkle1;
    color += vec3<f32>(0.7, 0.85, 1.0) * star_bright2 * twinkle2;
    
    // Simple nebula (single noise sample, no FBM)
    let nebula_uv = backdrop_uv * 0.8 + vec2<f32>(globals.time * 0.003, globals.time * 0.002);
    let nebula = noise(nebula_uv * 2.0) * 0.025;
    color += vec3<f32>(0.12, 0.06, 0.18) * nebula;
    
    // Arena wall
    let wall_d = sdRing(p_dist, globals.arena_radius - 5.0, globals.arena_radius);
    let wall_glow = exp(-max(wall_d, 0.0) * 0.1) * 0.15;
    color += vec3<f32>(0.3, 0.3, 0.5) * wall_glow;
    let wall_mask = 1.0 - smoothstep(-aa, aa, wall_d);
    color = mix(color, vec3<f32>(0.35, 0.35, 0.45), wall_mask);
    
    // Pre-compute shimmer (frame-global, doesn't depend on block)
    let shimmer_phase = fract(globals.time / 22.0);
    var shimmer_value = 0.0;
    if (shimmer_phase < 0.12) {
        let shimmer_active = smoothstep(0.0, 0.02, shimmer_phase) * (1.0 - smoothstep(0.08, 0.12, shimmer_phase));
        let shimmer_slot = floor(globals.time / 22.0);
        let pattern_seed = fract(sin(shimmer_slot * 127.1) * 43758.5453);
        if (pattern_seed > 0.5) {
            let sweep_dir = vec2<f32>(cos(pattern_seed * TAU), sin(pattern_seed * TAU));
            let linear_wave = shimmer_phase * 1200.0 - 400.0;
            shimmer_value = exp(-abs(dot(p_dist, sweep_dir) - linear_wave) / 25.0) * 0.6 * shimmer_active;
        } else {
            let radial_pos = shimmer_phase * 2.5 * TAU - PI;
            var angle_diff = atan2(p_dist.y, p_dist.x) - radial_pos;
            angle_diff = angle_diff - round(angle_diff / TAU) * TAU;
            shimmer_value = exp(-abs(angle_diff) / 0.25) * 0.5 * shimmer_active;
        }
    }
    
    // Blocks - single pass: find closest and store its properties
    var closest_block_idx = -1;
    var closest_block_d = 9999.0;
    var closest_block_kind = 0u;
    var closest_block_radius = 0.0;
    var closest_block_thickness = 0.0;
    var closest_block_wobble = 0.0;
    let block_r = length(p_dist);
    let block_angle = atan2(p_dist.y, p_dist.x);
    
    // Pre-compute closest ball once (for portal metaballs)
    var closest_ball_dist = 9999.0;
    var closest_ball_pos = vec2<f32>(0.0, 0.0);
    for (var j = 0u; j < globals.ball_count && j < MAX_BALLS; j++) {
        let ball = balls[j];
        if (ball.radius <= 0.0) { continue; }
        let ball_dist = length(p_dist - ball.pos);
        if (ball_dist < closest_ball_dist) {
            closest_ball_dist = ball_dist;
            closest_ball_pos = ball.pos;
        }
    }
    
    // Pre-compute portal wobble once
    let wobble = (sin(globals.time * 8.0 + block_angle * 3.0) * 1.5
                + sin(globals.time * 12.0 + block_angle * 5.0 + 1.0) * 0.8
                + sin(globals.time * 5.0 + block_angle * 2.0 + 2.5) * 1.0) * 0.25;
    
    for (var i = 0u; i < globals.block_count && i < MAX_BLOCKS; i++) {
        let b = blocks[i];
        if (b.thickness <= 0.0) { continue; }
        
        // Early radius bounds check - skip if clearly too far
        let r_dist = abs(block_r - b.radius) - b.thickness * 0.5;
        if (r_dist > closest_block_d + 5.0) { continue; } // Can't be closer
        
        var d = sdArc(p_dist, b.theta_start, b.theta_end, b.radius, b.thickness);
        
        // Portal blocks: metaball reach toward nearby balls
        if (b.kind == 4u) {
            if (closest_ball_dist < 80.0) {
                let reach_strength = 1.0 - closest_ball_dist / 80.0;
                let to_ball_dist = length(closest_ball_pos - p_dist);
                d = d - exp(-to_ball_dist * 0.05) * reach_strength * reach_strength * 25.0;
            }
            d += wobble;
        }
        
        // Wobble deformation for Jello blocks only
        if (b.kind == 5u && b.wobble > 0.0) {
            let wobble_freq = 8.0;
            let wobble_amp = b.wobble * 6.0;
            let wave = sin(block_angle * wobble_freq + globals.time * 15.0) * wobble_amp;
            d += wave;
        }
        
        if (d < closest_block_d) {
            closest_block_d = d;
            closest_block_idx = i32(i);
            closest_block_kind = b.kind;
            closest_block_radius = b.radius;
            closest_block_thickness = b.thickness;
            closest_block_wobble = b.wobble;
        }
    }
    
    // Render only the closest block (no overlap stacking)
    if (closest_block_idx >= 0 && closest_block_d < aa * 2.0) {
        // Use stored properties to avoid re-reading block array
        let block_t = clamp((block_r - (closest_block_radius - closest_block_thickness * 0.5)) / closest_block_thickness, 0.0, 1.0);
        
        // Block type colors and properties (use stored kind)
        var inner_color = vec3<f32>(0.2, 0.5, 0.9);
        var outer_color = vec3<f32>(0.4, 0.75, 1.0);
        var stroke_color = vec3<f32>(0.8, 0.95, 1.0);
        var shimmer_color = vec3<f32>(1.0, 1.0, 1.0);
        var emission = 0.12;
        var opacity = 0.75;
        var has_specular = false;
        
        if (closest_block_kind == 0u) { // Glass
            has_specular = true;
            opacity = 0.45;
            emission = 0.15;
        } else if (closest_block_kind == 1u) { // Armored
            inner_color = vec3<f32>(0.4, 0.45, 0.5);
            outer_color = vec3<f32>(0.7, 0.75, 0.8);
            stroke_color = vec3<f32>(0.9, 0.92, 0.95);
            emission = 0.1;
            opacity = 0.85;
        } else if (closest_block_kind == 2u) { // Explosive
            inner_color = vec3<f32>(1.0, 0.2, 0.0);
            outer_color = vec3<f32>(1.0, 0.6, 0.1);
            stroke_color = vec3<f32>(1.0, 0.9, 0.3);
            shimmer_color = vec3<f32>(1.0, 1.0, 0.5);
            emission = 0.35;
            opacity = 0.7;
        } else if (closest_block_kind == 3u) { // Invincible
            inner_color = vec3<f32>(0.8, 0.6, 0.1);
            outer_color = vec3<f32>(1.0, 0.9, 0.3);
            stroke_color = vec3<f32>(1.0, 1.0, 0.8);
            shimmer_color = vec3<f32>(1.0, 1.0, 0.9);
            emission = 0.25;
            opacity = 0.9;
        } else if (closest_block_kind == 4u) { // Portal
            inner_color = vec3<f32>(0.0, 0.4, 0.5);
            outer_color = vec3<f32>(0.1, 0.8, 0.7);
            stroke_color = vec3<f32>(0.3, 1.0, 0.9);
            shimmer_color = vec3<f32>(0.5, 1.0, 1.0);
            emission = 0.3;
            opacity = 0.55;
            has_specular = true;
        } else if (closest_block_kind == 5u) { // Jello - lime green, wobbly
            // Pulse color based on wobble intensity
            let wobble_pulse = closest_block_wobble * 0.3;
            inner_color = vec3<f32>(0.2 + wobble_pulse, 0.8, 0.1);
            outer_color = vec3<f32>(0.4 + wobble_pulse, 1.0, 0.3);
            stroke_color = vec3<f32>(0.6, 1.0, 0.5);
            shimmer_color = vec3<f32>(0.8, 1.0, 0.6);
            emission = 0.2 + closest_block_wobble * 0.3;
            opacity = 0.6;
            has_specular = true;
        }
        
        let block_color = mix(inner_color, outer_color, block_t);
        
        // Subtle outer glow
        let glow = exp(-max(closest_block_d, 0.0) * 0.2) * emission;
        color += block_color * glow * 0.3;
        
        // Block fill
        let mask = 1.0 - smoothstep(-aa, aa, closest_block_d);
        var shimmered_color = block_color + shimmer_color * shimmer_value;
        
        // Specular highlight for glass-like blocks
        if (has_specular && block_r > 1.0) {
            // Light from top-right (0.707, 0.707 normalized)
            let to_pixel = p_dist / block_r;
            let spec_angle = to_pixel.x * 0.707 + to_pixel.y * 0.707;
            let specular = max(spec_angle, 0.0) * max(spec_angle, 0.0) * max(spec_angle, 0.0) * max(spec_angle, 0.0) * 0.5; // pow 4 approx
            shimmered_color += vec3<f32>(specular, specular, specular);
        }
        
        // Single blend - no overlap stacking
        color = mix(color, shimmered_color, mask * opacity);
        
        // Stroke only on outer radial edge
        let radial_dist = abs(block_r - closest_block_radius) - closest_block_thickness * 0.5;
        let outer_edge = smoothstep(0.0, 2.0, closest_block_radius - block_r);
        let stroke_d = abs(radial_dist) - 1.0;
        let stroke_mask = 1.0 - smoothstep(-aa * 0.5, aa * 0.5, stroke_d);
        color = mix(color, stroke_color, stroke_mask * mask * outer_edge * 0.6);
    }
    
    // Black hole with swirling accretion disk
    let hole_d = sdCircle(p, globals.black_hole_radius);
    
    // Swirling accretion disk
    let swirl = blackHoleSwirl(p, globals.black_hole_radius);
    color += swirl;
    
    // Event horizon glow (bright ring at the edge)
    let horizon_d = abs(hole_d) - 2.0;
    let pulse = sin(globals.time * 2.0) * 0.15 + 0.85;
    let horizon_glow = exp(-max(horizon_d, 0.0) * 0.4) * 0.6 * pulse;
    color += vec3<f32>(1.0, 0.6, 0.2) * horizon_glow;
    
    // Black hole core (pure black void)
    let hole_mask = 1.0 - smoothstep(-aa, aa * 1.5, hole_d);
    color = mix(color, vec3<f32>(0.0, 0.0, 0.0), hole_mask);
    
    // Trail (after black hole so death spiral is visible)
    for (var i = 0u; i < globals.trail_count && i < MAX_TRAIL; i++) {
        let t = trail[i];
        if (t.alpha <= 0.0) { continue; }
        
        let trail_r = 5.0 * t.alpha;
        let d = sdCircle(p_dist - t.pos, trail_r);
        let trail_color = velocityColor(t.speed);
        let glow = exp(-max(d, 0.0) * 0.15) * t.alpha * 0.5;
        color += trail_color * glow;
    }
    
    // Paddle - draw as a simple thick arc
    // Debug: calculate paddle center position
    let paddle_center = vec2<f32>(cos(paddle.theta), sin(paddle.theta)) * paddle.radius;
    
    // Simple approach: distance to paddle center line, then check angular extent
    let to_p = p;
    let p_angle = atan2(to_p.y, to_p.x);
    let p_radius = length(to_p);
    
    // Angular distance from paddle center
    var angle_diff = p_angle - paddle.theta;
    angle_diff = angle_diff - round(angle_diff / TAU) * TAU;
    
    // Check if within paddle arc and radius
    let in_angle = abs(angle_diff) < paddle.arc_width * 0.5;
    let in_radius = abs(p_radius - paddle.radius) < paddle.thickness * 0.5;
    
    // SDF approximation
    var paddle_d = 9999.0;
    if (in_angle) {
        paddle_d = abs(p_radius - paddle.radius) - paddle.thickness * 0.5;
    } else {
        // Distance to arc endpoints
        let end1 = vec2<f32>(cos(paddle.theta - paddle.arc_width * 0.5), sin(paddle.theta - paddle.arc_width * 0.5)) * paddle.radius;
        let end2 = vec2<f32>(cos(paddle.theta + paddle.arc_width * 0.5), sin(paddle.theta + paddle.arc_width * 0.5)) * paddle.radius;
        paddle_d = min(length(p - end1), length(p - end2)) - paddle.thickness * 0.5;
    }
    
    // Paddle with subtle gradient and glow
    let paddle_pulse = sin(globals.time * 3.0) * 0.05 + 0.95;
    
    // Gradient from cyan (outer) to green (inner)
    let paddle_t = (p_radius - (paddle.radius - paddle.thickness * 0.5)) / paddle.thickness;
    let paddle_inner = vec3<f32>(0.1, 1.0, 0.4);  // Bright green
    let paddle_outer = vec3<f32>(0.2, 0.8, 1.0);  // Cyan
    let paddle_base = mix(paddle_inner, paddle_outer, clamp(paddle_t, 0.0, 1.0));
    
    // Subtle outer glow
    let paddle_glow = exp(-max(paddle_d, 0.0) * 0.25) * 0.15 * paddle_pulse;
    color += vec3<f32>(0.2, 0.9, 0.6) * paddle_glow;
    
    // Stroke (white outline)
    let stroke_width = 1.5;
    let stroke_d = abs(paddle_d) - stroke_width;
    let stroke_mask = 1.0 - smoothstep(-aa * 0.5, aa * 0.5, stroke_d);
    let stroke_color = vec3<f32>(1.0, 1.0, 1.0);
    
    // Core paddle
    let paddle_mask = 1.0 - smoothstep(-aa, aa, paddle_d);
    color = mix(color, paddle_base * paddle_pulse, paddle_mask);
    
    // Apply stroke on top
    color = mix(color, stroke_color, stroke_mask * paddle_mask);
    
    // Balls (always on top, fully opaque)
    for (var i = 0u; i < globals.ball_count && i < MAX_BALLS; i++) {
        let ball = balls[i];
        if (ball.radius <= 0.0) { continue; }
        
        let d = sdCircle(p - ball.pos, ball.radius);
        let ball_color = velocityColor(ball.speed);
        
        // Subtle glow (reduced)
        let glow = exp(-max(d, 0.0) * 0.3) * 0.12;
        color += ball_color * glow;
        
        // Solid ball (fully opaque)
        let mask = 1.0 - smoothstep(-aa, aa, d);
        color = mix(color, ball_color, mask);
        
        // Stroke (white outline)
        let ball_stroke_d = abs(d) - 1.2;
        let ball_stroke_mask = 1.0 - smoothstep(-aa * 0.5, aa * 0.5, ball_stroke_d);
        color = mix(color, vec3<f32>(1.0, 1.0, 1.0), ball_stroke_mask * mask);
    }
    
    // Particles! 🎆 MAKE IT RAIN!
    for (var i = 0u; i < globals.particle_count && i < MAX_PARTICLES; i++) {
        let part = particles[i];
        if (part.life <= 0.0 || part.size <= 0.0) { continue; }
        
        let d = length(p - part.pos) - part.size;
        
        // Color based on block type - BRIGHT and saturated
        var part_color = vec3<f32>(0.5, 0.8, 1.0); // Glass - bright cyan
        if (part.color_u == 1u) { part_color = vec3<f32>(0.85, 0.9, 1.0); } // Armored - bright silver
        else if (part.color_u == 2u) { part_color = vec3<f32>(1.0, 0.6, 0.1); } // Explosive - orange fire
        else if (part.color_u == 3u) { part_color = vec3<f32>(1.0, 0.95, 0.4); } // Invincible - gold
        else if (part.color_u == 4u) { part_color = vec3<f32>(0.3, 1.0, 0.9); } // Portal - teal
        else if (part.color_u == 5u) { part_color = vec3<f32>(0.5, 1.0, 0.4); } // Jello - lime
        
        // BIG outer glow
        let outer_glow = exp(-max(d, 0.0) * 0.15) * part.life * 0.8;
        color += part_color * outer_glow;
        
        // Inner glow (tighter)
        let inner_glow = exp(-max(d, 0.0) * 0.4) * part.life * 0.4;
        color += vec3<f32>(1.0, 1.0, 1.0) * inner_glow;
        
        // Hot white core
        let core_d = d + part.size * 0.5; // Smaller core
        let core_mask = 1.0 - smoothstep(-aa, aa, core_d);
        let core_color = mix(part_color, vec3<f32>(1.0, 1.0, 1.0), 0.7);
        color = mix(color, core_color * (1.0 + part.life * 0.5), core_mask * part.life);
    }
    
    // Vignette
    let vig = 1.0 - length(in.uv) * 0.25;
    color *= vig;
    
    // Tone mapping (simple)
    color = color / (color + vec3<f32>(1.0));
    
    return vec4<f32>(color, 1.0);
}
