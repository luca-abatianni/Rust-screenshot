#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use chrono;
use eframe::{egui, App};
use screenshots::Screen;

fn main() -> Result<(), eframe::Error> {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(600.0, 200.0)),
        ..Default::default()
    };
    eframe::run_native(
        "rust-screenshot",
        options,
        Box::new(|cc| {
            // This gives us image support:
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Box::new(MyApp::new(cc))
        }),
    )
}

struct MyApp {
    screens: Vec<Screen>,
    screen_current_id: u32,
}

impl MyApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_visuals.
        // Restore app state using cc.storage (requires the "persistence" feature).
        // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that you can use
        // for e.g. egui::PaintCallback.
        MyApp {
            screens: Screen::all().unwrap(),
            screen_current_id: Screen::all().unwrap()[0].display_info.id,
        }
    }
    fn get_screen_by_id(&self, id: u32) -> Option<&Screen> {
        self.screens.iter().find(|&d| d.display_info.id == id)
    }
    fn get_current_screen(&self) -> Option<&Screen> {
        self.get_screen_by_id(self.screen_current_id)
    }
}

impl App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Screen Grabbing utility");
            egui::ComboBox::from_label(format!(
                "Currently selected enum: {:?}",
                self.get_current_screen().unwrap()
            ))
            // When created from a label the text will b shown on the side of the combobox
            .selected_text(format!(
                "{:?}",
                self.get_current_screen().unwrap().display_info
            )) // This is the currently selected option (in text form)
            .show_ui(ui, |ui| {
                // In this closure the various options can be added
                for option in &self.screens {
                    // The first parameter is a mutable reference to allow the choice to be modified when the user selects
                    // something else. The second parameter is the actual value of the option (to be compared with the currently)
                    // selected one to allow egui to highlight the correct label. The third parameter is the string to show.
                    ui.selectable_value(
                        &mut self.screen_current_id,
                        option.display_info.id,
                        format!("{:?}", option.display_info),
                    );
                }
            });
            if ui.button("Take a screenshot").clicked() {
                take_screenshot();
            }
        });
    }
}

fn take_screenshot() {
    let screens = Screen::all().unwrap();

    for screen in screens {
        println!("Capturing {screen:?}");
        let image = screen.capture().unwrap();
        image
            .save(format!(
                "target/{:?}.png",
                chrono::offset::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
            ))
            .unwrap();
    }
}
