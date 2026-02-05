//! Roto Pong entry point
//!
//! Handles platform-specific initialization and runs the game loop.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
mod wasm_game {
    use std::cell::RefCell;
    use std::rc::Rc;
    use wasm_bindgen::prelude::*;
    use web_sys::{HtmlCanvasElement, MouseEvent, TouchEvent};

    use roto_pong::consts::*;
    use roto_pong::renderer::SdfRenderState;
    use roto_pong::sim::{GameState, TickInput, tick};

    // JS binding for pointer lock
    #[wasm_bindgen(inline_js = "
        export function request_pointer_lock() {
            const canvas = document.getElementById('canvas');
            console.log('request_pointer_lock called, canvas:', canvas);
            if (canvas) {
                console.log('Requesting pointer lock...');
                const result = canvas.requestPointerLock();
                console.log('requestPointerLock result:', result);
                if (result && result.then) {
                    result.then(() => {
                        console.log('Pointer lock promise resolved');
                        console.log('pointerLockElement:', document.pointerLockElement);
                        console.log('Is canvas locked?', document.pointerLockElement === canvas);
                    }).catch(e => console.error('Pointer lock failed:', e));
                }
                // Also check immediately
                setTimeout(() => {
                    console.log('After 100ms - pointerLockElement:', document.pointerLockElement);
                }, 100);
            }
        }
        
        export function check_pointer_lock() {
            const el = document.pointerLockElement;
            console.log('Current pointerLockElement:', el);
            return el !== null;
        }
    ")]
    extern "C" {
        fn request_pointer_lock();
        fn check_pointer_lock() -> bool;
    }

    /// Game instance holding all state
    struct Game {
        state: GameState,
        render_state: Option<SdfRenderState>,
        accumulator: f32,
        last_time: f64,
        input: TickInput,
        canvas_center: (f32, f32),
        // FPS tracking
        frame_times: [f64; 60],
        frame_index: usize,
        fps: u32,
        // Track phase for auto-save
        last_phase: roto_pong::sim::GamePhase,
        // Pointer lock state
        pointer_locked: bool,
    }

    impl Game {
        fn new(seed: u64) -> Self {
            use roto_pong::sim::GamePhase;
            Self {
                state: GameState::new(seed),
                render_state: None,
                accumulator: 0.0,
                last_time: 0.0,
                input: TickInput::default(),
                canvas_center: (0.0, 0.0),
                frame_times: [0.0; 60],
                frame_index: 0,
                fps: 0,
                last_phase: GamePhase::Serve,
                pointer_locked: false,
            }
        }

        fn set_canvas_center(&mut self, w: f32, h: f32) {
            self.canvas_center = (w / 2.0, h / 2.0);
        }

        /// Convert mouse/touch position to paddle angle
        fn pos_to_angle(&self, x: f32, y: f32) -> f32 {
            let dx = x - self.canvas_center.0;
            let dy = -(y - self.canvas_center.1); // Negate Y (screen coords are flipped)
            dy.atan2(dx)
        }

        /// Run simulation ticks
        fn update(&mut self, dt: f32, time: f64) {
            let dt = dt.min(0.1);
            self.accumulator += dt;

            let mut substeps = 0;
            while self.accumulator >= SIM_DT && substeps < MAX_SUBSTEPS {
                let input = self.input.clone();
                tick(&mut self.state, &input, SIM_DT);
                self.accumulator -= SIM_DT;
                substeps += 1;

                // Clear one-shot inputs after processing
                self.input.launch = false;
                self.input.pause = false;
                self.input.skip_wave = false;
            }
            
            // Track frame times for FPS
            self.frame_times[self.frame_index] = time;
            self.frame_index = (self.frame_index + 1) % 60;
            
            // Calculate FPS from oldest to newest frame
            let oldest_idx = self.frame_index;
            let oldest_time = self.frame_times[oldest_idx];
            if oldest_time > 0.0 {
                let elapsed = time - oldest_time;
                if elapsed > 0.0 {
                    self.fps = (60000.0 / elapsed).round() as u32;
                }
            }
            
            // Auto-save on phase transitions
            use roto_pong::sim::GamePhase;
            let current_phase = self.state.phase;
            if current_phase != self.last_phase {
                // Save when entering Breather (wave cleared) or Paused
                if current_phase == GamePhase::Breather || current_phase == GamePhase::Paused {
                    self.save_game();
                }
                self.last_phase = current_phase;
            }
        }

        /// Render the current frame
        fn render(&mut self, time: f64) {
            if let Some(ref mut render_state) = self.render_state {
                match render_state.render(&self.state, time) {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost) => {
                        render_state.resize(render_state.size.0, render_state.size.1);
                    }
                    Err(wgpu::SurfaceError::OutOfMemory) => {
                        log::error!("Out of memory!");
                    }
                    Err(e) => log::warn!("Render error: {:?}", e),
                }
            }
        }

        /// Update HUD elements in DOM
        fn update_hud(&self) {
            use roto_pong::sim::GamePhase;
            
            let window = web_sys::window().unwrap();
            let document = window.document().unwrap();

            // Update score
            if let Some(el) = document.query_selector("#hud-score .hud-value").ok().flatten() {
                el.set_text_content(Some(&self.state.score.to_string()));
            }

            // Update lives
            if let Some(el) = document.query_selector("#hud-lives .hud-value").ok().flatten() {
                el.set_text_content(Some(&self.state.lives.to_string()));
            }

            // Update wave
            if let Some(el) = document.query_selector("#hud-wave .hud-value").ok().flatten() {
                el.set_text_content(Some(&(self.state.wave_index + 1).to_string()));
            }

            // Update FPS
            if let Some(el) = document.query_selector("#hud-fps .hud-value").ok().flatten() {
                el.set_text_content(Some(&self.fps.to_string()));
            }

            // Update combo (only show when 2+ for actual combo)
            if let Some(el) = document.get_element_by_id("hud-combo") {
                if self.state.combo > 1 {
                    let _ = el.set_attribute("class", "hud-item");
                    
                    // Update combo value
                    if let Some(val) = document.query_selector("#hud-combo .hud-value").ok().flatten() {
                        let old_text = val.text_content().unwrap_or_default();
                        let new_text = self.state.combo.to_string();
                        if old_text != new_text {
                            val.set_text_content(Some(&new_text));
                            // Trigger pop animation
                            let _ = el.set_attribute("class", "hud-item pop");
                        }
                    }
                    
                    // Update multiplier (1.1x at combo 2, up to 3.0x)
                    if let Some(mult) = document.query_selector("#hud-combo .multiplier").ok().flatten() {
                        let multiplier = (1.0 + (self.state.combo - 1) as f32 * 0.1).min(3.0);
                        mult.set_text_content(Some(&format!("x{:.1}", multiplier)));
                    }
                } else {
                    let _ = el.set_attribute("class", "hud-item hidden");
                }
            }

            // Show/hide serve prompt
            if let Some(el) = document.get_element_by_id("serve-prompt") {
                if self.state.phase == GamePhase::Serve {
                    let _ = el.set_attribute("class", "");
                } else {
                    let _ = el.set_attribute("class", "hidden");
                }
            }

            // Show/hide pause menu
            if let Some(el) = document.get_element_by_id("pause-menu") {
                if self.state.phase == GamePhase::Paused {
                    let _ = el.set_attribute("class", "");
                } else {
                    let _ = el.set_attribute("class", "hidden");
                }
            }

            // Show/hide game over
            if let Some(el) = document.get_element_by_id("game-over") {
                if self.state.phase == GamePhase::GameOver {
                    let _ = el.set_attribute("class", "");
                    // Update final stats
                    if let Some(score_el) = document.get_element_by_id("final-score") {
                        score_el.set_text_content(Some(&self.state.score.to_string()));
                    }
                    if let Some(wave_el) = document.get_element_by_id("final-wave") {
                        wave_el.set_text_content(Some(&(self.state.wave_index + 1).to_string()));
                    }
                    // Clear saved game on game over
                    clear_saved_game();
                } else {
                    let _ = el.set_attribute("class", "hidden");
                }
            }
        }
        
        /// Save game state to LocalStorage
        fn save_game(&self) {
            if let Ok(json) = serde_json::to_string(&self.state) {
                if let Some(storage) = web_sys::window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                {
                    let _ = storage.set_item("roto_pong_save", &json);
                    log::info!("Game saved (wave {})", self.state.wave_index + 1);
                }
            }
        }

        /// Reset game state for restart
        fn restart(&mut self, seed: u64) {
            self.state = GameState::new(seed);
            self.accumulator = 0.0;
            self.input = TickInput::default();
        }
        
        /// Load game state from saved data
        fn load_state(&mut self, state: GameState) {
            self.state = state;
            self.accumulator = 0.0;
            self.input = TickInput::default();
        }
    }
    
    /// Load saved game from LocalStorage
    fn load_saved_game() -> Option<GameState> {
        let storage = web_sys::window()?.local_storage().ok()??;
        let json = storage.get_item("roto_pong_save").ok()??;
        serde_json::from_str(&json).ok()
    }
    
    /// Clear saved game from LocalStorage
    fn clear_saved_game() {
        if let Some(storage) = web_sys::window()
            .and_then(|w| w.local_storage().ok())
            .flatten()
        {
            let _ = storage.remove_item("roto_pong_save");
            log::info!("Saved game cleared");
        }
    }

    pub async fn run() {
        console_error_panic_hook::set_once();
        console_log::init_with_level(log::Level::Info).expect("Failed to init logger");

        log::info!("Roto Pong starting...");

        let window = web_sys::window().expect("no window");
        let document = window.document().expect("no document");

        // Hide loading indicator
        if let Some(loading) = document.get_element_by_id("loading") {
            let _ = loading.set_attribute("class", "hidden");
        }

        let canvas: HtmlCanvasElement = document
            .get_element_by_id("canvas")
            .expect("no canvas")
            .dyn_into()
            .expect("not a canvas");

        // Set canvas size
        let dpr = window.device_pixel_ratio();
        let client_w = canvas.client_width();
        let client_h = canvas.client_height();
        let width = (client_w as f64 * dpr) as u32;
        let height = (client_h as f64 * dpr) as u32;
        canvas.set_width(width);
        canvas.set_height(height);

        // Initialize game
        let seed = js_sys::Date::now() as u64;
        let game = Rc::new(RefCell::new(Game::new(seed)));
        game.borrow_mut()
            .set_canvas_center(client_w as f32, client_h as f32);

        log::info!("Game initialized with seed: {}", seed);

        // Initialize WebGPU
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..Default::default()
        });

        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .expect("Failed to create surface");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to get adapter");

        log::info!("Using adapter: {:?}", adapter.get_info().name);

        let mut render_state =
            SdfRenderState::new(surface, &adapter, width, height).await;
        render_state.set_start_time(js_sys::Date::now());
        game.borrow_mut().render_state = Some(render_state);

        // Check for saved game
        let saved_game = load_saved_game();
        let has_save = saved_game.is_some();
        
        if let Some(ref save) = saved_game {
            // Show continue prompt
            if let Some(el) = document.get_element_by_id("continue-prompt") {
                let _ = el.set_attribute("class", "");
            }
            if let Some(el) = document.get_element_by_id("continue-wave") {
                el.set_text_content(Some(&(save.wave_index + 1).to_string()));
            }
            if let Some(el) = document.get_element_by_id("continue-score") {
                el.set_text_content(Some(&save.score.to_string()));
            }
            log::info!("Found saved game at wave {}", save.wave_index + 1);
        } else {
            // Generate initial wave for new game
            let mut g = game.borrow_mut();
            roto_pong::sim::generate_wave(&mut g.state);
        }

        // Set up input handlers
        setup_input_handlers(&canvas, game.clone());
        
        // Set up restart button
        setup_restart_button(game.clone());
        
        // Set up pause menu buttons
        setup_pause_menu(game.clone());
        
        // Set up continue prompt buttons
        setup_continue_prompt(game.clone(), saved_game);
        
        // Set up auto-pause on visibility change
        setup_auto_pause(game.clone());

        // Show HUD (unless we're showing continue prompt)
        if let Some(hud) = document.get_element_by_id("hud") {
            if !has_save {
                let _ = hud.set_attribute("class", "");
            }
        }

        // Start game loop
        request_animation_frame(game);

        log::info!("Roto Pong running!");
    }

    fn setup_input_handlers(canvas: &HtmlCanvasElement, game: Rc<RefCell<Game>>) {
        // Pointer lock change handler
        {
            let game = game.clone();
            let document = web_sys::window().unwrap().document().unwrap();
            let closure = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::Event| {
                let document = web_sys::window().unwrap().document().unwrap();
                let locked = document.pointer_lock_element().is_some();
                if locked {
                    log::info!("Pointer lock ACQUIRED");
                } else {
                    log::warn!("Pointer lock RELEASED");
                }
                game.borrow_mut().pointer_locked = locked;
            });
            let _ = document.add_event_listener_with_callback(
                "pointerlockchange",
                closure.as_ref().unchecked_ref(),
            );
            closure.forget();
        }
        
        // Visibility change - might cause lock release
        {
            let document = web_sys::window().unwrap().document().unwrap();
            let closure = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::Event| {
                log::warn!("Visibility changed - this can release pointer lock");
            });
            let _ = document.add_event_listener_with_callback(
                "visibilitychange",
                closure.as_ref().unchecked_ref(),
            );
            closure.forget();
        }

        // Pointer lock error handler
        {
            let document = web_sys::window().unwrap().document().unwrap();
            let closure = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::Event| {
                log::error!("Pointer lock error!");
            });
            let _ = document.add_event_listener_with_callback(
                "pointerlockerror",
                closure.as_ref().unchecked_ref(),
            );
            closure.forget();
        }

        // Mouse move - use movementX when pointer locked, otherwise absolute position
        {
            let game = game.clone();
            let canvas_clone = canvas.clone();
            let closure = Closure::<dyn FnMut(_)>::new(move |event: MouseEvent| {
                let mut g = game.borrow_mut();
                
                if g.pointer_locked {
                    // Pointer locked: use relative movement
                    let sensitivity = 0.075; // Radians per pixel
                    let delta = event.movement_x() as f32 * sensitivity;
                    let current = g.state.paddle.theta;
                    g.input.target_theta = Some(current + delta);
                } else {
                    // Normal mode: use absolute position
                    let w = canvas_clone.client_width() as f32;
                    let h = canvas_clone.client_height() as f32;
                    g.set_canvas_center(w, h);
                    let angle = g.pos_to_angle(event.offset_x() as f32, event.offset_y() as f32);
                    g.input.target_theta = Some(angle);
                }
            });
            let _ = canvas
                .add_event_listener_with_callback("mousemove", closure.as_ref().unchecked_ref());
            closure.forget();
        }

        // Mouse click - request pointer lock and launch
        {
            let game = game.clone();
            let canvas_clone = canvas.clone();
            let closure = Closure::<dyn FnMut(_)>::new(move |_event: MouseEvent| {
                let mut g = game.borrow_mut();
                g.input.launch = true;
                
                // Request pointer lock if not already locked
                if !g.pointer_locked {
                    drop(g); // Release borrow before async call
                    request_pointer_lock();
                }
            });
            let _ = canvas
                .add_event_listener_with_callback("mousedown", closure.as_ref().unchecked_ref());
            closure.forget();
        }

        // Touch move
        {
            let game = game.clone();
            let canvas_clone = canvas.clone();
            let closure = Closure::<dyn FnMut(_)>::new(move |event: TouchEvent| {
                event.prevent_default();
                if let Some(touch) = event.touches().get(0) {
                    let mut g = game.borrow_mut();
                    let w = canvas_clone.client_width() as f32;
                    let h = canvas_clone.client_height() as f32;
                    g.set_canvas_center(w, h);
                    let rect = canvas_clone.get_bounding_client_rect();
                    let x = touch.client_x() as f32 - rect.left() as f32;
                    let y = touch.client_y() as f32 - rect.top() as f32;
                    let angle = g.pos_to_angle(x, y);
                    g.input.target_theta = Some(angle);
                }
            });
            let _ = canvas
                .add_event_listener_with_callback("touchmove", closure.as_ref().unchecked_ref());
            closure.forget();
        }

        // Touch start (launch)
        {
            let game = game.clone();
            let canvas_clone = canvas.clone();
            let closure = Closure::<dyn FnMut(_)>::new(move |event: TouchEvent| {
                event.prevent_default();
                let mut g = game.borrow_mut();
                g.input.launch = true;
                if let Some(touch) = event.touches().get(0) {
                    let w = canvas_clone.client_width() as f32;
                    let h = canvas_clone.client_height() as f32;
                    g.set_canvas_center(w, h);
                    let rect = canvas_clone.get_bounding_client_rect();
                    let x = touch.client_x() as f32 - rect.left() as f32;
                    let y = touch.client_y() as f32 - rect.top() as f32;
                    let angle = g.pos_to_angle(x, y);
                    g.input.target_theta = Some(angle);
                }
            });
            let _ = canvas
                .add_event_listener_with_callback("touchstart", closure.as_ref().unchecked_ref());
            closure.forget();
        }

        // Keyboard
        {
            let game = game.clone();
            let window = web_sys::window().unwrap();
            let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::KeyboardEvent| {
                let mut g = game.borrow_mut();
                match event.key().as_str() {
                    " " | "Enter" => g.input.launch = true,
                    "Escape" => g.input.pause = true,
                    "+" | "=" => g.input.skip_wave = true, // Debug: skip to next wave
                    "i" | "I" => {
                        g.input.idle_mode = !g.input.idle_mode;
                        log::info!("Idle mode: {}", g.input.idle_mode);
                    }
                    _ => {}
                }
            });
            let _ = window
                .add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref());
            closure.forget();
        }
    }

    fn request_animation_frame(game: Rc<RefCell<Game>>) {
        let window = web_sys::window().unwrap();
        let closure = Closure::once(move |time: f64| {
            game_loop(game, time);
        });
        let _ = window.request_animation_frame(closure.as_ref().unchecked_ref());
        closure.forget();
    }

    fn game_loop(game: Rc<RefCell<Game>>, time: f64) {
        {
            let mut g = game.borrow_mut();

            // Calculate delta time
            let dt = if g.last_time > 0.0 {
                ((time - g.last_time) / 1000.0) as f32
            } else {
                SIM_DT
            };
            g.last_time = time;

            g.update(dt, time);
            g.render(time);
            g.update_hud();
        }

        request_animation_frame(game);
    }

    fn setup_restart_button(game: Rc<RefCell<Game>>) {
        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();

        if let Some(btn) = document.get_element_by_id("restart-btn") {
            let closure = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::MouseEvent| {
                let seed = js_sys::Date::now() as u64;
                let mut g = game.borrow_mut();
                g.restart(seed);
                
                // Regenerate initial wave
                roto_pong::sim::generate_wave(&mut g.state);
                
                // Clear any saved game
                clear_saved_game();
                
                log::info!("Game restarted with seed: {}", seed);
            });
            let _ = btn.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref());
            closure.forget();
        }
    }
    
    fn setup_pause_menu(game: Rc<RefCell<Game>>) {
        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();

        // Resume button
        if let Some(btn) = document.get_element_by_id("resume-btn") {
            let game = game.clone();
            let closure = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::MouseEvent| {
                game.borrow_mut().input.pause = true; // Toggle back to playing
            });
            let _ = btn.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref());
            closure.forget();
        }
        
        // Save & Quit button
        if let Some(btn) = document.get_element_by_id("save-quit-btn") {
            let closure = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::MouseEvent| {
                let g = game.borrow();
                g.save_game();
                // Reload page to show continue prompt
                if let Some(window) = web_sys::window() {
                    let _ = window.location().reload();
                }
            });
            let _ = btn.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref());
            closure.forget();
        }
    }
    
    fn setup_continue_prompt(game: Rc<RefCell<Game>>, saved_game: Option<GameState>) {
        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();

        // Continue button
        if let Some(btn) = document.get_element_by_id("continue-btn") {
            let game = game.clone();
            let saved = saved_game.clone();
            let closure = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::MouseEvent| {
                if let Some(ref state) = saved {
                    game.borrow_mut().load_state(state.clone());
                    log::info!("Loaded saved game at wave {}", state.wave_index + 1);
                }
                // Hide continue prompt, show HUD
                let document = web_sys::window().unwrap().document().unwrap();
                if let Some(el) = document.get_element_by_id("continue-prompt") {
                    let _ = el.set_attribute("class", "hidden");
                }
                if let Some(el) = document.get_element_by_id("hud") {
                    let _ = el.set_attribute("class", "");
                }
            });
            let _ = btn.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref());
            closure.forget();
        }
        
        // New Game button
        if let Some(btn) = document.get_element_by_id("new-game-btn") {
            let closure = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::MouseEvent| {
                // Clear saved game
                clear_saved_game();
                
                // Start fresh
                let seed = js_sys::Date::now() as u64;
                game.borrow_mut().restart(seed);
                roto_pong::sim::generate_wave(&mut game.borrow_mut().state);
                
                // Hide continue prompt, show HUD
                let document = web_sys::window().unwrap().document().unwrap();
                if let Some(el) = document.get_element_by_id("continue-prompt") {
                    let _ = el.set_attribute("class", "hidden");
                }
                if let Some(el) = document.get_element_by_id("hud") {
                    let _ = el.set_attribute("class", "");
                }
                
                log::info!("Started new game with seed: {}", seed);
            });
            let _ = btn.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref());
            closure.forget();
        }
    }
    
    fn setup_auto_pause(game: Rc<RefCell<Game>>) {
        use roto_pong::sim::GamePhase;
        
        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();

        // Visibility change (tab switch, minimize)
        {
            let game = game.clone();
            let document_clone = document.clone();
            let closure = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::Event| {
                if document_clone.visibility_state() == web_sys::VisibilityState::Hidden {
                    let mut g = game.borrow_mut();
                    // Auto-pause if playing
                    if g.state.phase == GamePhase::Playing || g.state.phase == GamePhase::Serve {
                        g.input.pause = true;
                        log::info!("Auto-paused (tab hidden)");
                    }
                }
            });
            let _ = document.add_event_listener_with_callback(
                "visibilitychange",
                closure.as_ref().unchecked_ref(),
            );
            closure.forget();
        }

        // Window blur (click outside)
        {
            let closure = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::FocusEvent| {
                let mut g = game.borrow_mut();
                if g.state.phase == GamePhase::Playing || g.state.phase == GamePhase::Serve {
                    g.input.pause = true;
                    log::info!("Auto-paused (window blur)");
                }
            });
            let _ = window.add_event_listener_with_callback("blur", closure.as_ref().unchecked_ref());
            closure.forget();
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub async fn wasm_main() {
    wasm_game::run().await;
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    env_logger::init();
    log::info!("Roto Pong (native) starting...");
    log::info!("Native mode requires winit integration - run with `trunk serve` for web version");

    // Run tests
    println!("\nRunning collision tests...");
    test_arc_collision();
}

#[cfg(target_arch = "wasm32")]
fn main() {
    // WASM entry point is wasm_main, this is just to satisfy the compiler
}

#[cfg(not(target_arch = "wasm32"))]
fn test_arc_collision() {
    use roto_pong::polar_to_cartesian;
    use roto_pong::sim::{ArcSegment, ball_arc_collision};
    use std::f32::consts::PI;

    let paddle = ArcSegment::new(360.0, 12.0, -PI / 2.0 - 0.2, -PI / 2.0 + 0.2);
    let ball_pos = polar_to_cartesian(357.0, -PI / 2.0);
    let ball_radius = 8.0;

    let result = ball_arc_collision(ball_pos, ball_radius, &paddle);
    assert!(result.hit, "Collision should be detected");
    println!("✓ Arc collision tests passed!");
}
