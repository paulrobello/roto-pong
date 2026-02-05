//! Fixed timestep simulation tick
//!
//! Core game loop that advances simulation deterministically.

use glam::Vec2;

use super::state::{BREATHER_DURATION_TICKS, BallState, GamePhase, GameState};
use super::{CollisionResult, ball_arc_collision};
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
        let max_speed = 12.0; // radians per second
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

            // Collision detection and response
            let paddle_arc = state.paddle.as_arc();
            let paddle_outer = PADDLE_RADIUS + PADDLE_THICKNESS / 2.0;
            let _paddle_inner = PADDLE_RADIUS - PADDLE_THICKNESS / 2.0;

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
                    let crossing_outer = old_r > paddle_outer + ball.radius 
                                       && new_r <= paddle_outer + ball.radius;
                    
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
                            ball.vel = (base_reflect + deflection + english).normalize() * boosted_speed;
                            
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
                            
                            let base_reflect = super::collision::reflect_velocity(ball.vel, paddle_result.normal);
                            let speed = ball.vel.length();
                            let tangent = Vec2::new(-paddle_result.normal.y, paddle_result.normal.x);
                            let deflection = tangent * hit_offset * speed * 0.6;
                            let english = tangent * state.paddle.angular_vel * PADDLE_RADIUS * 0.15;
                            
                            // Apply paddle boost to help escape gravity
                            let boosted_speed = (speed * PADDLE_BOOST).min(BALL_MAX_SPEED);
                            ball.vel = (base_reflect + deflection + english).normalize() * boosted_speed;
                            
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
                let block_arcs: Vec<_> = state.blocks.iter()
                    .map(|b| (b.id, b.arc.theta_start, b.arc.theta_end, b.arc.radius, b.arc.thickness, b.kind))
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
                    for (idx, &(block_id, theta_start, theta_end, radius, thickness, kind)) in block_arcs.iter().enumerate() {
                        let block_dist = super::sdf::sd_arc(ball.pos, theta_start, theta_end, radius, thickness);
                        let inside_block = block_dist < ball.radius;
                        
                        // Portal blocks: track entry/exit, only damage on exit
                        if matches!(kind, super::state::BlockKind::Portal { .. }) {
                            let was_inside = ball.inside_portals.contains(&block_id);
                            
                            if inside_block && !was_inside {
                                // Just entered portal - start tracking
                                ball.inside_portals.push(block_id);
                                // Bend the ball's path significantly (refraction on entry)
                                let bend_amount = 0.4; // Increased IOR effect
                                let to_center = -ball.pos.normalize_or_zero();
                                ball.vel = (ball.vel.normalize() + to_center * bend_amount).normalize() * ball.vel.length();
                            } else if !inside_block && was_inside {
                                // Just exited portal - NOW damage it
                                ball.inside_portals.retain(|&id| id != block_id);
                                if idx < state.blocks.len() {
                                    if !blocks_to_damage.contains(&idx) {
                                        blocks_to_damage.push(idx);
                                        state.combo += 1;
                                    }
                                }
                            }
                            // While inside, ball passes through freely
                            continue;
                        }
                        
                        if inside_block {
                            // Compute normal via SDF gradient
                            let eps = 1.0;
                            let dx = super::sdf::sd_arc(ball.pos + Vec2::new(eps, 0.0), theta_start, theta_end, radius, thickness)
                                   - super::sdf::sd_arc(ball.pos - Vec2::new(eps, 0.0), theta_start, theta_end, radius, thickness);
                            let dy = super::sdf::sd_arc(ball.pos + Vec2::new(0.0, eps), theta_start, theta_end, radius, thickness)
                                   - super::sdf::sd_arc(ball.pos - Vec2::new(0.0, eps), theta_start, theta_end, radius, thickness);
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
                            if idx < state.blocks.len() && state.blocks[idx].kind != super::state::BlockKind::Invincible {
                                if !blocks_to_damage.contains(&idx) {
                                    blocks_to_damage.push(idx);
                                    state.combo += 1;
                                }
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
                            let hash = particle_seed.wrapping_mul(2654435761).wrapping_add(i as u32 * 7919);
                            let angle_offset = ((hash % 1000) as f32 / 1000.0 - 0.5) * arc_span * 1.2;
                            let radius_offset = ((hash / 1000 % 1000) as f32 / 1000.0 - 0.5) * block.arc.thickness;
                            let spawn_angle = mid_angle + angle_offset;
                            let spawn_radius = block.arc.radius + radius_offset;
                            
                            let pos = Vec2::new(spawn_angle.cos() * spawn_radius, spawn_angle.sin() * spawn_radius);
                            
                            // Velocity: EXPLODE outward with variety
                            let base_speed = 120.0 + ((hash / 1000000 % 150) as f32);
                            let vel_angle = spawn_angle + ((hash / 100000 % 1000) as f32 / 1000.0 - 0.5) * 2.0;
                            // Mix of outward and tangential motion
                            let outward = Vec2::new(vel_angle.cos(), vel_angle.sin());
                            let tangent = Vec2::new(-vel_angle.sin(), vel_angle.cos());
                            let tang_factor = ((hash / 10000000 % 1000) as f32 / 1000.0 - 0.5) * 0.5;
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
                        
                        // EXPLOSIVE BLOCKS: Destroy neighbors in blast radius!
                        let destroyed_radius = block.arc.radius;
                        let destroyed_mid_angle = mid_angle;
                        let is_explosive = block.kind == super::state::BlockKind::Explosive;
                        
                        // Collect neighbors to damage (for explosives) or wobble (for jello)
                        let mut explosion_victims = Vec::new();
                        for (n_idx, neighbor) in state.blocks.iter_mut().enumerate() {
                            let neighbor_mid = (neighbor.arc.theta_start + neighbor.arc.theta_end) / 2.0;
                            let mut angle_diff = (neighbor_mid - destroyed_mid_angle).abs();
                            if angle_diff > std::f32::consts::PI {
                                angle_diff = std::f32::consts::TAU - angle_diff;
                            }
                            let radius_diff = (neighbor.arc.radius - destroyed_radius).abs();
                            
                            // Neighbor if same layer (close radius) and adjacent angle, OR adjacent layer
                            let same_layer_adjacent = radius_diff < 10.0 && angle_diff < 0.6;
                            let adjacent_layer = radius_diff < 60.0 && radius_diff > 5.0 && angle_diff < 0.3;
                            let is_neighbor = same_layer_adjacent || adjacent_layer;
                            
                            // Wobble jello neighbors
                            if is_neighbor && neighbor.kind == super::state::BlockKind::Jello {
                                neighbor.wobble = (neighbor.wobble + 0.5).min(1.0);
                            }
                            
                            // EXPLOSION: damage ALL neighbors (except invincible)
                            if is_explosive && is_neighbor && neighbor.kind != super::state::BlockKind::Invincible {
                                explosion_victims.push(n_idx);
                            }
                        }
                        
                        // Apply explosion damage to neighbors with VISIBLE CHAIN REACTION
                        let explosion_center = Vec2::new(
                            destroyed_mid_angle.cos() * destroyed_radius,
                            destroyed_mid_angle.sin() * destroyed_radius
                        );
                        
                        for victim_idx in explosion_victims.into_iter().rev() {
                            if victim_idx < state.blocks.len() {
                                let victim = &state.blocks[victim_idx];
                                let v_mid = (victim.arc.theta_start + victim.arc.theta_end) / 2.0;
                                let v_radius = victim.arc.radius;
                                let victim_center = Vec2::new(v_mid.cos() * v_radius, v_mid.sin() * v_radius);
                                
                                // FIREBALL particles traveling FROM explosion TO victim!
                                let direction = (victim_center - explosion_center).normalize_or_zero();
                                let distance = (victim_center - explosion_center).length();
                                
                                for i in 0..8 {
                                    if state.particles.len() >= super::state::MAX_PARTICLES {
                                        state.particles.remove(0);
                                    }
                                    let hash = (state.time_ticks as u32).wrapping_mul(7919).wrapping_add(victim_idx as u32 * 1000 + i);
                                    
                                    // Start at explosion, travel toward victim
                                    let spread = ((hash % 1000) as f32 / 1000.0 - 0.5) * 0.3;
                                    let perpendicular = Vec2::new(-direction.y, direction.x);
                                    let fireball_dir = (direction + perpendicular * spread).normalize();
                                    
                                    // Speed based on distance so they arrive at similar times
                                    let speed = distance * 3.0 + 50.0 + ((hash / 1000 % 100) as f32);
                                    
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
                                    let hash = (state.time_ticks as u32).wrapping_add(i * 3571 + victim_idx as u32);
                                    let angle = v_mid + ((hash % 1000) as f32 / 1000.0 - 0.5) * 0.8;
                                    let pos = Vec2::new(angle.cos() * v_radius, angle.sin() * v_radius);
                                    let vel = Vec2::new(angle.cos(), angle.sin()) * (80.0 + (hash / 1000 % 80) as f32);
                                    state.particles.push(super::state::Particle {
                                        pos,
                                        vel,
                                        color: 2, // Orange
                                        life: 0.5,
                                        size: 4.0,
                                    });
                                }
                                
                                // Now damage the victim
                                state.blocks[victim_idx].hp = state.blocks[victim_idx].hp.saturating_sub(2);
                                state.blocks[victim_idx].trigger_wobble();
                            }
                        }
                        
                        // Remove dead blocks from explosion
                        state.blocks.retain(|b| b.hp > 0);
                        
                        // Score with combo multiplier! (1.0 + combo * 0.1, max 3.0x)
                        let base_score = match block.kind {
                            super::state::BlockKind::Glass => 10,
                            super::state::BlockKind::Armored => 25,
                            super::state::BlockKind::Explosive => 50,
                            super::state::BlockKind::Jello => 20,
                            super::state::BlockKind::Invincible => 0, // Should never happen
                            _ => 15,
                        };
                        let multiplier = (1.0 + state.combo as f32 * 0.1).min(3.0);
                        state.score += (base_score as f32 * multiplier) as u64;
                    }
                }

                // Record trail position (every 2nd tick for performance)
                if state.time_ticks % 2 == 0 {
                    ball.record_trail();
                }
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

            // Black hole check - start death animation
            for ball in state.balls.iter_mut() {
                if matches!(ball.state, BallState::Free)
                    && ball.pos.length() <= BLACK_HOLE_LOSS_RADIUS + ball.radius
                {
                    ball.state = BallState::Dying { 
                        timer: 0.0, 
                        start_pos: (ball.pos.x, ball.pos.y) 
                    };
                    state.combo = 0;
                }
            }
            
            // Update dying balls
            let death_duration = 0.8; // seconds
            for ball in state.balls.iter_mut() {
                if let BallState::Dying { ref mut timer, start_pos } = ball.state {
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

/// Simple collision helpers (re-exported from collision module)
fn ball_outer_wall_collision(pos: Vec2, radius: f32) -> CollisionResult {
    super::collision::ball_outer_wall_collision(pos, radius, ARENA_OUTER_RADIUS)
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
            0 => 12 + wave * 2,  // Outer: 12-32 blocks
            1 => 10 + wave,      // Second: 10-22
            2 => 8 + wave / 2,   // Third: 8-14
            _ => 6 + wave / 3,   // Inner: 6-10
        };
        let num_blocks = base_blocks.min(28) as usize;
        
        // Layer style: packed (no gaps) or spaced (gaps)
        let packed = (layer_seed % 3) != 0; // ~67% packed, 33% spaced
        
        // Rotation: occasionally ONE layer rotates (wave 2+)
        // Use a better hash to decorrelate layers
        let rotation_hash = layer_seed
            .wrapping_mul(2654435761) // Golden ratio hash
            .wrapping_add(layer * 7919); // Prime offset per layer
        let rotation_roll = rotation_hash % 100;
        
        // ~20% chance per layer rotates, so usually 0-1 spinning rings
        let rotation_speed = if wave >= 2 && rotation_roll < 20 {
            let base_speed = 0.2 + (layer as f32) * 0.08; // Gentle rotation
            let direction = if (rotation_hash / 100) % 2 == 0 { 1.0 } else { -1.0 };
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
            if (block_seed % skip_chance) == 0 && wave > 1 {
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
                let width_mult = if (block_seed % 5) == 0 { 0.75 } else if (block_seed % 3) == 0 { 0.65 } else { 0.55 };
                let width = base_arc * width_mult;
                let gap = (base_arc - width) / 2.0;
                (width, gap)
            };
            
            let theta_start = theta + gap;
            let theta_end = theta_start + arc_width;
            
            // Block type based on wave and position (limit invincible per layer)
            let kind = determine_block_kind(wave, layer, i as u32, block_seed, num_blocks, invincible_in_layer);
            
            if kind == BlockKind::Invincible {
                invincible_in_layer += 1;
            }
            
            let hp = match kind {
                BlockKind::Armored => 2 + (wave / 5) as u8, // Armored gets tougher
                BlockKind::Explosive => 1,
                BlockKind::Invincible => 255, // Doesn't matter, can't be damaged
                BlockKind::Portal { .. } => 3, // 3 passes before breaking
                BlockKind::Jello => 2, // Takes 2 hits, wobbles each time
                _ => 1,
            };
            
            // Variable thickness for some blocks
            let thickness = if (block_seed % 11) == 0 && wave > 3 {
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
fn determine_block_kind(wave: u32, layer: u32, index: u32, seed: u32, layer_block_count: usize, invincible_in_layer: u32) -> super::state::BlockKind {
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
    let can_place_invincible = wave >= 5 
        && invincible_in_layer < max_invincible.min(2)
        && (index % 4) == 0; // Spread them out (every 4th slot eligible)
    
    if can_place_invincible && roll < 8 {
        return BlockKind::Invincible;
    }
    
    // Explosive blocks (wave 3+, outer layer only, ~12% chance)
    if wave >= 3 && layer == 0 && roll < 12 {
        return BlockKind::Explosive;
    }
    
    // Portal blocks (wave 4+, ~8% chance, not on innermost layer)
    if wave >= 4 && layer < 3 && roll >= 12 && roll < 20 {
        return BlockKind::Portal { pair_id: seed }; // pair_id for future pairing
    }
    
    // Jello blocks (wave 3+, ~10% chance, inner layers preferred)
    if wave >= 3 && layer >= 1 && roll >= 20 && roll < 30 {
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
    let armored_chance = armored_chance + (layer as u32 * 10);
    
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
