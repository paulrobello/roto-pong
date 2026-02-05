//! Fixed timestep simulation tick
//!
//! Core game loop that advances simulation deterministically.

use glam::Vec2;

use super::ball_arc_collision;
use super::state::{BREATHER_DURATION_TICKS, BallState, GamePhase, GameState, Pickup, PickupKind};
use crate::consts::*;
// use crate::{cartesian_to_polar, normalize_angle, polar_to_cartesian};

/// Input commands for a single tick (deterministic)
#[derive(Debug, Clone, Default)]
pub struct TickInput {
    /// Target paddle angle (from mouse/touch position)
    pub target_theta: Option<f32>,
    /// Launch ball (click/tap/space)
    pub launch: bool,
    /// Pause toggle
    pub pause: bool,
    /// Skip to next wave (debug/testing)
    pub skip_wave: bool,
    /// Idle/demo mode - AI plays the game
    pub idle_mode: bool,
}

/// Advance the game state by one fixed timestep
pub fn tick(state: &mut GameState, input: &TickInput, dt: f32) {
    // Handle pause toggle
    if input.pause {
        match state.phase {
            GamePhase::Playing | GamePhase::Serve => {
                state.phase = GamePhase::Paused;
                return;
            }
            GamePhase::Paused => {
                state.phase = if state
                    .balls
                    .iter()
                    .any(|b| matches!(b.state, BallState::Attached { .. }))
                {
                    GamePhase::Serve
                } else {
                    GamePhase::Playing
                };
            }
            _ => {}
        }
    }

    // Don't tick if paused or game over
    match state.phase {
        GamePhase::Paused | GamePhase::GameOver => return,
        _ => {}
    }

    // Decay screen shake
    state.screen_shake *= 0.9; // Fast decay
    if state.screen_shake < 0.01 {
        state.screen_shake = 0.0;
    }

    // Idle/demo mode - AI plays the game
    let mut input = input.clone();
    if input.idle_mode {
        // Auto-launch ball in serve phase
        if matches!(state.phase, GamePhase::Serve) {
            input.launch = true;
        }

        // Find the most dangerous ball (closest to black hole)
        let maybe_ball = state
            .balls
            .iter()
            .filter(|b| matches!(b.state, BallState::Free))
            .min_by(|a, b| {
                a.pos
                    .length()
                    .partial_cmp(&b.pos.length())
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

        // Check if ALL balls are "safe" (far from paddle, moving away)
        let all_balls_safe = state
            .balls
            .iter()
            .filter(|b| matches!(b.state, BallState::Free))
            .all(|ball| {
                let ball_dist = ball.pos.length();
                let moving_outward = ball.vel.dot(ball.pos.normalize_or_zero()) > 0.0;
                // Safe if ball is far out OR moving away from center
                ball_dist > 200.0 || (ball_dist > 100.0 && moving_outward)
            });
        let ball_is_safe = state
            .balls
            .iter()
            .filter(|b| matches!(b.state, BallState::Free))
            .count()
            == 0
            || all_balls_safe;

        // If safe, go grab the nearest pickup
        let target_pickup = if ball_is_safe && !state.pickups.is_empty() {
            state
                .pickups
                .iter()
                .min_by(|a, b| {
                    let dist_a = a.pos.length();
                    let dist_b = b.pos.length();
                    dist_a
                        .partial_cmp(&dist_b)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|p| p.pos.y.atan2(p.pos.x))
        } else {
            None
        };

        if let Some(pickup_angle) = target_pickup {
            // Go get the pickup!
            input.target_theta = Some(pickup_angle);
        } else if let Some(ball) = maybe_ball {
            // Track the ball with some offset to avoid perfect loops
            // Add oscillating offset based on time to create variety
            let time_factor = state.time_ticks as f32 * 0.01;
            let offset = (time_factor.sin() * 0.3) + (time_factor * 0.7).sin() * 0.15;

            // Predict where ball is heading (lead the target slightly)
            let ball_future = ball.pos + ball.vel.normalize_or_zero() * 30.0;
            let future_angle = ball_future.y.atan2(ball_future.x);

            // Blend between current ball angle and predicted, add offset
            let target = future_angle + offset;
            input.target_theta = Some(target);
        }
    }
    let input = &input;

    // Debug: skip to next wave
    if input.skip_wave {
        state.blocks.clear();
        state.balls.clear();
        state.wave_index += 1;
        state.breather_ticks = 0; // Skip breather too
        generate_wave(state);
        state.spawn_ball_attached();
        state.phase = GamePhase::Serve;
        return;
    }

    state.time_ticks += 1;

    // Update paddle position
    if let Some(target) = input.target_theta {
        let max_speed = 9.6; // radians per second (reduced 20%)
        state.paddle.move_toward(target, dt, max_speed);
    }

    match state.phase {
        GamePhase::Serve => {
            // Rotate blocks even before launch
            for block in &mut state.blocks {
                block.rotate(dt);
            }

            // Update attached balls to follow paddle
            for ball in &mut state.balls {
                ball.update_attached(&state.paddle);
            }

            // Keep particles animating during serve
            for particle in state.particles.iter_mut() {
                particle.pos += particle.vel * dt;
                let to_center = -particle.pos.normalize_or_zero();
                particle.vel += to_center * 50.0 * dt;
                particle.vel *= 0.98;
                particle.life -= dt * 1.5;
                particle.size *= 0.995;
            }
            state.particles.retain(|p| p.life > 0.0);

            // Launch on input
            if input.launch {
                for ball in &mut state.balls {
                    if matches!(ball.state, BallState::Attached { .. }) {
                        let speed = BALL_START_SPEED; // TODO: from tuning
                        ball.launch(&state.paddle, speed, 0.5);
                    }
                }
                state.phase = GamePhase::Playing;
            }
        }

        GamePhase::Playing => {
            // Rotate blocks
            for block in &mut state.blocks {
                block.rotate(dt);
            }

            // Update sliding balls (portal traversal)
            let portal_slide_speed = 1.5; // radians per second - slow satisfying slide
            let mut portal_exits: Vec<(usize, u32)> = Vec::new(); // (ball_idx, block_id) for damage

            // Collect portal block info for chaining detection
            let portal_blocks: Vec<_> = state
                .blocks
                .iter()
                .filter(|b| matches!(b.kind, super::state::BlockKind::Portal { .. }))
                .map(|b| (b.id, b.arc.theta_start, b.arc.theta_end, b.arc.radius))
                .collect();

            for (ball_idx, ball) in state.balls.iter_mut().enumerate() {
                if let BallState::Sliding {
                    block_id,
                    ref mut theta,
                    direction,
                    entry_speed,
                    arc_start,
                    arc_end,
                    radius,
                    ref mut total_traveled,
                    max_travel,
                } = ball.state
                {
                    // Move along the arc
                    let move_amount = portal_slide_speed * dt;
                    *theta += direction * move_amount;
                    *total_traveled += move_amount;

                    // Update ball position to be on the arc
                    ball.pos = Vec2::new(theta.cos() * radius, theta.sin() * radius);

                    // Check if we've exceeded our random max travel distance
                    let exceeded_max = *total_traveled >= max_travel;

                    // Check if we've reached the end of current block
                    let exit_theta = if direction > 0.0 { arc_end } else { arc_start };
                    let at_exit = if direction > 0.0 {
                        *theta >= exit_theta
                    } else {
                        *theta <= exit_theta
                    };

                    if exceeded_max {
                        // Force exit - traveled too far!
                        let current_theta = *theta;
                        let exit_r = radius + PADDLE_THICKNESS / 2.0 + ball.radius + 5.0;

                        let tangent =
                            Vec2::new(-current_theta.sin(), current_theta.cos()) * direction;
                        let radial = Vec2::new(current_theta.cos(), current_theta.sin());
                        let exit_dir = (tangent * 0.6 + radial * 0.4).normalize();

                        ball.pos =
                            Vec2::new(current_theta.cos() * exit_r, current_theta.sin() * exit_r);
                        ball.vel = exit_dir * entry_speed * 0.5; // 50% speed on portal exit
                        ball.state = BallState::Free;

                        portal_exits.push((ball_idx, block_id));
                    } else if at_exit {
                        // Check for adjacent portal block to chain into
                        let mut found_chain = false;
                        let current_total = *total_traveled; // Capture before reassigning state

                        for &(next_id, next_start, next_end, next_radius) in &portal_blocks {
                            if next_id == block_id {
                                continue;
                            } // Skip current block

                            // Must be same radius (same ring)
                            if (next_radius - radius).abs() > 5.0 {
                                continue;
                            }

                            // Check if this portal's edge touches our exit point
                            let angle_tolerance = 0.15; // ~8 degrees tolerance for adjacency
                            let touches_start = (next_start - exit_theta).abs() < angle_tolerance
                                || (next_start - exit_theta).abs()
                                    > std::f32::consts::TAU - angle_tolerance;
                            let touches_end = (next_end - exit_theta).abs() < angle_tolerance
                                || (next_end - exit_theta).abs()
                                    > std::f32::consts::TAU - angle_tolerance;

                            if touches_start || touches_end {
                                // Chain into this portal! Damage current block first.
                                portal_exits.push((ball_idx, block_id));

                                // Update sliding state to new portal (preserve total_traveled & max_travel!)
                                ball.state = BallState::Sliding {
                                    block_id: next_id,
                                    theta: if touches_start { next_start } else { next_end },
                                    direction: if touches_start { 1.0 } else { -1.0 },
                                    entry_speed,
                                    arc_start: next_start,
                                    arc_end: next_end,
                                    radius: next_radius,
                                    total_traveled: current_total, // Keep accumulating!
                                    max_travel, // Keep the same random exit point
                                };
                                found_chain = true;
                                break;
                            }
                        }

                        if !found_chain {
                            // No adjacent portal - exit to free movement
                            let exit_r = radius + PADDLE_THICKNESS / 2.0 + ball.radius + 5.0;

                            // Exit tangentially with outward kick
                            let tangent =
                                Vec2::new(-exit_theta.sin(), exit_theta.cos()) * direction;
                            let radial = Vec2::new(exit_theta.cos(), exit_theta.sin());
                            let exit_dir = (tangent * 0.6 + radial * 0.4).normalize();

                            ball.pos =
                                Vec2::new(exit_theta.cos() * exit_r, exit_theta.sin() * exit_r);
                            ball.vel = exit_dir * entry_speed * 0.5; // 50% speed on portal exit
                            ball.state = BallState::Free;

                            // Mark block for damage
                            portal_exits.push((ball_idx, block_id));
                        }
                    }

                    // Record trail while sliding
                    ball.record_trail();
                }
            }

            // Damage portal blocks that balls exited from
            for (_ball_idx, block_id) in portal_exits {
                if let Some(block) = state.blocks.iter_mut().find(|b| b.id == block_id) {
                    block.hp = block.hp.saturating_sub(1);
                    if block.hp == 0 {
                        state.combo += 1;
                    }
                }
            }
            // Remove destroyed portal blocks
            state.blocks.retain(|b| b.hp > 0);

            // Collision detection and response
            let paddle_arc = state.paddle.as_arc();
            let paddle_outer = PADDLE_RADIUS + PADDLE_THICKNESS / 2.0;
            let _paddle_inner = PADDLE_RADIUS - PADDLE_THICKNESS / 2.0;

            // Collect pickups to spawn (deferred to avoid borrow issues)
            let mut pickups_to_spawn: Vec<(PickupKind, Vec2)> = Vec::new();

            for ball in &mut state.balls {
                if !matches!(ball.state, BallState::Free) {
                    continue;
                }

                // Decrement paddle cooldown
                if ball.paddle_cooldown > 0 {
                    ball.paddle_cooldown -= 1;
                }

                // --- BLACK HOLE GRAVITY ---
                // Apply gravitational pull toward center (stronger when closer)
                let dist_to_center = ball.pos.length();
                let to_center = -ball.pos.normalize_or_zero();
                // Inverse distance scaling: much stronger near the hole
                let gravity_multiplier = (200.0 / dist_to_center.max(50.0)).min(4.0);
                ball.vel += to_center * BLACK_HOLE_GRAVITY * gravity_multiplier * dt;

                // Clamp speed to min/max (gravity can slow but not stop the ball)
                let speed = ball.vel.length();
                if speed < BALL_MIN_SPEED {
                    ball.vel = ball.vel.normalize_or_zero() * BALL_MIN_SPEED;
                } else if speed > BALL_MAX_SPEED {
                    ball.vel = ball.vel.normalize_or_zero() * BALL_MAX_SPEED;
                }

                let displacement = ball.vel * dt;
                let old_pos = ball.pos;
                let new_pos = ball.pos + displacement;

                // --- PREDICTIVE PADDLE COLLISION ---
                // Check if trajectory crosses paddle radius BEFORE moving
                if ball.paddle_cooldown == 0 {
                    let old_r = old_pos.length();
                    let new_r = new_pos.length();

                    // Ball moving inward and crossing paddle outer edge?
                    let crossing_outer =
                        old_r > paddle_outer + ball.radius && new_r <= paddle_outer + ball.radius;

                    if crossing_outer {
                        // Calculate where ball will be when it reaches paddle radius
                        let target_r = paddle_outer + ball.radius;
                        // Linear interpolation to find crossing point
                        let t = if (old_r - new_r).abs() > 0.001 {
                            (old_r - target_r) / (old_r - new_r)
                        } else {
                            0.5
                        };
                        let crossing_pos = old_pos + displacement * t.clamp(0.0, 1.0);
                        let crossing_angle = crossing_pos.y.atan2(crossing_pos.x);

                        // Check if crossing point is within paddle arc
                        if paddle_arc.contains_angle(crossing_angle) {
                            // HIT! Reflect at the crossing point
                            let ball_angle = crossing_angle;
                            let paddle_center = state.paddle.theta;
                            let half_arc = state.paddle.arc_width / 2.0;

                            // Normalize hit position: 0 = center, -1/+1 = edges
                            let mut hit_offset = crate::normalize_angle(ball_angle - paddle_center);
                            hit_offset = (hit_offset / half_arc).clamp(-1.0, 1.0);

                            // Normal pointing outward from paddle
                            let normal = Vec2::new(ball_angle.cos(), ball_angle.sin());

                            // Base reflection
                            let base_reflect = super::collision::reflect_velocity(ball.vel, normal);

                            // Add deflection based on hit position
                            let speed = ball.vel.length();
                            let tangent = Vec2::new(-normal.y, normal.x);
                            let deflection = tangent * hit_offset * speed * 0.6;

                            // Also add english from paddle rotation
                            let english = tangent * state.paddle.angular_vel * PADDLE_RADIUS * 0.15;

                            // Apply paddle boost to help escape gravity
                            let boosted_speed = (speed * PADDLE_BOOST).min(BALL_MAX_SPEED);
                            ball.vel =
                                (base_reflect + deflection + english).normalize() * boosted_speed;

                            // Position ball exactly at the reflection point (just outside paddle)
                            let safe_dist = paddle_outer + ball.radius + 1.0;
                            ball.pos = Vec2::new(
                                safe_dist * ball_angle.cos(),
                                safe_dist * ball_angle.sin(),
                            );

                            // Set cooldown to prevent immediate re-collision
                            ball.paddle_cooldown = 8;
                            continue; // Skip normal movement for this ball
                        }
                    }
                }

                // --- POST-PREDICTIVE COLLISION CHECKS ---
                // (Ball will be moved in substeps below)

                // Fallback: discrete paddle collision (catches edge cases)
                if ball.paddle_cooldown == 0 {
                    let paddle_result = ball_arc_collision(ball.pos, ball.radius, &paddle_arc);
                    if paddle_result.hit {
                        let moving_toward = ball.vel.dot(paddle_result.normal) < 0.0;

                        if moving_toward {
                            let ball_angle = ball.pos.y.atan2(ball.pos.x);
                            let paddle_center = state.paddle.theta;
                            let half_arc = state.paddle.arc_width / 2.0;

                            let mut hit_offset = crate::normalize_angle(ball_angle - paddle_center);
                            hit_offset = (hit_offset / half_arc).clamp(-1.0, 1.0);

                            let base_reflect =
                                super::collision::reflect_velocity(ball.vel, paddle_result.normal);
                            let speed = ball.vel.length();
                            let tangent =
                                Vec2::new(-paddle_result.normal.y, paddle_result.normal.x);
                            let deflection = tangent * hit_offset * speed * 0.6;
                            let english = tangent * state.paddle.angular_vel * PADDLE_RADIUS * 0.15;

                            // Apply paddle boost to help escape gravity
                            let boosted_speed = (speed * PADDLE_BOOST).min(BALL_MAX_SPEED);
                            ball.vel =
                                (base_reflect + deflection + english).normalize() * boosted_speed;

                            let safe_dist = paddle_outer + ball.radius + 1.0;
                            let ball_angle_rad = ball.pos.y.atan2(ball.pos.x);
                            ball.pos = Vec2::new(
                                safe_dist * ball_angle_rad.cos(),
                                safe_dist * ball_angle_rad.sin(),
                            );

                            ball.paddle_cooldown = 8;
                        }
                    }
                }

                // SDF-based collision detection with raymarching
                // Move ball and check for collisions using signed distance fields
                let speed = ball.vel.length();
                let move_dist = speed * dt;
                let step_size = ball.radius * 0.3; // Small steps for accuracy
                let num_steps = ((move_dist / step_size).ceil() as usize).clamp(1, 20);
                let step_dt = dt / num_steps as f32;

                let mut blocks_to_damage = Vec::new();

                // Clone block arcs for SDF closure (needed for borrow checker)
                let block_arcs: Vec<_> = state
                    .blocks
                    .iter()
                    .map(|b| {
                        (
                            b.id,
                            b.arc.theta_start,
                            b.arc.theta_end,
                            b.arc.radius,
                            b.arc.thickness,
                            b.kind,
                        )
                    })
                    .collect();

                for _step in 0..num_steps {
                    // Move ball by one substep
                    ball.pos += ball.vel * step_dt;

                    // --- SDF Wall Collision ---
                    let wall_dist = ball.pos.length() - ARENA_OUTER_RADIUS;
                    if wall_dist > -ball.radius {
                        // Hit outer wall
                        let normal = -ball.pos.normalize_or_zero();
                        ball.vel = reflect_velocity(ball.vel, normal);
                        let penetration = wall_dist + ball.radius;
                        ball.pos += normal * (penetration + 1.0);
                    }

                    // --- SDF Block Collisions ---
                    for (idx, &(block_id, theta_start, theta_end, radius, thickness, kind)) in
                        block_arcs.iter().enumerate()
                    {
                        let block_dist =
                            super::sdf::sd_arc(ball.pos, theta_start, theta_end, radius, thickness);
                        let inside_block = block_dist < ball.radius;

                        // Portal blocks: ball slides through visibly and exits at the end
                        if matches!(kind, super::state::BlockKind::Portal { .. }) {
                            // Only enter portal if ball is Free (not already sliding)
                            if inside_block && matches!(ball.state, BallState::Free) {
                                // Determine slide direction based on entry angle
                                let entry_theta = ball.pos.y.atan2(ball.pos.x);

                                // Find which end of the arc we're closer to
                                let dist_to_start = (entry_theta - theta_start)
                                    .abs()
                                    .min(std::f32::consts::TAU - (entry_theta - theta_start).abs());
                                let dist_to_end = (entry_theta - theta_end)
                                    .abs()
                                    .min(std::f32::consts::TAU - (entry_theta - theta_end).abs());

                                // Slide toward the farther end
                                let direction = if dist_to_start < dist_to_end {
                                    1.0
                                } else {
                                    -1.0
                                };

                                // Clamp entry theta to arc bounds
                                let clamped_theta = entry_theta
                                    .clamp(theta_start.min(theta_end), theta_start.max(theta_end));

                                // Pick random exit distance (0.5 to 2π radians)
                                let hash = ball
                                    .id
                                    .wrapping_mul(31337)
                                    .wrapping_add(state.time_ticks as u32)
                                    .wrapping_mul(7919);
                                let rand_t = (hash % 1000) as f32 / 1000.0; // 0.0 to 1.0
                                let random_max = 0.5 + rand_t * (std::f32::consts::TAU - 0.5);

                                ball.state = BallState::Sliding {
                                    block_id,
                                    theta: clamped_theta,
                                    direction,
                                    entry_speed: ball.vel.length(),
                                    arc_start: theta_start,
                                    arc_end: theta_end,
                                    radius,
                                    total_traveled: 0.0,
                                    max_travel: random_max, // Random exit point
                                };
                                // Store velocity direction for later
                                ball.vel = ball.vel.normalize() * ball.vel.length();
                            }
                            continue;
                        }

                        if inside_block {
                            // Compute normal via SDF gradient
                            let eps = 1.0;
                            let dx = super::sdf::sd_arc(
                                ball.pos + Vec2::new(eps, 0.0),
                                theta_start,
                                theta_end,
                                radius,
                                thickness,
                            ) - super::sdf::sd_arc(
                                ball.pos - Vec2::new(eps, 0.0),
                                theta_start,
                                theta_end,
                                radius,
                                thickness,
                            );
                            let dy = super::sdf::sd_arc(
                                ball.pos + Vec2::new(0.0, eps),
                                theta_start,
                                theta_end,
                                radius,
                                thickness,
                            ) - super::sdf::sd_arc(
                                ball.pos - Vec2::new(0.0, eps),
                                theta_start,
                                theta_end,
                                radius,
                                thickness,
                            );
                            let normal = Vec2::new(dx, dy).normalize_or_zero();

                            if !ball.piercing {
                                // Only reflect if moving toward the surface
                                if ball.vel.dot(normal) < 0.0 {
                                    ball.vel = reflect_velocity(ball.vel, normal);
                                }
                                // Push out
                                let penetration = ball.radius - block_dist;
                                ball.pos += normal * (penetration + 1.5);
                            }

                            // Damage block (check original state.blocks)
                            if idx < state.blocks.len()
                                && state.blocks[idx].kind != super::state::BlockKind::Invincible
                                && !blocks_to_damage.contains(&idx)
                            {
                                blocks_to_damage.push(idx);
                                state.combo += 1;
                            }
                            break; // One collision per substep
                        }
                    }
                }

                // Apply block damage
                for idx in blocks_to_damage.into_iter().rev() {
                    // Trigger wobble on jello blocks
                    state.blocks[idx].trigger_wobble();

                    state.blocks[idx].hp = state.blocks[idx].hp.saturating_sub(1);
                    if state.blocks[idx].hp == 0 {
                        let block = state.blocks.remove(idx);

                        // SPAWN PARTICLES! 🎆
                        let mid_angle = (block.arc.theta_start + block.arc.theta_end) / 2.0;
                        let arc_span = block.arc.theta_end - block.arc.theta_start;
                        let color = match block.kind {
                            super::state::BlockKind::Glass => 0,
                            super::state::BlockKind::Armored => 1,
                            super::state::BlockKind::Explosive => 2,
                            super::state::BlockKind::Invincible => 3,
                            super::state::BlockKind::Portal { .. } => 4,
                            super::state::BlockKind::Jello => 5,
                            _ => 0,
                        };

                        // Spawn 20-40 particles - MAKE IT RAIN!
                        let particle_count = (20.0 + arc_span * 30.0).min(40.0) as usize;
                        let particle_seed = state.time_ticks as u32;

                        for i in 0..particle_count {
                            if state.particles.len() >= super::state::MAX_PARTICLES {
                                // Remove oldest particles to make room
                                state.particles.remove(0);
                            }
                            // Deterministic "random" spread using hash
                            let hash = particle_seed
                                .wrapping_mul(2654435761)
                                .wrapping_add(i as u32 * 7919);
                            let angle_offset =
                                ((hash % 1000) as f32 / 1000.0 - 0.5) * arc_span * 1.2;
                            let radius_offset =
                                ((hash / 1000 % 1000) as f32 / 1000.0 - 0.5) * block.arc.thickness;
                            let spawn_angle = mid_angle + angle_offset;
                            let spawn_radius = block.arc.radius + radius_offset;

                            let pos = Vec2::new(
                                spawn_angle.cos() * spawn_radius,
                                spawn_angle.sin() * spawn_radius,
                            );

                            // Velocity: EXPLODE outward with variety
                            let base_speed = 120.0 + ((hash / 1000000 % 150) as f32);
                            let vel_angle =
                                spawn_angle + ((hash / 100000 % 1000) as f32 / 1000.0 - 0.5) * 2.0;
                            // Mix of outward and tangential motion
                            let outward = Vec2::new(vel_angle.cos(), vel_angle.sin());
                            let tangent = Vec2::new(-vel_angle.sin(), vel_angle.cos());
                            let tang_factor =
                                ((hash / 10000000 % 1000) as f32 / 1000.0 - 0.5) * 0.5;
                            let vel = (outward + tangent * tang_factor).normalize() * base_speed;

                            let size = 4.0 + ((hash / 10000 % 100) as f32 / 100.0) * 6.0;

                            state.particles.push(super::state::Particle {
                                pos,
                                vel,
                                color,
                                life: 1.0,
                                size,
                            });
                        }

                        // PICKUP SPAWN! ~10% chance on block destroy
                        let pickup_hash =
                            particle_seed.wrapping_mul(31337).wrapping_add(idx as u32);
                        if pickup_hash.is_multiple_of(10) {
                            let pickup_kind = match pickup_hash / 10 % 5 {
                                0 => PickupKind::MultiBall,
                                1 => PickupKind::Slow,
                                2 => PickupKind::Piercing,
                                3 => PickupKind::WidenPaddle,
                                _ => PickupKind::Shield,
                            };
                            let spawn_pos = Vec2::new(
                                mid_angle.cos() * block.arc.radius,
                                mid_angle.sin() * block.arc.radius,
                            );
                            pickups_to_spawn.push((pickup_kind, spawn_pos));
                        }

                        // EXPLOSIVE BLOCKS: Destroy neighbors in blast radius!
                        let destroyed_radius = block.arc.radius;
                        let destroyed_mid_angle = mid_angle;
                        let is_explosive = block.kind == super::state::BlockKind::Explosive;

                        // Screen shake on explosions!
                        if is_explosive {
                            state.screen_shake = (state.screen_shake + 0.4).min(1.0);
                        }

                        // Collect neighbors to damage (for explosives) or wobble (for jello)
                        let mut explosion_victims = Vec::new();
                        for (n_idx, neighbor) in state.blocks.iter_mut().enumerate() {
                            let neighbor_mid =
                                (neighbor.arc.theta_start + neighbor.arc.theta_end) / 2.0;
                            let mut angle_diff = (neighbor_mid - destroyed_mid_angle).abs();
                            if angle_diff > std::f32::consts::PI {
                                angle_diff = std::f32::consts::TAU - angle_diff;
                            }
                            let radius_diff = (neighbor.arc.radius - destroyed_radius).abs();

                            // Neighbor if same layer (close radius) and adjacent angle, OR adjacent layer
                            let same_layer_adjacent = radius_diff < 10.0 && angle_diff < 0.6;
                            let adjacent_layer =
                                radius_diff < 60.0 && radius_diff > 5.0 && angle_diff < 0.3;
                            let is_neighbor = same_layer_adjacent || adjacent_layer;

                            // Wobble jello neighbors
                            if is_neighbor && neighbor.kind == super::state::BlockKind::Jello {
                                neighbor.wobble = (neighbor.wobble + 0.5).min(1.0);
                            }

                            // EXPLOSION: damage ALL neighbors (except invincible)
                            if is_explosive
                                && is_neighbor
                                && neighbor.kind != super::state::BlockKind::Invincible
                            {
                                explosion_victims.push(n_idx);
                            }
                        }

                        // Apply explosion damage to neighbors with VISIBLE CHAIN REACTION
                        let explosion_center = Vec2::new(
                            destroyed_mid_angle.cos() * destroyed_radius,
                            destroyed_mid_angle.sin() * destroyed_radius,
                        );

                        for victim_idx in explosion_victims.into_iter().rev() {
                            if victim_idx < state.blocks.len() {
                                let victim = &state.blocks[victim_idx];
                                let v_mid = (victim.arc.theta_start + victim.arc.theta_end) / 2.0;
                                let v_radius = victim.arc.radius;
                                let victim_center =
                                    Vec2::new(v_mid.cos() * v_radius, v_mid.sin() * v_radius);

                                // FIREBALL particles traveling FROM explosion TO victim!
                                let direction =
                                    (victim_center - explosion_center).normalize_or_zero();
                                let distance = (victim_center - explosion_center).length();

                                for i in 0..8 {
                                    if state.particles.len() >= super::state::MAX_PARTICLES {
                                        state.particles.remove(0);
                                    }
                                    let hash = (state.time_ticks as u32)
                                        .wrapping_mul(7919)
                                        .wrapping_add(victim_idx as u32 * 1000 + i);

                                    // Start at explosion, travel toward victim
                                    let spread = ((hash % 1000) as f32 / 1000.0 - 0.5) * 0.3;
                                    let perpendicular = Vec2::new(-direction.y, direction.x);
                                    let fireball_dir =
                                        (direction + perpendicular * spread).normalize();

                                    // Speed based on distance so they arrive at similar times
                                    let speed =
                                        distance * 3.0 + 50.0 + ((hash / 1000 % 100) as f32);

                                    state.particles.push(super::state::Particle {
                                        pos: explosion_center + fireball_dir * 5.0,
                                        vel: fireball_dir * speed,
                                        color: 2, // Orange (explosive)
                                        life: 0.6,
                                        size: 6.0 + ((hash / 10000 % 100) as f32 / 100.0) * 4.0,
                                    });
                                }

                                // Impact particles AT the victim
                                for i in 0..6 {
                                    if state.particles.len() >= super::state::MAX_PARTICLES {
                                        state.particles.remove(0);
                                    }
                                    let hash = (state.time_ticks as u32)
                                        .wrapping_add(i * 3571 + victim_idx as u32);
                                    let angle = v_mid + ((hash % 1000) as f32 / 1000.0 - 0.5) * 0.8;
                                    let pos =
                                        Vec2::new(angle.cos() * v_radius, angle.sin() * v_radius);
                                    let vel = Vec2::new(angle.cos(), angle.sin())
                                        * (80.0 + (hash / 1000 % 80) as f32);
                                    state.particles.push(super::state::Particle {
                                        pos,
                                        vel,
                                        color: 2, // Orange
                                        life: 0.5,
                                        size: 4.0,
                                    });
                                }

                                // Now damage the victim
                                state.blocks[victim_idx].hp =
                                    state.blocks[victim_idx].hp.saturating_sub(2);
                                state.blocks[victim_idx].trigger_wobble();
                            }
                        }

                        // Remove dead blocks from explosion
                        state.blocks.retain(|b| b.hp > 0);

                        // Score with combo multiplier! (1.1x at combo 2, up to 3.0x at 21)
                        let base_score = match block.kind {
                            super::state::BlockKind::Glass => 10,
                            super::state::BlockKind::Armored => 25,
                            super::state::BlockKind::Explosive => 50,
                            super::state::BlockKind::Jello => 20,
                            super::state::BlockKind::Invincible => 0, // Should never happen
                            _ => 15,
                        };
                        let multiplier = if state.combo > 1 {
                            (1.0 + (state.combo - 1) as f32 * 0.1).min(3.0)
                        } else {
                            1.0
                        };
                        state.score += (base_score as f32 * multiplier) as u64;
                    }
                }

                // Record trail position (every 2nd tick for performance)
                if state.time_ticks.is_multiple_of(2) {
                    ball.record_trail();
                }
            }

            // Spawn collected pickups (deferred from block destruction)
            for (kind, pos) in pickups_to_spawn {
                let id = state.next_entity_id();
                state.pickups.push(Pickup {
                    id,
                    kind,
                    pos,
                    vel: Vec2::ZERO,
                    ttl_ticks: 1200, // 10 seconds at 120Hz
                });
            }

            // Update particles
            for particle in state.particles.iter_mut() {
                // Apply velocity
                particle.pos += particle.vel * dt;
                // Gravity toward black hole (weaker than ball)
                let to_center = -particle.pos.normalize_or_zero();
                particle.vel += to_center * 50.0 * dt;
                // Drag to slow down
                particle.vel *= 0.98;
                // Decay life
                particle.life -= dt * 1.5; // ~0.67 second lifetime
                // Shrink as they die
                particle.size *= 0.995;
            }
            // Remove dead particles
            state.particles.retain(|p| p.life > 0.0);

            // Update pickups
            let paddle_pos = Vec2::new(
                state.paddle.theta.cos() * PADDLE_RADIUS,
                state.paddle.theta.sin() * PADDLE_RADIUS,
            );
            for pickup in state.pickups.iter_mut() {
                // Move pickup
                pickup.pos += pickup.vel * dt;
                // Drift toward paddle (not black hole!)
                let to_paddle = (paddle_pos - pickup.pos).normalize_or_zero();
                pickup.vel += to_paddle * 80.0 * dt;
                // Light drag
                pickup.vel *= 0.98;
                // Clamp speed
                let speed = pickup.vel.length();
                if speed > 150.0 {
                    pickup.vel = pickup.vel.normalize() * 150.0;
                }
                // TTL countdown
                pickup.ttl_ticks = pickup.ttl_ticks.saturating_sub(1);
            }

            // Check pickup collection by paddle
            let paddle_theta = state.paddle.theta;
            let paddle_half_arc = state.paddle.arc_width / 2.0;
            let paddle_inner = PADDLE_RADIUS - PADDLE_THICKNESS / 2.0;
            let paddle_outer = PADDLE_RADIUS + PADDLE_THICKNESS / 2.0;

            let mut collected_effects: Vec<PickupKind> = Vec::new();
            state.pickups.retain(|pickup| {
                // Check if pickup is near paddle
                let pickup_dist = pickup.pos.length();
                let pickup_angle = pickup.pos.y.atan2(pickup.pos.x);
                let mut angle_diff = (pickup_angle - paddle_theta).abs();
                if angle_diff > std::f32::consts::PI {
                    angle_diff = std::f32::consts::TAU - angle_diff;
                }

                let in_arc = angle_diff < paddle_half_arc + 0.1; // Small collection radius
                let in_radius =
                    pickup_dist > paddle_inner - 10.0 && pickup_dist < paddle_outer + 10.0;

                if in_arc && in_radius {
                    collected_effects.push(pickup.kind);
                    false // Remove collected pickup
                } else if pickup.ttl_ticks == 0 || pickup_dist < BLACK_HOLE_RADIUS {
                    false // Remove expired or sucked in
                } else {
                    true // Keep
                }
            });

            // Apply collected effects
            for kind in collected_effects {
                match kind {
                    PickupKind::MultiBall => {
                        // Spawn 2 extra balls
                        if let Some(ball) = state
                            .balls
                            .iter()
                            .find(|b| matches!(b.state, BallState::Free))
                            .cloned()
                        {
                            for i in 0..2 {
                                let angle_offset: f32 = if i == 0 { 0.5 } else { -0.5 };
                                let new_vel = Vec2::new(
                                    ball.vel.x * angle_offset.cos()
                                        - ball.vel.y * angle_offset.sin(),
                                    ball.vel.x * angle_offset.sin()
                                        + ball.vel.y * angle_offset.cos(),
                                )
                                .normalize()
                                    * ball.vel.length();
                                let id = state.next_entity_id();
                                state.balls.push(super::state::Ball {
                                    id,
                                    pos: ball.pos,
                                    vel: new_vel,
                                    radius: BALL_RADIUS,
                                    state: BallState::Free,
                                    piercing: ball.piercing,
                                    paddle_cooldown: 0,
                                    trail: Vec::new(),
                                    inside_portals: Vec::new(),
                                });
                            }
                        }
                    }
                    PickupKind::Slow => {
                        state.effects.slow_ticks = 600; // 5 seconds at 120Hz
                    }
                    PickupKind::Piercing => {
                        state.effects.piercing_ticks = 480; // 4 seconds
                    }
                    PickupKind::WidenPaddle => {
                        state.effects.widen_ticks = 720; // 6 seconds
                    }
                    PickupKind::Shield => {
                        state.effects.shield_active = true;
                    }
                }
                // Visual feedback - particles
                state.screen_shake = (state.screen_shake + 0.15).min(1.0);
            }

            // Decay timed effects
            state.effects.slow_ticks = state.effects.slow_ticks.saturating_sub(1);
            state.effects.piercing_ticks = state.effects.piercing_ticks.saturating_sub(1);
            state.effects.widen_ticks = state.effects.widen_ticks.saturating_sub(1);

            // Apply piercing effect to all balls
            let piercing_active = state.effects.piercing_ticks > 0;
            for ball in state.balls.iter_mut() {
                ball.piercing = piercing_active;
            }

            // Apply widen paddle effect (50% wider when active)
            state.paddle.arc_width = if state.effects.widen_ticks > 0 {
                PADDLE_ARC_WIDTH * 1.5
            } else {
                PADDLE_ARC_WIDTH
            };

            // Apply slow effect - reduce ball speed by 40%
            if state.effects.slow_ticks > 0 {
                for ball in state.balls.iter_mut() {
                    if matches!(ball.state, BallState::Free) {
                        let speed = ball.vel.length();
                        let slowed_max = BALL_MAX_SPEED * 0.6;
                        if speed > slowed_max {
                            ball.vel = ball.vel.normalize() * slowed_max;
                        }
                    }
                }
            }

            // Black hole check - start death animation (or bounce if shield active)
            let mut shield_used = false;
            for ball in state.balls.iter_mut() {
                if matches!(ball.state, BallState::Free)
                    && ball.pos.length() <= BLACK_HOLE_LOSS_RADIUS + ball.radius
                {
                    if state.effects.shield_active && !shield_used {
                        // Shield saves the ball! Bounce it away
                        let outward = ball.pos.normalize_or_zero();
                        ball.vel = outward * BALL_MAX_SPEED * 0.8;
                        ball.pos = outward * (BLACK_HOLE_LOSS_RADIUS + ball.radius + 5.0);
                        shield_used = true;
                        state.screen_shake = (state.screen_shake + 0.5).min(1.0);
                    } else {
                        ball.state = BallState::Dying {
                            timer: 0.0,
                            start_pos: (ball.pos.x, ball.pos.y),
                        };
                        state.combo = 0;
                    }
                }
            }
            if shield_used {
                state.effects.shield_active = false;
            }

            // Update dying balls
            let death_duration = 0.8; // seconds
            for ball in state.balls.iter_mut() {
                if let BallState::Dying {
                    ref mut timer,
                    start_pos,
                } = ball.state
                {
                    *timer += dt;
                    // Spiral into center
                    let t = (*timer / death_duration).min(1.0);
                    let spiral_angle = t * 6.0 * std::f32::consts::PI;
                    let shrink = 1.0 - t;
                    let radius = shrink * Vec2::new(start_pos.0, start_pos.1).length();
                    let base_angle = start_pos.1.atan2(start_pos.0);
                    let old_pos = ball.pos;
                    ball.pos = Vec2::new(
                        (base_angle + spiral_angle).cos() * radius,
                        (base_angle + spiral_angle).sin() * radius,
                    );
                    ball.radius = BALL_RADIUS * shrink * shrink; // Shrink faster

                    // Set velocity for trail color (based on movement)
                    if dt > 0.0 {
                        ball.vel = (ball.pos - old_pos) / dt;
                    }

                    // Record trail during death spiral
                    ball.record_trail();
                }
            }

            // Remove fully dead balls
            state.balls.retain(|ball| {
                if let BallState::Dying { timer, .. } = ball.state {
                    timer < death_duration
                } else {
                    true
                }
            });

            // Check if all balls lost (none alive or dying)
            if state.balls.is_empty() {
                state.lives = state.lives.saturating_sub(1);
                if state.lives == 0 {
                    state.phase = GamePhase::GameOver;
                } else {
                    // Respawn after delay (handled by respawn timer, simplified here)
                    state.spawn_ball_attached();
                    state.phase = GamePhase::Serve;
                }
            }

            // Check wave clear (invincible blocks don't count)
            let clearable_blocks = state.blocks.iter().filter(|b| b.counts_for_clear()).count();
            if clearable_blocks == 0 {
                // Remove invincible blocks too when wave clears
                state.blocks.clear();
                state.wave_index += 1;
                state.breather_ticks = BREATHER_DURATION_TICKS;
                state.phase = GamePhase::Breather;
                // Clear balls for breather
                state.balls.clear();
            }
        }

        GamePhase::Breather => {
            // Keep blocks rotating during breather
            for block in &mut state.blocks {
                block.rotate(dt);
            }

            // Keep particles animating during breather!
            for particle in state.particles.iter_mut() {
                particle.pos += particle.vel * dt;
                let to_center = -particle.pos.normalize_or_zero();
                particle.vel += to_center * 50.0 * dt;
                particle.vel *= 0.98;
                particle.life -= dt * 1.5;
                particle.size *= 0.995;
            }
            state.particles.retain(|p| p.life > 0.0);

            state.breather_ticks = state.breather_ticks.saturating_sub(1);
            if state.breather_ticks == 0 {
                // Generate next wave (TODO: proper generator)
                generate_wave(state);
                // Spawn ball for serve
                state.spawn_ball_attached();
                state.phase = GamePhase::Serve;
            }
        }

        _ => {}
    }

    // Ensure deterministic ordering
    state.normalize_order();
}

fn reflect_velocity(vel: Vec2, normal: Vec2) -> Vec2 {
    super::collision::reflect_velocity(vel, normal)
}

/// Generate wave with variable blocks, widths, and layers
pub fn generate_wave(state: &mut GameState) {
    use super::arc::ArcSegment;
    use super::state::{Block, BlockKind};
    use std::f32::consts::PI;

    let wave = state.wave_index;

    // Deterministic "randomness" based on wave number AND game seed
    // This gives variety between runs while keeping determinism within a run
    let wave_seed = ((wave as u64)
        .wrapping_mul(2654435761)
        .wrapping_add(state.seed)) as u32;

    // Number of layers increases with wave (1-4 layers)
    let num_layers = 1 + (wave / 3).min(3);

    // Layer radii from outer to inner (outer layer close to wall at 400)
    let layer_radii = [375.0, 320.0, 265.0, 210.0];

    for layer in 0..num_layers {
        let radius = layer_radii[layer as usize];
        let layer_seed = wave_seed.wrapping_add(layer * 1000);

        // More blocks in outer layers, fewer in inner
        let base_blocks = match layer {
            0 => 12 + wave * 2, // Outer: 12-32 blocks
            1 => 10 + wave,     // Second: 10-22
            2 => 8 + wave / 2,  // Third: 8-14
            _ => 6 + wave / 3,  // Inner: 6-10
        };
        let num_blocks = base_blocks.min(28) as usize;

        // Layer style: packed (no gaps) or spaced (gaps)
        let packed = !layer_seed.is_multiple_of(3); // ~67% packed, 33% spaced

        // Rotation: occasionally ONE layer rotates (wave 2+)
        // Use a better hash to decorrelate layers
        let rotation_hash = layer_seed
            .wrapping_mul(2654435761) // Golden ratio hash
            .wrapping_add(layer * 7919); // Prime offset per layer
        let rotation_roll = rotation_hash % 100;

        // ~20% chance per layer rotates, so usually 0-1 spinning rings
        let rotation_speed = if wave >= 2 && rotation_roll < 20 {
            let base_speed = 0.2 + (layer as f32) * 0.08; // Gentle rotation
            let direction = if (rotation_hash / 100).is_multiple_of(2) {
                1.0
            } else {
                -1.0
            };
            base_speed * direction
        } else {
            0.0 // Stationary (most layers)
        };

        let mut theta = (layer as f32) * 0.15; // Offset each layer
        let base_arc = (2.0 * PI) / num_blocks as f32;
        let mut invincible_in_layer = 0u32;

        for i in 0..num_blocks {
            let block_seed = layer_seed.wrapping_add(i as u32 * 100);

            // Skip some positions for variety (creates missing block gaps)
            // More skips in spaced layers, fewer in packed
            let skip_chance = if packed { 12 } else { 6 };
            if block_seed.is_multiple_of(skip_chance) && wave > 1 {
                theta += base_arc;
                continue;
            }

            // Block width depends on packing style
            let (arc_width, gap) = if packed {
                // Packed: blocks fill most of their slot
                let width = base_arc * 0.95; // 95% fill, tiny gap
                (width, base_arc * 0.025)
            } else {
                // Spaced: variable widths with gaps
                let width_mult = if block_seed.is_multiple_of(5) {
                    0.75
                } else if block_seed.is_multiple_of(3) {
                    0.65
                } else {
                    0.55
                };
                let width = base_arc * width_mult;
                let gap = (base_arc - width) / 2.0;
                (width, gap)
            };

            let theta_start = theta + gap;
            let theta_end = theta_start + arc_width;

            // Block type based on wave and position (limit invincible per layer)
            let kind = determine_block_kind(
                wave,
                layer,
                i as u32,
                block_seed,
                num_blocks,
                invincible_in_layer,
            );

            if kind == BlockKind::Invincible {
                invincible_in_layer += 1;
            }

            let hp = match kind {
                BlockKind::Armored => 2 + (wave / 5) as u8, // Armored gets tougher
                BlockKind::Explosive => 1,
                BlockKind::Invincible => 255, // Doesn't matter, can't be damaged
                BlockKind::Portal { .. } => 3, // 3 passes before breaking
                BlockKind::Jello => 2,        // Takes 2 hits, wobbles each time
                _ => 1,
            };

            // Variable thickness for some blocks
            let thickness = if block_seed.is_multiple_of(11) && wave > 3 {
                BLOCK_THICKNESS * 1.5
            } else {
                BLOCK_THICKNESS
            };

            let block = Block {
                id: state.next_entity_id(),
                kind,
                hp,
                arc: ArcSegment::new(radius, thickness, theta_start, theta_end),
                rotation_speed,
                wobble: 0.0,
            };
            state.blocks.push(block);

            theta += base_arc;
        }
    }
}

/// Determine block type based on wave progression
/// invincible_count tracks how many invincible blocks already in this layer
fn determine_block_kind(
    wave: u32,
    layer: u32,
    index: u32,
    seed: u32,
    layer_block_count: usize,
    invincible_in_layer: u32,
) -> super::state::BlockKind {
    use super::state::BlockKind;

    // Wave 0-1: all glass (tutorial waves)
    if wave <= 1 {
        return BlockKind::Glass;
    }

    // Use a simple hash for variety
    let roll = seed % 100;

    // Invincible blocks (wave 5+, very sparse)
    // Max 2 per layer, and never adjacent (check index spacing)
    // Also need gaps - so cap at ~15% of layer
    let max_invincible = (layer_block_count / 7).max(1) as u32; // ~14% max
    let can_place_invincible =
        wave >= 5 && invincible_in_layer < max_invincible.min(2) && index.is_multiple_of(4); // Spread them out (every 4th slot eligible)

    if can_place_invincible && roll < 8 {
        return BlockKind::Invincible;
    }

    // Explosive blocks (wave 3+, outer layer only, ~12% chance)
    if wave >= 3 && layer == 0 && roll < 12 {
        return BlockKind::Explosive;
    }

    // Portal blocks (wave 4+, ~8% chance, not on innermost layer)
    if wave >= 4 && layer < 3 && (12..20).contains(&roll) {
        return BlockKind::Portal { pair_id: seed }; // pair_id for future pairing
    }

    // Jello blocks (wave 3+, ~10% chance, inner layers preferred)
    if wave >= 3 && layer >= 1 && (20..30).contains(&roll) {
        return BlockKind::Jello;
    }

    // Armored blocks increase with wave
    // Wave 2: 25%, Wave 3: 35%, Wave 4+: 45%
    let armored_chance = match wave {
        2 => 25,
        3 => 35,
        _ => 45,
    };

    // Inner layers get more armored blocks (+10% per layer)
    let armored_chance = armored_chance + (layer * 10);

    if roll < armored_chance {
        return BlockKind::Armored;
    }

    BlockKind::Glass
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tick_serve_to_playing() {
        let mut state = GameState::new(12345);
        assert_eq!(state.phase, GamePhase::Serve);
        assert_eq!(state.balls.len(), 1);

        // Tick without launch - should stay in Serve
        let input = TickInput::default();
        tick(&mut state, &input, SIM_DT);
        assert_eq!(state.phase, GamePhase::Serve);

        // Launch
        let input = TickInput {
            launch: true,
            ..Default::default()
        };
        tick(&mut state, &input, SIM_DT);
        assert_eq!(state.phase, GamePhase::Playing);
        assert!(matches!(state.balls[0].state, BallState::Free));
    }

    #[test]
    fn test_tick_pause() {
        use crate::sim::ArcSegment;
        use crate::sim::state::{Block, BlockKind};

        let mut state = GameState::new(12345);

        // Add a block so wave doesn't immediately clear
        let block_id = state.next_entity_id();
        state.blocks.push(Block {
            id: block_id,
            kind: BlockKind::Glass,
            hp: 1,
            arc: ArcSegment::new(200.0, 20.0, 0.0, 0.5),
            rotation_speed: 0.0,
            wobble: 0.0,
        });

        // Launch the ball first so we're in Playing state
        let launch = TickInput {
            launch: true,
            ..Default::default()
        };
        tick(&mut state, &launch, SIM_DT);
        assert_eq!(state.phase, GamePhase::Playing);

        // Now pause
        let input = TickInput {
            pause: true,
            ..Default::default()
        };
        tick(&mut state, &input, SIM_DT);
        assert_eq!(state.phase, GamePhase::Paused);

        // Unpause
        tick(&mut state, &input, SIM_DT);
        assert_eq!(state.phase, GamePhase::Playing);
    }

    #[test]
    fn test_determinism() {
        // Two states with same seed should produce identical results
        let mut state1 = GameState::new(99999);
        let mut state2 = GameState::new(99999);

        let inputs = [
            TickInput {
                target_theta: Some(0.5),
                ..Default::default()
            },
            TickInput {
                launch: true,
                ..Default::default()
            },
            TickInput {
                target_theta: Some(0.7),
                ..Default::default()
            },
            TickInput::default(),
        ];

        for input in &inputs {
            tick(&mut state1, input, SIM_DT);
            tick(&mut state2, input, SIM_DT);
        }

        assert_eq!(state1.time_ticks, state2.time_ticks);
        assert_eq!(state1.balls.len(), state2.balls.len());
        assert!((state1.paddle.theta - state2.paddle.theta).abs() < 0.0001);
    }
}
