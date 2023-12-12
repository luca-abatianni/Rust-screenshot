#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::env;

use chrono;
use eframe::{egui::{self, Window, Ui, Rect, Sense, Pos2, Vec2, Shape, Stroke, Color32, PointerState, Image, load::SizedTexture}, App, emath::RectTransform};
use screenshots::Screen;
use device_query::{DeviceQuery, DeviceState, MouseState, Keycode, DeviceEvents};
use std::{thread, time::Duration};
use image::imageops;

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
    cropped_screenshot_raw: Option<image::RgbaImage>,
    cropped_screenshot_built: Option<egui_extras::RetainedImage>,
    save_directory: String,
    delay: u32,
    delay_enable: bool,
    is_taking: bool,
    taking_refreshes: u32,
    is_cropping: bool,
    crop_mouse_clicked: bool,
    crop_start_pos: Pos2,
    crop_end_pos: Pos2
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
            cropped_screenshot_raw: None,
            cropped_screenshot_built: None,
            save_directory: env::current_dir()
                .unwrap()
                .into_os_string()
                .into_string()
                .unwrap(),
            delay: 0,
            delay_enable: false,
            is_taking: false,
            taking_refreshes: 0,
            is_cropping: false,
            crop_mouse_clicked: false,
            crop_start_pos: Pos2::new(0.0, 0.0),
            crop_end_pos: Pos2::new(0.0, 0.0),
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

    fn crop_screenshot(&mut self) {
        let current_screen = self.get_current_screen().unwrap();

        println!("Cropping {:?}", current_screen);
        let width = self.crop_end_pos[0] - self.crop_start_pos[0];
        let height = self.crop_end_pos[1] - self.crop_start_pos[1];
        let image = current_screen.capture_area(self.crop_start_pos[0] as i32, self.crop_start_pos[1] as i32, width as u32, height as u32).unwrap();
        //let image = imageops::crop_imm(&self.screenshot_raw, self.crop_start_pos[0] as i32, self.crop_start_pos[1] as i32, width as u32, height as u32);
        self.cropped_screenshot_raw = Some(image);
        self.cropped_screenshot_built = self.get_cropped_render_result();

        self.is_cropping = false;
    }

    fn check_screenshot(&mut self) -> bool {
        match &self.screenshot_raw {
            Some(s) => return true, 
            None => return false,
        }
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

    fn get_cropped_render_result(&self) -> Option<egui_extras::RetainedImage> {
        let cropped_screenshot_raw = &self.cropped_screenshot_raw;
        match cropped_screenshot_raw {
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
            if !self.is_cropping {ui.heading("My top panel");}
            else {
                ui.horizontal(|ui| {
                    ui.label("Crop Screenshot");
                });
            }
        });
        if !self.is_cropping { egui::SidePanel::left("my_left_panel").show(ctx, |ui| {
            egui::ComboBox::from_label("Select display")
                // When created from a label the text will b shown on the side of the combobox
                .selected_text(format!(
                    "Screen {:?}: {:?}x{:?}",
                    self.get_current_screen().unwrap().display_info.id,
                    self.get_current_screen().unwrap().display_info.width,
                    self.get_current_screen().unwrap().display_info.height
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

            ui.add(egui::Separator::default());

            //frame.set_visible(!self.is_taking);
            if self.is_taking {
                //self.take_screenshot();
                //self.is_taking = false;
                self.taking_refreshes += 1;
            } else {
                self.taking_refreshes = 0;
            }

            if self.taking_refreshes > 1 {
                thread::sleep(Duration::from_millis(100));

                self.cropped_screenshot_built = None;
                self.cropped_screenshot_raw = None;

                self.take_screenshot();
                //println!("Screenshot taken!");
                self.is_taking = false;
                frame.set_visible(true);
                //println!("Visibile");
            }

            if ui.button("Take a screenshot").clicked() {
                frame.set_visible(false);
                //frame.set_minimized(true);
                self.is_taking = true;
                //println!("Setted invisible!");
                //self.is_taking = true;
                //frame.set_visible(false);
                //thread::sleep(Duration::from_millis(4000));
                //self.take_screenshot();
                //frame.set_visible(true);
            }

            if ui.button("Crop screenshot").clicked() && self.check_screenshot() {
                frame.set_maximized(true);
                self.is_cropping = true;
            }

            if ui.button("Cancel crop").clicked() {
                self.cropped_screenshot_built = None;
                self.cropped_screenshot_raw = None;
            }

            ui.add(egui::Separator::default());

            if ui.button("Select default save location").clicked() {
                //self.take_screenshot();
                match tinyfiledialogs::select_folder_dialog("Select default save location", "") {
                    Some(dir) => self.save_directory = dir,
                    None => {}
                }
            }
            ui.label(&self.save_directory);

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

            ui.add(egui::Separator::default());

            ui.checkbox(&mut self.delay_enable, "Enable delay");
            ui.set_enabled(self.delay_enable);
            ui.add(
                egui::DragValue::new(&mut self.delay)
                    .speed(0.1)
                    .prefix("Timer: ")
                    .suffix(" seconds"),
            );


            // let (response, painter) = ui.allocate_painter(egui::Vec2 { x: 200.0, y: 200.0 }, Sense::hover());
            // let to_screen = RectTransform::from_to(
            //     Rect::from_min_size(Pos2::ZERO, response.rect.size()),
            //     response.rect,
            // );

            // let first_point = Pos2 { x: 0.0, y: 0.0 };
            // let second_point = Pos2 { x: 200.0, y: 200.0 };
            // // Make the points relative to the "canvas"
            // let first_point_in_screen = to_screen.transform_pos(first_point);
            // let second_point_in_screen = to_screen.transform_pos(second_point);

            // painter.add(Shape::LineSegment {
            //     points: [first_point_in_screen, second_point_in_screen],
            //     stroke: Stroke {
            //         width: 10.0,
            //         color: Color32::BLUE,
            //     },
            // });

        }); }

        if self.is_cropping {
            ctx.input(|i| {
                if i.pointer.any_down() && !self.crop_mouse_clicked {
                    if let Some(pos) = i.pointer.latest_pos() {
                        self.crop_start_pos = i.pointer.latest_pos().unwrap();
                        self.crop_mouse_clicked = true;
                        println!("Crop start position: {:?} ", self.crop_start_pos);
                    }
                }
                if !i.pointer.any_down() && self.crop_mouse_clicked {
                    if let Some(pos) = i.pointer.latest_pos() {
                        self.crop_end_pos = i.pointer.latest_pos().unwrap();
                        self.crop_mouse_clicked = false;
                        self.crop_screenshot();
                        println!("Crop end position: {:?} ", self.crop_end_pos);
                    }
                }
            })
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            let s = &self.screenshot_built;
            let cropped_s = &self.cropped_screenshot_built;
            let scale_factor = self.get_current_screen().unwrap().display_info.scale_factor;

            //ui.add(egui::ImageButton::new();

            match cropped_s {
                Some(r) => {
                    if self.is_cropping {r.show_scaled(ui, 3.0);}
                    else {r.show_scaled(ui, 3.0/scale_factor);}
                }
                None => {
                    match s {
                        Some(r) => {
                            if self.is_cropping {r.show_scaled(ui, 0.99/scale_factor);}
                            else {r.show_scaled(ui, 0.9/scale_factor);}
                        }, 
                        None => {}
                    }
                }
            }

            //TODO add screenshot to ui.image after click
        });
    }
}


