#![cfg_attr(windows, windows_subsystem = "windows")]
use eframe::{egui, App};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use enigo::{Enigo, MouseButton, MouseControllable, Key as EnigoKey, KeyboardControllable};
use rdev::{listen, EventType, Key as RdevKey};
use rfd::MessageDialog;

// Define activation modes
#[derive(Clone, PartialEq, Debug)]
enum ActiveMode {
    None,
    Clicking,
    KeystrokeInjection,
}

#[derive(Clone)]
struct AppState {
    interval_ms: u64,
    active_mode: ActiveMode,
    last_action: Instant,
    status: String,
    log: String,
    key_to_inject: String,
    current_key_index: usize,
    current_key_display: String,
    parsed_keys: Vec<String>,
    hold_mode: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            interval_ms: 1000,
            active_mode: ActiveMode::None,
            last_action: Instant::now(),
            status: "Stopped".to_string(),
            log: String::new(),
            key_to_inject: "w, s".to_string(), 
            current_key_index: 0,
            current_key_display: String::new(),
            parsed_keys: vec!["w".to_string(), "s".to_string()],
            hold_mode: false,
        }
    }
}

// A helper struct to create UI buttons consistently
struct ButtonConfig {
    text: &'static str,
    color: egui::Color32,
    action: fn(&mut AppState, now: Instant),
}

impl AppState {
    // Helper to update state for a given mode
    fn set_mode(&mut self, mode: ActiveMode, status: &str, log_message: &str, now: Instant) {
        // Clone the mode for later comparison
        let mode_clone = mode.clone();
        self.active_mode = mode;
        self.status = status.to_string();
        self.log.push_str(log_message);
        self.last_action = now;
        
        // Clear key display if stopping
        if mode_clone == ActiveMode::None {
            self.current_key_display = String::new();
        }
        // Set initial key display if starting keystroke injection
        else if mode_clone == ActiveMode::KeystrokeInjection && !self.parsed_keys.is_empty() {
            self.current_key_display = self.parsed_keys[0].clone();
        }
    }
    
    // Parse key sequence from input
    fn parse_key_sequence(&mut self) {
        self.parsed_keys = self.key_to_inject
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }
}

struct AutoClickerApp {
    state: Arc<Mutex<AppState>>,
    next_repaint: Instant,
    is_running: Arc<AtomicBool>,
}

impl App for AutoClickerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = Instant::now();
        
        if let Ok(mut state) = self.state.lock() {
            egui::CentralPanel::default().show(ctx, |ui| {
                // Header section
                ui.heading("Rust Auto Clicker");
                ui.horizontal(|ui| {
                    ui.label("Interval (ms):");
                    ui.add(egui::DragValue::new(&mut state.interval_ms).clamp_range(1..=10_000));
                    ui.checkbox(&mut state.hold_mode, "Hold Mode").on_hover_text("When enabled, the action key/button will be held down continuously instead of once per interval.");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format!("Status: {}", state.status));
                    });
                });
                
                // Log area
                ui.add_space(5.0);
                self.show_log_area(ui, &mut state);
                
                // Current key display
                if !state.current_key_display.is_empty() && state.active_mode == ActiveMode::KeystrokeInjection {
                    ui.horizontal(|ui| {
                        ui.label("Current key:");
                        ui.strong(&state.current_key_display);
                    });
                } else {
                    ui.add_space(5.0);
                }
                
                // Control buttons section
                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.heading("Mouse Clicking");
                        self.create_action_button(ui, &mut state, now, ButtonConfig {
                            text: "Start Clicking (F6)",
                            color: egui::Color32::from_rgb(0, 180, 255),
                            action: |state, now| {
                                state.set_mode(
                                    ActiveMode::Clicking,
                                    "Clicking...",
                                    "Started clicking! (F6)\n",
                                    now
                                );
                            },
                        });
                        
                        ui.add_space(5.0);
                        ui.heading("Keystroke Injection");
                        
                        // Key input field
                        ui.horizontal(|ui| {
                            ui.label("Keys:");
                            if ui.text_edit_singleline(&mut state.key_to_inject)
                                .on_hover_text("Enter keys separated by commas (e.g., 'w, s, d' or 'space, enter')")
                                .changed() 
                            {
                                state.parse_key_sequence();
                            }
                        });
                        
                        self.create_action_button(ui, &mut state, now, ButtonConfig {
                            text: "Start Keystroke Injection (F5)",
                            color: egui::Color32::from_rgb(0, 180, 255),
                            action: |state, now| {
                                if !state.parsed_keys.is_empty() {
                                    state.set_mode(
                                        ActiveMode::KeystrokeInjection,
                                        "Injecting keystrokes...",
                                        &format!("Started injecting keys '{}' (F5)\n", state.key_to_inject),
                                        now
                                    );
                                    state.current_key_index = 0;
                                } else {
                                    state.log.push_str("Cannot inject empty key sequence!\n");
                                }
                            },
                        });
                        
                        ui.add_space(5.0);
                        self.create_action_button(ui, &mut state, now, ButtonConfig {
                            text: "Stop All (F7)",
                            color: egui::Color32::from_rgb(255, 100, 100),
                            action: |state, now| {
                                state.set_mode(
                                    ActiveMode::None,
                                    "Stopped",
                                    "Stopped all actions\n",
                                    now
                                );
                            },
                        });
                    });
                });
                
                // Footer
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    ui.small("Note: Auto clicker works in background. Hotkeys: F5=Keys, F6=Click, F7=Stop");
                });
            });
        }
        
        // Maintain UI responsiveness at 60fps
        if now >= self.next_repaint {
            ctx.request_repaint_after(Duration::from_millis(16));
            self.next_repaint = now + Duration::from_millis(16);
        }
    }
    
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.is_running.store(false, Ordering::SeqCst);
    }
}

impl AutoClickerApp {
    // Helper to create consistent action buttons
    fn create_action_button(&self, ui: &mut egui::Ui, state: &mut AppState, now: Instant, config: ButtonConfig) {
        egui::Frame::none()
            .stroke(egui::Stroke::new(2.0, config.color))
            .show(ui, |ui| {
                if ui.add_sized(
                    [ui.available_width(), 40.0],
                    egui::Button::new(config.text)
                ).clicked() {
                    (config.action)(state, now);
                }
            });
    }
    
    // Helper to create the log area
    fn show_log_area(&self, ui: &mut egui::Ui, state: &mut AppState) {
        egui::Frame::none()
            .fill(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 100))
            .stroke(egui::Stroke::new(2.0, egui::Color32::GRAY))
            .show(ui, |ui| {
                let available_height = 200.0;
                egui::ScrollArea::vertical()
                    .max_height(available_height)
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        ui.set_min_height(available_height);
                        ui.style_mut().visuals.override_text_color = Some(egui::Color32::WHITE);
                        ui.add(
                            egui::TextEdit::multiline(&mut state.log)
                                .desired_width(ui.available_width())
                                .font(egui::TextStyle::Monospace)
                                .interactive(false)
                        );
                    });
            });
    }
    
    // Helper for special key handling
    fn send_key(enigo: &mut Enigo, key_str: &str) {
        // Map of special key names to their Enigo Key enum values
        match key_str.to_lowercase().as_str() {
            "space" => enigo.key_click(EnigoKey::Space),
            "enter" | "return" => enigo.key_click(EnigoKey::Return),
            "tab" => enigo.key_click(EnigoKey::Tab),
            "backspace" | "back" => enigo.key_click(EnigoKey::Backspace),
            "esc" | "escape" => enigo.key_click(EnigoKey::Escape),
            "up" => enigo.key_click(EnigoKey::UpArrow),
            "down" => enigo.key_click(EnigoKey::DownArrow),
            "left" => enigo.key_click(EnigoKey::LeftArrow),
            "right" => enigo.key_click(EnigoKey::RightArrow),
            "shift" => enigo.key_click(EnigoKey::Shift),
            "control" | "ctrl" => enigo.key_click(EnigoKey::Control),
            "alt" => enigo.key_click(EnigoKey::Alt),
            "win" | "windows" | "meta" => enigo.key_click(EnigoKey::Meta),
            "caps" | "capslock" => enigo.key_click(EnigoKey::CapsLock),
            "delete" | "del" => enigo.key_click(EnigoKey::Delete),
            "home" => enigo.key_click(EnigoKey::Home),
            "end" => enigo.key_click(EnigoKey::End),
            "pageup" | "pgup" => enigo.key_click(EnigoKey::PageUp),
            "pagedown" | "pgdn" => enigo.key_click(EnigoKey::PageDown),
            _ => if let Some(c) = key_str.chars().next() {
                enigo.key_click(EnigoKey::Layout(c));
            },
        }
    }
}

#[derive(PartialEq, Clone)]
enum ActionType {
    Click,
    KeyPress(String),
}

fn main() {
    // Initialize application state
    let mut app_state = AppState::default();
    app_state.parse_key_sequence();
    let state = Arc::new(Mutex::new(app_state));
    
    // Thread control flag
    let is_running = Arc::new(AtomicBool::new(true));
    
    // Start the background threads
    start_hotkey_thread(Arc::clone(&state), Arc::clone(&is_running));
    start_action_thread(Arc::clone(&state), Arc::clone(&is_running));
    
    // Create and run the app
    let app = AutoClickerApp { 
        state,
        next_repaint: Instant::now(),
        is_running,
    };
    
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 550.0]),
        ..Default::default()
    };
    
    if let Err(e) = eframe::run_native(
        "Rust Auto Clicker",
        native_options,
        Box::new(|_cc| Box::new(app)),
    ) {
        MessageDialog::new()
            .set_title("Auto Clicker Error")
            .set_description(&format!("Failed to start GUI: {e}"))
            .show();
    }
}

// Start the hotkey listener thread
fn start_hotkey_thread(state: Arc<Mutex<AppState>>, is_running: Arc<AtomicBool>) {
    let state_err = Arc::clone(&state);
    
    thread::spawn(move || {
        let result = listen(move |event| {
            if !is_running.load(Ordering::SeqCst) {
                return;
            }
            
            if let EventType::KeyPress(key) = event.event_type {
                if let Ok(mut state) = state.lock() {
                    let now = Instant::now();
                    match key {
                        RdevKey::F5 => {
                            if !state.parsed_keys.is_empty() {
                                // Store key_to_inject in a temporary variable before calling set_mode
                                let keys = state.key_to_inject.clone();
                                let log_message = format!("Started injecting keys '{}' (F5)\n", keys);
                                state.set_mode(
                                    ActiveMode::KeystrokeInjection,
                                    "Injecting keystrokes...",
                                    &log_message,
                                    now
                                );
                                state.current_key_index = 0;
                            } else {
                                state.log.push_str("Cannot inject empty key sequence!\n");
                            }
                        },
                        RdevKey::F6 => {
                            state.set_mode(
                                ActiveMode::Clicking,
                                "Clicking...",
                                "Started clicking! (F6)\n",
                                now
                            );
                        },
                        RdevKey::F7 => {
                            state.set_mode(
                                ActiveMode::None,
                                "Stopped",
                                "Stopped all actions! (F7)\n",
                                now
                            );
                        },
                        _ => {}
                    }
                }
            }
        });
        
        if let Err(e) = result {
            if let Ok(mut state) = state_err.lock() {
                state.log.push_str(&format!("Hotkey listener error: {:?}\n", e));
            }
        }
    });
}

// Start the action thread that performs clicks and key presses
fn start_action_thread(state: Arc<Mutex<AppState>>, is_running: Arc<AtomicBool>) {
    thread::spawn(move || {
        let mut enigo = Enigo::new();
        let mut next_action_time = Instant::now();
        
        // State to track currently held action and when it should be released
        let mut currently_held_action: Option<ActionType> = None;
        let mut release_time: Option<Instant> = None;
        
        while is_running.load(Ordering::SeqCst) {
            let now = Instant::now();
            
            let mut release_held_action_type: Option<ActionType> = None;
            let mut action_to_perform_this_loop: Option<ActionType> = None;
            
            // Check if a held action should be released based on time
            if let Some(r_time) = release_time {
                if now >= r_time {
                    if let Some(held) = currently_held_action.take() {
                        release_held_action_type = Some(held);
                        release_time = None; // Clear release time after scheduling release
                    }
                }
            }
            
            { // Scope for state lock
                if let Ok(mut state) = state.lock() {
                    let current_mode = state.active_mode.clone();
                    let hold_mode_active = state.hold_mode;
                    let interval = Duration::from_millis(state.interval_ms);
                    
                    match &current_mode {
                        ActiveMode::None => {
                            // If stopped, release anything being held
                            if let Some(held) = currently_held_action.take() {
                                release_held_action_type = Some(held);
                                release_time = None;
                            }
                        },
                        ActiveMode::Clicking => {
                            if hold_mode_active {
                                // Start hold if not currently holding
                                if currently_held_action.is_none() {
                                     // Release previous if any before starting new hold
                                    if let Some(held) = currently_held_action.take() {
                                        release_held_action_type = Some(held);
                                    }
                                    currently_held_action = Some(ActionType::Click);
                                    action_to_perform_this_loop = Some(ActionType::Click); // Indicate mouse down
                                    release_time = Some(now + interval);
                                }
                                // If already holding, do nothing until release_time
                            } else { // Non-hold clicking
                                // Release if hold was previously active
                                if let Some(held) = currently_held_action.take() {
                                    release_held_action_type = Some(held);
                                    release_time = None;
                                }
                                if now >= next_action_time {
                                    action_to_perform_this_loop = Some(ActionType::Click); // Indicate mouse click
                                    next_action_time = now + interval;
                                }
                            }
                        },
                        ActiveMode::KeystrokeInjection => {
                            if state.parsed_keys.is_empty() {
                                if let Some(held) = currently_held_action.take() {
                                    release_held_action_type = Some(held);
                                    release_time = None;
                                }
                            } else if hold_mode_active { // Hold keystroke
                                // Start hold if not currently holding a key or if the key needs to change
                                let idx = state.current_key_index % state.parsed_keys.len();
                                let key = state.parsed_keys[idx].clone();
                                let next_key_action = ActionType::KeyPress(key.clone());
                                
                                if currently_held_action != Some(next_key_action.clone()) {
                                    // Start holding the next key, releasing previous if any
                                     if let Some(held) = currently_held_action.take() {
                                        release_held_action_type = Some(held);
                                    }
                                    currently_held_action = Some(next_key_action.clone());
                                    action_to_perform_this_loop = Some(next_key_action); // Indicate key down
                                    release_time = Some(now + interval);
                                    // Advance index ONLY when successfully starting to hold a new key
                                    state.current_key_index = (idx + 1) % state.parsed_keys.len();
                                }
                                // If already holding the correct key, do nothing until release_time
                                
                                // Update the display even in hold mode
                                state.current_key_display = key;
                            } else { // Non-hold keystroke
                                // Release if hold was previously active
                                if let Some(held) = currently_held_action.take() {
                                    release_held_action_type = Some(held);
                                    release_time = None;
                                }
                                if now >= next_action_time {
                                    let idx = state.current_key_index % state.parsed_keys.len();
                                    let key = state.parsed_keys[idx].clone();
                                    state.current_key_display = key.clone();
                                    state.current_key_index = (idx + 1) % state.parsed_keys.len();
                                    action_to_perform_this_loop = Some(ActionType::KeyPress(key)); // Indicate key click
                                    next_action_time = now + interval;
                                }
                            }
                        }
                    }
                     // In non-hold mode, ensure next_action_time is in the future unless an action was just performed
                     // This logic is largely replaced by the next_action_time update within the non-hold blocks
                }
            } // End of state lock scope
            
            // Perform release outside of lock
            if let Some(action_type) = release_held_action_type {
                match action_type {
                    ActionType::Click => enigo.mouse_up(MouseButton::Left),
                    ActionType::KeyPress(key_str) => {
                        if let Some(key) = map_key_str_to_enigo_key(&key_str) {
                            enigo.key_up(key);
                        }
                    },
                }
            }

            // Perform action outside of lock
            if let Some(action_type) = action_to_perform_this_loop {
                 match action_type {
                     ActionType::Click => {
                         // In hold mode, this is mouse_down
                         // In non-hold mode, this is mouse_click (handled below)
                          if currently_held_action.is_some() { // Check if we are starting a hold
                              enigo.mouse_down(MouseButton::Left);
                         } else { // Otherwise, it's a single click
                              enigo.mouse_click(MouseButton::Left);
                         }
                     },
                     ActionType::KeyPress(key_str) => {
                         // In hold mode, this is key_down
                         // In non-hold mode, this is key_click (handled below)
                          if currently_held_action.is_some() { // Check if we are starting a hold
                             if let Some(key) = map_key_str_to_enigo_key(&key_str) {
                                 enigo.key_down(key);
                             }
                         } else { // Otherwise, it's a single key click
                              AutoClickerApp::send_key(&mut enigo, &key_str);
                         }
                     },
                 }
            }
            
            // Add a small sleep to prevent busy-waiting and excessive CPU usage
            let sleep_duration = if currently_held_action.is_some() && release_time.is_some() {
                // If holding, sleep until the release time
                release_time.unwrap().saturating_duration_since(now).max(Duration::from_millis(1))
            } else if let Ok(_state) = state.lock(){
                 // If not holding, sleep until the next scheduled action time
                 next_action_time.saturating_duration_since(now).max(Duration::from_millis(1))
            } else {
                 // Default sleep if state lock fails or no action is scheduled
                 Duration::from_millis(10)
            };
             thread::sleep(sleep_duration);
        }
        
        // Ensure any held action is released on shutdown
        if let Some(action_type) = currently_held_action.take() {
             match action_type {
                 ActionType::Click => enigo.mouse_up(MouseButton::Left),
                 ActionType::KeyPress(key_str) => {
                     if let Some(key) = map_key_str_to_enigo_key(&key_str) {
                         enigo.key_up(key);
                     }
                 },
             }
         }
    });
}

// Add a helper function to map key strings to EnigoKey, needed for key_down/key_up
fn map_key_str_to_enigo_key(key_str: &str) -> Option<EnigoKey> {
    match key_str.to_lowercase().as_str() {
        "space" => Some(EnigoKey::Space),
        "enter" | "return" => Some(EnigoKey::Return),
        "tab" => Some(EnigoKey::Tab),
        "backspace" | "back" => Some(EnigoKey::Backspace),
        "esc" | "escape" => Some(EnigoKey::Escape),
        "up" => Some(EnigoKey::UpArrow),
        "down" => Some(EnigoKey::DownArrow),
        "left" => Some(EnigoKey::LeftArrow),
        "right" => Some(EnigoKey::RightArrow),
        "shift" => Some(EnigoKey::Shift),
        "control" | "ctrl" => Some(EnigoKey::Control),
        "alt" => Some(EnigoKey::Alt),
        "win" | "windows" | "meta" => Some(EnigoKey::Meta),
        "caps" | "capslock" => Some(EnigoKey::CapsLock),
        "delete" | "del" => Some(EnigoKey::Delete),
        "home" => Some(EnigoKey::Home),
        "end" => Some(EnigoKey::End),
        "pageup" | "pgup" => Some(EnigoKey::PageUp),
        "pagedown" | "pgdn" => Some(EnigoKey::PageDown),
        _ => if let Some(c) = key_str.chars().next() {
            // This handles single character keys
            Some(EnigoKey::Layout(c))
        } else { None },
    }
}

