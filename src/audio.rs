//! Audio system using Web Audio API
//!
//! Procedurally generated sound effects - no external files needed!

use web_sys::{AudioContext, GainNode, OscillatorNode, OscillatorType};

/// Sound effect types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundEffect {
    /// Ball hits paddle
    PaddleHit,
    /// Ball hits wall
    WallHit,
    /// Ball hits block (doesn't break)
    BlockHit,
    /// Block breaks - glass
    BlockBreakGlass,
    /// Block breaks - armored
    BlockBreakArmored,
    /// Block breaks - explosive
    BlockBreakExplosive,
    /// Block breaks - jello
    BlockBreakJello,
    /// Block breaks - crystal
    BlockBreakCrystal,
    /// Block breaks - electric
    BlockBreakElectric,
    /// Block breaks - portal
    BlockBreakPortal,
    /// Pickup collected
    PickupCollect,
    /// Ball lost to black hole
    BlackHoleConsume,
    /// Wave cleared
    WaveClear,
    /// Ball launched
    Launch,
    /// Game over
    GameOver,
    /// New high score
    HighScore,
}

/// Audio manager for the game
pub struct AudioManager {
    ctx: Option<AudioContext>,
    master_volume: f32,
    sfx_volume: f32,
    muted: bool,
}

impl Default for AudioManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioManager {
    pub fn new() -> Self {
        // Try to create audio context (may fail if not in secure context)
        let ctx = AudioContext::new().ok();
        if ctx.is_none() {
            log::warn!("Failed to create AudioContext - audio disabled");
        }
        Self {
            ctx,
            master_volume: 0.8,
            sfx_volume: 1.0,
            muted: false,
        }
    }

    /// Resume audio context (required after user gesture)
    pub fn resume(&self) {
        if let Some(ctx) = &self.ctx {
            let _ = ctx.resume();
        }
    }

    /// Set master volume (0.0 - 1.0)
    pub fn set_master_volume(&mut self, vol: f32) {
        self.master_volume = vol.clamp(0.0, 1.0);
    }

    /// Set SFX volume (0.0 - 1.0)
    pub fn set_sfx_volume(&mut self, vol: f32) {
        self.sfx_volume = vol.clamp(0.0, 1.0);
    }

    /// Mute/unmute all audio
    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }

    /// Get effective volume
    fn effective_volume(&self) -> f32 {
        if self.muted {
            0.0
        } else {
            self.master_volume * self.sfx_volume
        }
    }

    /// Play a sound effect
    pub fn play(&self, effect: SoundEffect) {
        let vol = self.effective_volume();
        if vol <= 0.0 {
            return;
        }

        let Some(ctx) = &self.ctx else { return };

        // Resume context if suspended (browsers require user gesture)
        if ctx.state() == web_sys::AudioContextState::Suspended {
            let _ = ctx.resume();
        }

        match effect {
            SoundEffect::PaddleHit => self.play_paddle_hit(ctx, vol),
            SoundEffect::WallHit => self.play_wall_hit(ctx, vol),
            SoundEffect::BlockHit => self.play_block_hit(ctx, vol),
            SoundEffect::BlockBreakGlass => self.play_glass_break(ctx, vol),
            SoundEffect::BlockBreakArmored => self.play_armored_break(ctx, vol),
            SoundEffect::BlockBreakExplosive => self.play_explosion(ctx, vol),
            SoundEffect::BlockBreakJello => self.play_jello_break(ctx, vol),
            SoundEffect::BlockBreakCrystal => self.play_crystal_break(ctx, vol),
            SoundEffect::BlockBreakElectric => self.play_electric_break(ctx, vol),
            SoundEffect::BlockBreakPortal => self.play_portal_break(ctx, vol),
            SoundEffect::PickupCollect => self.play_pickup(ctx, vol),
            SoundEffect::BlackHoleConsume => self.play_black_hole(ctx, vol),
            SoundEffect::WaveClear => self.play_wave_clear(ctx, vol),
            SoundEffect::Launch => self.play_launch(ctx, vol),
            SoundEffect::GameOver => self.play_game_over(ctx, vol),
            SoundEffect::HighScore => self.play_high_score(ctx, vol),
        }
    }

    // === Sound generators ===

    /// Create an oscillator with gain envelope
    fn create_osc(
        &self,
        ctx: &AudioContext,
        freq: f32,
        osc_type: OscillatorType,
    ) -> Option<(OscillatorNode, GainNode)> {
        let osc = ctx.create_oscillator().ok()?;
        let gain = ctx.create_gain().ok()?;

        osc.set_type(osc_type);
        osc.frequency().set_value(freq);
        osc.connect_with_audio_node(&gain).ok()?;
        gain.connect_with_audio_node(&ctx.destination()).ok()?;

        Some((osc, gain))
    }

    /// Paddle hit - solid thump
    fn play_paddle_hit(&self, ctx: &AudioContext, vol: f32) {
        let Some((osc, gain)) = self.create_osc(ctx, 150.0, OscillatorType::Sine) else {
            return;
        };
        let t = ctx.current_time();

        gain.gain().set_value_at_time(vol * 0.6, t).ok();
        gain.gain()
            .exponential_ramp_to_value_at_time(0.01, t + 0.1)
            .ok();
        osc.frequency().set_value_at_time(150.0, t).ok();
        osc.frequency()
            .exponential_ramp_to_value_at_time(60.0, t + 0.1)
            .ok();

        osc.start().ok();
        osc.stop_with_when(t + 0.15).ok();
    }

    /// Wall hit - higher ping
    fn play_wall_hit(&self, ctx: &AudioContext, vol: f32) {
        let Some((osc, gain)) = self.create_osc(ctx, 400.0, OscillatorType::Sine) else {
            return;
        };
        let t = ctx.current_time();

        gain.gain().set_value_at_time(vol * 0.3, t).ok();
        gain.gain()
            .exponential_ramp_to_value_at_time(0.01, t + 0.08)
            .ok();

        osc.start().ok();
        osc.stop_with_when(t + 0.1).ok();
    }

    /// Block hit (no break) - soft tap
    fn play_block_hit(&self, ctx: &AudioContext, vol: f32) {
        let Some((osc, gain)) = self.create_osc(ctx, 300.0, OscillatorType::Triangle) else {
            return;
        };
        let t = ctx.current_time();

        gain.gain().set_value_at_time(vol * 0.25, t).ok();
        gain.gain()
            .exponential_ramp_to_value_at_time(0.01, t + 0.05)
            .ok();

        osc.start().ok();
        osc.stop_with_when(t + 0.08).ok();
    }

    /// Glass break - crackling zap shatter
    fn play_glass_break(&self, ctx: &AudioContext, vol: f32) {
        let t = ctx.current_time();

        // Crackling frequency jumps
        if let Some((osc, gain)) = self.create_osc(ctx, 100.0, OscillatorType::Sawtooth) {
            gain.gain().set_value_at_time(vol * 0.35, t).ok();
            gain.gain()
                .exponential_ramp_to_value_at_time(0.01, t + 0.18)
                .ok();
            osc.frequency().set_value_at_time(100.0, t).ok();
            osc.frequency().set_value_at_time(3500.0, t + 0.01).ok();
            osc.frequency().set_value_at_time(200.0, t + 0.02).ok();
            osc.frequency().set_value_at_time(4000.0, t + 0.03).ok();
            osc.frequency().set_value_at_time(150.0, t + 0.04).ok();
            osc.frequency().set_value_at_time(3000.0, t + 0.05).ok();
            osc.frequency().set_value_at_time(100.0, t + 0.07).ok();
            osc.frequency().set_value_at_time(2500.0, t + 0.08).ok();
            osc.frequency().set_value_at_time(80.0, t + 0.1).ok();
            osc.frequency().set_value_at_time(2000.0, t + 0.12).ok();
            osc.frequency().set_value_at_time(50.0, t + 0.15).ok();
            osc.start().ok();
            osc.stop_with_when(t + 0.2).ok();
        }

        // High frequency sizzle
        if let Some((osc, gain)) = self.create_osc(ctx, 6000.0, OscillatorType::Square) {
            gain.gain().set_value_at_time(vol * 0.12, t).ok();
            gain.gain()
                .exponential_ramp_to_value_at_time(0.01, t + 0.1)
                .ok();
            osc.frequency().set_value_at_time(6000.0, t).ok();
            osc.frequency().set_value_at_time(8000.0, t + 0.02).ok();
            osc.frequency().set_value_at_time(5000.0, t + 0.04).ok();
            osc.frequency().set_value_at_time(7000.0, t + 0.06).ok();
            osc.start().ok();
            osc.stop_with_when(t + 0.12).ok();
        }

        // Bass thump
        if let Some((osc, gain)) = self.create_osc(ctx, 60.0, OscillatorType::Sine) {
            gain.gain().set_value_at_time(vol * 0.3, t).ok();
            gain.gain()
                .exponential_ramp_to_value_at_time(0.01, t + 0.1)
                .ok();
            osc.start().ok();
            osc.stop_with_when(t + 0.12).ok();
        }
    }

    /// Armored break - deep metallic clang
    fn play_armored_break(&self, ctx: &AudioContext, vol: f32) {
        let t = ctx.current_time();

        // Deep bass impact
        if let Some((osc, gain)) = self.create_osc(ctx, 80.0, OscillatorType::Sine) {
            gain.gain().set_value_at_time(vol * 0.5, t).ok();
            gain.gain()
                .exponential_ramp_to_value_at_time(0.01, t + 0.25)
                .ok();
            osc.frequency().set_value_at_time(80.0, t).ok();
            osc.frequency()
                .exponential_ramp_to_value_at_time(40.0, t + 0.2)
                .ok();
            osc.start().ok();
            osc.stop_with_when(t + 0.3).ok();
        }

        // Metallic clang - lower frequencies
        if let Some((osc, gain)) = self.create_osc(ctx, 400.0, OscillatorType::Square) {
            gain.gain().set_value_at_time(vol * 0.25, t).ok();
            gain.gain()
                .exponential_ramp_to_value_at_time(0.01, t + 0.2)
                .ok();
            osc.frequency().set_value_at_time(400.0, t).ok();
            osc.frequency().set_value_at_time(300.0, t + 0.05).ok();
            osc.frequency().set_value_at_time(200.0, t + 0.1).ok();
            osc.start().ok();
            osc.stop_with_when(t + 0.25).ok();
        }

        // Mid resonance for body
        if let Some((osc, gain)) = self.create_osc(ctx, 250.0, OscillatorType::Triangle) {
            gain.gain().set_value_at_time(vol * 0.2, t).ok();
            gain.gain()
                .exponential_ramp_to_value_at_time(0.01, t + 0.15)
                .ok();
            osc.start().ok();
            osc.stop_with_when(t + 0.2).ok();
        }
    }

    /// Explosion - boom!
    fn play_explosion(&self, ctx: &AudioContext, vol: f32) {
        let Some((osc, gain)) = self.create_osc(ctx, 100.0, OscillatorType::Sawtooth) else {
            return;
        };
        let t = ctx.current_time();

        gain.gain().set_value_at_time(vol * 0.5, t).ok();
        gain.gain()
            .exponential_ramp_to_value_at_time(0.01, t + 0.4)
            .ok();
        osc.frequency().set_value_at_time(100.0, t).ok();
        osc.frequency()
            .exponential_ramp_to_value_at_time(30.0, t + 0.4)
            .ok();

        osc.start().ok();
        osc.stop_with_when(t + 0.5).ok();

        // Add high frequency crack
        if let Some((osc2, gain2)) = self.create_osc(ctx, 1500.0, OscillatorType::Square) {
            gain2.gain().set_value_at_time(vol * 0.2, t).ok();
            gain2
                .gain()
                .exponential_ramp_to_value_at_time(0.01, t + 0.1)
                .ok();
            osc2.start().ok();
            osc2.stop_with_when(t + 0.15).ok();
        }
    }

    /// Jello break - wobbly boing
    fn play_jello_break(&self, ctx: &AudioContext, vol: f32) {
        let Some((osc, gain)) = self.create_osc(ctx, 400.0, OscillatorType::Sine) else {
            return;
        };
        let t = ctx.current_time();

        gain.gain().set_value_at_time(vol * 0.35, t).ok();
        gain.gain()
            .exponential_ramp_to_value_at_time(0.01, t + 0.3)
            .ok();

        // Wobble frequency
        osc.frequency().set_value_at_time(400.0, t).ok();
        osc.frequency().set_value_at_time(500.0, t + 0.05).ok();
        osc.frequency().set_value_at_time(350.0, t + 0.1).ok();
        osc.frequency().set_value_at_time(450.0, t + 0.15).ok();
        osc.frequency().set_value_at_time(300.0, t + 0.2).ok();

        osc.start().ok();
        osc.stop_with_when(t + 0.35).ok();
    }

    /// Crystal break - sparkly chime
    fn play_crystal_break(&self, ctx: &AudioContext, vol: f32) {
        // Multiple harmonics for shimmer
        for (i, freq) in [1200.0, 1800.0, 2400.0].iter().enumerate() {
            let delay = i as f64 * 0.02;
            if let Some((osc, gain)) = self.create_osc(ctx, *freq, OscillatorType::Sine) {
                let t = ctx.current_time() + delay;
                gain.gain().set_value_at_time(vol * 0.2, t).ok();
                gain.gain()
                    .exponential_ramp_to_value_at_time(0.01, t + 0.3)
                    .ok();
                osc.start_with_when(t).ok();
                osc.stop_with_when(t + 0.35).ok();
            }
        }
    }

    /// Electric break - deep humming zap
    fn play_electric_break(&self, ctx: &AudioContext, vol: f32) {
        let t = ctx.current_time();

        // Low frequency electrical hum (60Hz mains hum style)
        if let Some((osc, gain)) = self.create_osc(ctx, 60.0, OscillatorType::Sawtooth) {
            gain.gain().set_value_at_time(vol * 0.4, t).ok();
            gain.gain()
                .exponential_ramp_to_value_at_time(0.01, t + 0.3)
                .ok();
            // Slight wobble in the hum
            osc.frequency().set_value_at_time(60.0, t).ok();
            osc.frequency().set_value_at_time(65.0, t + 0.05).ok();
            osc.frequency().set_value_at_time(55.0, t + 0.1).ok();
            osc.frequency().set_value_at_time(70.0, t + 0.15).ok();
            osc.frequency().set_value_at_time(50.0, t + 0.2).ok();
            osc.start().ok();
            osc.stop_with_when(t + 0.35).ok();
        }

        // Mid-range buzzing zap
        if let Some((osc, gain)) = self.create_osc(ctx, 120.0, OscillatorType::Square) {
            gain.gain().set_value_at_time(vol * 0.25, t).ok();
            gain.gain()
                .exponential_ramp_to_value_at_time(0.01, t + 0.2)
                .ok();
            // Zappy jumps but staying low
            osc.frequency().set_value_at_time(120.0, t).ok();
            osc.frequency().set_value_at_time(400.0, t + 0.02).ok();
            osc.frequency().set_value_at_time(150.0, t + 0.04).ok();
            osc.frequency().set_value_at_time(350.0, t + 0.06).ok();
            osc.frequency().set_value_at_time(100.0, t + 0.1).ok();
            osc.frequency().set_value_at_time(300.0, t + 0.12).ok();
            osc.frequency().set_value_at_time(80.0, t + 0.15).ok();
            osc.start().ok();
            osc.stop_with_when(t + 0.25).ok();
        }

        // Harmonic buzz (180Hz - 3rd harmonic of 60Hz)
        if let Some((osc, gain)) = self.create_osc(ctx, 180.0, OscillatorType::Triangle) {
            gain.gain().set_value_at_time(vol * 0.2, t).ok();
            gain.gain()
                .exponential_ramp_to_value_at_time(0.01, t + 0.25)
                .ok();
            osc.start().ok();
            osc.stop_with_when(t + 0.3).ok();
        }

        // Sub bass punch
        if let Some((osc, gain)) = self.create_osc(ctx, 40.0, OscillatorType::Sine) {
            gain.gain().set_value_at_time(vol * 0.35, t).ok();
            gain.gain()
                .exponential_ramp_to_value_at_time(0.01, t + 0.15)
                .ok();
            osc.start().ok();
            osc.stop_with_when(t + 0.2).ok();
        }
    }

    /// Portal break - whoosh
    fn play_portal_break(&self, ctx: &AudioContext, vol: f32) {
        let Some((osc, gain)) = self.create_osc(ctx, 600.0, OscillatorType::Sine) else {
            return;
        };
        let t = ctx.current_time();

        gain.gain().set_value_at_time(0.01, t).ok();
        gain.gain()
            .linear_ramp_to_value_at_time(vol * 0.3, t + 0.1)
            .ok();
        gain.gain()
            .exponential_ramp_to_value_at_time(0.01, t + 0.4)
            .ok();
        osc.frequency().set_value_at_time(600.0, t).ok();
        osc.frequency()
            .exponential_ramp_to_value_at_time(200.0, t + 0.4)
            .ok();

        osc.start().ok();
        osc.stop_with_when(t + 0.5).ok();
    }

    /// Pickup collect - happy ding
    fn play_pickup(&self, ctx: &AudioContext, vol: f32) {
        for (i, freq) in [600.0, 800.0, 1000.0].iter().enumerate() {
            let delay = i as f64 * 0.08;
            if let Some((osc, gain)) = self.create_osc(ctx, *freq, OscillatorType::Sine) {
                let t = ctx.current_time() + delay;
                gain.gain().set_value_at_time(vol * 0.25, t).ok();
                gain.gain()
                    .exponential_ramp_to_value_at_time(0.01, t + 0.15)
                    .ok();
                osc.start_with_when(t).ok();
                osc.stop_with_when(t + 0.2).ok();
            }
        }
    }

    /// Black hole consume - ominous descend
    fn play_black_hole(&self, ctx: &AudioContext, vol: f32) {
        let Some((osc, gain)) = self.create_osc(ctx, 300.0, OscillatorType::Sine) else {
            return;
        };
        let t = ctx.current_time();

        gain.gain().set_value_at_time(vol * 0.4, t).ok();
        gain.gain()
            .exponential_ramp_to_value_at_time(0.01, t + 0.8)
            .ok();
        osc.frequency().set_value_at_time(300.0, t).ok();
        osc.frequency()
            .exponential_ramp_to_value_at_time(20.0, t + 0.8)
            .ok();

        osc.start().ok();
        osc.stop_with_when(t + 1.0).ok();
    }

    /// Wave clear - triumphant fanfare
    fn play_wave_clear(&self, ctx: &AudioContext, vol: f32) {
        for (i, freq) in [400.0, 500.0, 600.0, 800.0].iter().enumerate() {
            let delay = i as f64 * 0.1;
            if let Some((osc, gain)) = self.create_osc(ctx, *freq, OscillatorType::Triangle) {
                let t = ctx.current_time() + delay;
                gain.gain().set_value_at_time(vol * 0.3, t).ok();
                gain.gain()
                    .exponential_ramp_to_value_at_time(0.01, t + 0.4)
                    .ok();
                osc.start_with_when(t).ok();
                osc.stop_with_when(t + 0.5).ok();
            }
        }
    }

    /// Launch - whoosh up
    fn play_launch(&self, ctx: &AudioContext, vol: f32) {
        let Some((osc, gain)) = self.create_osc(ctx, 200.0, OscillatorType::Triangle) else {
            return;
        };
        let t = ctx.current_time();

        gain.gain().set_value_at_time(vol * 0.3, t).ok();
        gain.gain()
            .exponential_ramp_to_value_at_time(0.01, t + 0.2)
            .ok();
        osc.frequency().set_value_at_time(200.0, t).ok();
        osc.frequency()
            .exponential_ramp_to_value_at_time(600.0, t + 0.15)
            .ok();

        osc.start().ok();
        osc.stop_with_when(t + 0.25).ok();
    }

    /// Game over - sad descending
    fn play_game_over(&self, ctx: &AudioContext, vol: f32) {
        for (i, freq) in [400.0, 350.0, 300.0, 200.0].iter().enumerate() {
            let delay = i as f64 * 0.2;
            if let Some((osc, gain)) = self.create_osc(ctx, *freq, OscillatorType::Sine) {
                let t = ctx.current_time() + delay;
                gain.gain().set_value_at_time(vol * 0.3, t).ok();
                gain.gain()
                    .exponential_ramp_to_value_at_time(0.01, t + 0.3)
                    .ok();
                osc.start_with_when(t).ok();
                osc.stop_with_when(t + 0.4).ok();
            }
        }
    }

    /// High score - celebratory
    fn play_high_score(&self, ctx: &AudioContext, vol: f32) {
        for (i, freq) in [500.0, 600.0, 700.0, 800.0, 1000.0].iter().enumerate() {
            let delay = i as f64 * 0.08;
            if let Some((osc, gain)) = self.create_osc(ctx, *freq, OscillatorType::Triangle) {
                let t = ctx.current_time() + delay;
                gain.gain().set_value_at_time(vol * 0.25, t).ok();
                gain.gain()
                    .exponential_ramp_to_value_at_time(0.01, t + 0.25)
                    .ok();
                osc.start_with_when(t).ok();
                osc.stop_with_when(t + 0.3).ok();
            }
        }
    }
}
