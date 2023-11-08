#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::env;

use chrono;
use eframe::{egui, App};
use screenshots::Screen;
use std::{thread, time::Duration};

fn main() -> Result<(), eframe::Error> {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    let options = eframe::NativeOptions {
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
    screenshot_raw: Option<image::RgbaImage>,
    screenshot_built: Option<egui_extras::RetainedImage>,
    save_directory: String,
    delay: u32,
    delay_enable: bool,
    is_taking: bool,
    taking_refreshes: u32,
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
            screenshot_raw: None,
            screenshot_built: None,
            save_directory: env::current_dir()
                .unwrap()
                .into_os_string()
                .into_string()
                .unwrap(),
            delay: 0,
            delay_enable: false,
            is_taking: false,
            taking_refreshes: 0,
        }
    }
    fn get_screen_by_id(&self, id: u32) -> Option<&Screen> {
        self.screens.iter().find(|&d| d.display_info.id == id)
    }
    fn get_current_screen(&self) -> Option<&Screen> {
        self.get_screen_by_id(self.screen_current_id)
    }

    fn take_screenshot(&mut self) {
        let current_screen = self.get_current_screen().unwrap();

        println!("Capturing {:?}", current_screen);
        let image = current_screen.capture().unwrap();
        self.screenshot_raw = Some(image);
        self.screenshot_built = self.get_render_result();
    }

    fn get_render_result(&self) -> Option<egui_extras::RetainedImage> {
        let screenshot_raw = &self.screenshot_raw;
        match screenshot_raw {
            Some(s) => {
                let color_image = egui::ColorImage::from_rgba_unmultiplied(
                    [
                        s.width().try_into().unwrap(),
                        s.height().try_into().unwrap(),
                    ],
                    &s,
                );
                return Some(egui_extras::RetainedImage::from_color_image(
                    "0.png",
                    color_image,
                ));
            }
            None => return None,
        }
    }
}

impl App for MyApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        //println!("{:?}", frame.info());
        egui::TopBottomPanel::top("my_top_panel").show(ctx, |ui| {
            ui.heading("Screen Grabbing utility");
        });
        egui::SidePanel::left("my_left_panel").show(ctx, |ui| {
            egui::ComboBox::from_label("display")
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

            //frame.set_visible(!self.is_taking);
            if self.is_taking {
                //self.take_screenshot();
                //self.is_taking = false;
                self.taking_refreshes += 1;
            } else {
                self.taking_refreshes = 0;
            }

            if self.taking_refreshes > 0 {
                self.take_screenshot();
                self.is_taking = false;
                frame.set_visible(true)
            }

            if ui.button("Take a screenshot").clicked() {
                self.is_taking = true;
                frame.set_visible(false);
                //self.is_taking = true;
                //frame.set_visible(false);
                //thread::sleep(Duration::from_millis(4000));
                //self.take_screenshot();
                //frame.set_visible(true);
            }
            ui.label(&self.save_directory);
            if ui.button("Select default save location").clicked() {
                //self.take_screenshot();
                match tinyfiledialogs::select_folder_dialog("Select default save location", "") {
                    Some(dir) => self.save_directory = dir,
                    None => {}
                }
            }
            let _ = ui.button("Save");
            if ui.button("Save as").clicked() {
                //tinyfiledialogs::save_file_dialog("Save as", &self.save_directory);
                //TODO use save_file_dialog_with_filter
                println!(
                    "{}",
                    tinyfiledialogs::save_file_dialog_with_filter(
                        "Save as",
                        format!("{}/output", &self.save_directory).as_str(),
                        &["*.png", "*.jpg", "*.gif"],
                        "Image",
                    )
                    .unwrap_or("no_selection".to_string())
                );
            }

            let _ = ui.button("Copy To Clipboard");
            ui.checkbox(&mut self.delay_enable, "Enable delay");
            ui.set_enabled(self.delay_enable);
            ui.add(
                egui::DragValue::new(&mut self.delay)
                    .speed(0.1)
                    .prefix("Timer: ")
                    .suffix(" seconds"),
            );
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            let s = &self.screenshot_built;
            match s {
                Some(r) => {
                    r.show_scaled(ui, 0.3);
                }
                None => {}
            }

            //TODO add screenshot to ui.image after click
        });
    }
}
