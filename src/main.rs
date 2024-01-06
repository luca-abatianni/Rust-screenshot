#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::{env, borrow::Cow, collections::HashMap};
use minifb::{self, WindowOptions, ScaleMode};
use chrono::prelude::*;
use eframe::{egui::{self, Pos2, Key, Modifiers, KeyboardShortcut}, App};
use screenshots::Screen;
use std::{thread, time::Duration};
use arboard::{Clipboard, ImageData};
use image::imageops::FilterType::Nearest;

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
    save_extension: String,
    auto_save: bool,
    delay: u32,
    delay_enable: bool,
    is_taking: bool,
    taking_refreshes: u32,
    is_cropping: bool,
    crop_start_pos: Pos2,
    crop_end_pos: Pos2, 
    screenshot_shortcut: KeyboardShortcut,
    crop_shortcut: KeyboardShortcut,
    in_settings: bool
}

impl MyApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
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
            save_extension: String::from(".png"),
            auto_save: false,
            delay: 0,
            delay_enable: false,
            is_taking: false,
            taking_refreshes: 0,
            is_cropping: false,
            crop_start_pos: Pos2::new(0.0, 0.0),
            crop_end_pos: Pos2::new(0.0, 0.0),
            screenshot_shortcut: KeyboardShortcut { modifiers: Modifiers::CTRL, key: Key::S },
            crop_shortcut: KeyboardShortcut { modifiers: Modifiers::CTRL, key: Key::R }.to_owned(),
            in_settings: false
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

        if self.delay_enable && self.delay > 0 {
            thread::sleep(Duration::from_secs(self.delay as u64));
        }
            
        let image = current_screen.capture().unwrap();
        self.screenshot_raw = Some(image);
        self.screenshot_built = self.get_render_result();

        if self.auto_save {
            self.save_screenshot(None);
        }
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

        if self.auto_save {
            self.save_screenshot(None);
        }
    }

    fn check_screenshot(&mut self) -> bool {
        match &self.screenshot_raw {
            Some(_s) => return true, 
            None => return false,
        }
    }

    fn save_screenshot(&mut self, prefix: Option<String>) {
        let prefix = prefix.unwrap_or(format!("{}/rust_screenshot", &self.save_directory));
        println!("{}", prefix);
        match &self.cropped_screenshot_raw {
            Some(s) => s.save(format!("{}_{}{}", prefix, Utc::now().format("%d-%m-%Y_%H-%M-%S"), &self.save_extension)).unwrap(),
            None => {
                match &self.screenshot_raw {
                    Some(s) => s.save(format!("{}_{}{}", prefix, Utc::now().format("%d-%m-%Y_%H-%M-%S"), &self.save_extension)).unwrap(),
                    None => return
                }
            }
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

        //TOP PANEL;
        egui::TopBottomPanel::top("my_top_panel").show(ctx, |ui| {
            if !self.is_cropping {ui.heading("Rust-Screenshot");}
            else {
                ui.horizontal(|ui| {
                    ui.label("Crop Screenshot");
                });
            }
        });

        //LEFT PANEL (only if not cropping)
        egui::SidePanel::left("my_left_panel").show(ctx, |ui| {

            ui.label("DISPLAY");
            ui.add(egui::Separator::default());

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
            ui.label("SCREENSHOT");
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
            if ui.add_sized([280., 20.], egui::Button::new("TAKE A SCREENSHOT")).clicked() || ctx.input_mut(|i| i.consume_shortcut(&self.screenshot_shortcut)){
                frame.set_visible(false);
                self.is_taking = true;
            }

            if (ui.add_sized([280., 40.], egui::Button::new("✂ CROP SCREENSHOT")).clicked() || ctx.input_mut(|i| i.consume_shortcut(&self.crop_shortcut))) && self.check_screenshot() {
                self.is_cropping = true;

                let scale_factor = self.get_current_screen().unwrap().display_info.scale_factor as usize;
                let width;
                let height;
                match &self.cropped_screenshot_raw {
                    Some(_c) => { 
                        width = self.cropped_screenshot_built.as_ref().unwrap().width().clone() / scale_factor;
                        height = self.cropped_screenshot_built.as_ref().unwrap().height().clone() / scale_factor; 
                    },
                    None => {
                        width = self.screenshot_built.as_ref().unwrap().width().clone() / scale_factor;
                        height = self.screenshot_built.as_ref().unwrap().height().clone() / scale_factor; 
                    }
                };

                let resized_image: image::RgbaImage;
                match &self.cropped_screenshot_raw {
                    Some(_c) => { 
                        resized_image = image::imageops::resize(&self.cropped_screenshot_raw.as_ref().unwrap().clone(), width as u32, height as u32, Nearest);
                    },
                    None => {
                        resized_image = image::imageops::resize(&self.screenshot_raw.as_ref().unwrap().clone(), width as u32, height as u32, Nearest);
                    }
                };

                let mut buffer: Vec<u32> = vec![0; (width * height) as usize];
                println!("{} {}", width, height);

                resized_image.enumerate_pixels().for_each(|(x, y, pixel)| {
                    let offset = ((y as usize) * width + (x as usize)) as usize;
                    if offset < buffer.len() {
                        buffer[offset] = ((pixel[0] as u32) << 16) | ((pixel[1] as u32) << 8) | pixel[2] as u32;
                    }
                });

                let mut window = minifb::Window::new(
                    "Crop",
                    width,
                    height,
                    WindowOptions {
                        resize: false,
                        scale_mode: ScaleMode::Center,
                        ..WindowOptions::default()
                    }
                ).unwrap_or_else(|e| {
                    panic!("{}", e);
                });
                window.limit_update_rate(Some(std::time::Duration::from_micros(16600)));

                let original_buffer = buffer.clone();

                let mut mouse_pos_start = None;
                let mut mouse_pos_end = None;
                let mut pressed = false;

                let mut min_x = 0;
                let mut min_y = 0;
                let mut max_x = 0;
                let mut max_y = 0;

                while window.is_open() && self.is_cropping {
                    let mouse_pos_cur = window.get_mouse_pos(minifb::MouseMode::Pass).unwrap();
                    
                    if window.get_mouse_down(minifb::MouseButton::Left) {
                        if !pressed {
                            mouse_pos_start = Some(mouse_pos_cur);
                            pressed = true;
                        } else {
                            mouse_pos_end = Some(mouse_pos_cur);
                        }
                    } else if pressed {
                        pressed = false;
                    }

                    buffer.clone_from(&original_buffer);

                    if let (Some(rect_start), Some(rect_end)) = (mouse_pos_start, mouse_pos_end) {
                        min_x = ((rect_start.0).min(rect_end.0))as usize;
                        min_y = ((rect_start.1).min(rect_end.1)) as usize;
                        max_x = ((rect_start.0).max(rect_end.0)) as usize;
                        max_y = ((rect_start.1).max(rect_end.1)) as usize;

                        for x in min_x..=max_x {
                            buffer[(min_y * width + x) as usize] = 0xFFFFFF;
                            buffer[(max_y * width+ x) as usize] = 0xFFFFFF;
                        }
                        for y in min_y..=max_y {
                            buffer[(y * width + min_x) as usize] = 0xFFFFFF;
                            buffer[(y * width + max_x) as usize] = 0xFFFFFF;
                        }
                    }

                    if !pressed {
                        if let (Some(_rect_start), Some(_rect_end)) = (mouse_pos_start, mouse_pos_end) {
                            self.crop_start_pos = Pos2::new((min_x + 1) as f32, (min_y + 67) as f32);
                            self.crop_end_pos = Pos2::new((max_x - 1) as f32, (max_y + 65) as f32);
                            self.crop_screenshot();

                            mouse_pos_start = None;
                            mouse_pos_end = None;
                            min_x = 0;
                            min_y = 0;
                            max_x = 0;
                            max_y = 0;
                            self.is_cropping = false;
                        }
                    }

                    window
                        .update_with_buffer(&buffer, width, height)
                        .unwrap();
                }
            }

            if ui.add_sized([280., 20.], egui::Button::new("CANCEL CROP")).clicked(){
                self.cropped_screenshot_built = None;
                self.cropped_screenshot_raw = None;
            }

            ui.add(egui::Separator::default());
            ui.label("SAVE");
            ui.add(egui::Separator::default());


            if ui.add_sized([280., 20.], egui::Button::new("SELECT DEFAULT SAVE LOCATION")).clicked() {
                let fd = rfd::FileDialog::new();
                match fd.pick_folder() {
                    Some(path) => self.save_directory = path.into_os_string().into_string().unwrap(),
                    None => (),
                }
            }
            ui.label(format!("Current folder: {}", &self.save_directory));

       

            if ui.add_sized([280., 20.], egui::Button::new("SAVE")).clicked() {
                self.save_screenshot(None);
            }

            if ui.add_sized([280., 20.], egui::Button::new("SAVE AS")).clicked() {
                //self.save_screenshot(Some( path.into_os_string().into_string().unwrap()))
                let fd = rfd::FileDialog::new();
                match fd.save_file() {
                    Some(path) => {self.save_screenshot(Some(path.into_os_string().into_string().unwrap())) },
                    None => (),
                }
            }

            // Il crate screenshots restituisce oggetti di tipo ImageBuffer<Rgba<u8>, Vec<u8>>
            // Il crate arboard restituisce oggetti di tipo ImageData { pub width: usize, pub height: usize, pub bytes: Cow<'a, [u8]>}
            // C'è bisogno di convertire manualmente l'ImageBuffer in ImageData perchè non c'è un cast diretto
            if ui.add_sized([280., 20.], egui::Button::new("COPY TO CLIPBOARD")).clicked() {

                let mut clipboard = Clipboard::new().unwrap();

                let working_screenshot = match &self.cropped_screenshot_raw {
                    Some(_c) => self.cropped_screenshot_raw.as_ref(),
                    None => self.screenshot_raw.as_ref(),
                };
                
                match working_screenshot {
                    Some(ws) => {
                        let img = ImageData{
                            width: ws.width() as usize,
                            height:  ws.height() as usize,
                            bytes: Cow::from(ws.as_raw())
                        };
                        match clipboard.set_image(img) {
                            Ok(_) => (),
                            error => println!("Error while copying to clipboard! -> {:?}", error)
                        }
                    },
                    None => println!("No screenshot to save to clipboard")
                }


            }

            egui::ComboBox::from_label("Select extension")
            .selected_text(format!("{}", &self.save_extension))
            .show_ui(ui, |ui| {
                let options: [String; 3] = [
                    String::from(".png"),
                    String::from(".jpeg"),
                    String::from(".gif")
                ];

                for option in &options {
                    ui.selectable_value(
                        &mut self.save_extension,
                        option.clone(),
                        format!("{}", option)
                    );
                }
            });

            ui.checkbox(&mut self.auto_save, "Auto-save screenshot");


            ui.add(egui::Separator::default());
            ui.label("DELAY");
            ui.add(egui::Separator::default());

            ui.checkbox(&mut self.delay_enable, "Enable delay");
            
            ui.add_sized([280., 20.],
                egui::DragValue::new(&mut self.delay)
                    .speed(0.1)
                    .clamp_range(0..=30)
                    .prefix("Timer: ")
                    .suffix(" seconds")
            );

            ui.add(egui::Separator::default());
            ui.label("SETTINGS");
            ui.add(egui::Separator::default());

            if ui.add_sized([280., 20.], egui::Button::new("EDIT SETTINGS")).clicked() {
                self.in_settings = true;
            };

        });

        if self.in_settings {
            let modifiers_options = HashMap::from([
                (Modifiers::ALT, "ALT"),
                (Modifiers::CTRL, "CTRL"),
                (Modifiers::SHIFT, "SHIFT"),
                (Modifiers::COMMAND, "COMMAND"),
            ]);

            egui::Window::new("Settings").show(ctx, |ui| {

                egui::ComboBox::from_label("First screenshot button")
                .selected_text(format!("{:?}", modifiers_options.get(&self.screenshot_shortcut.modifiers).unwrap()))
                .show_ui(ui, |ui| {
                    let options: [Modifiers; 4] = [
                        Modifiers::ALT, Modifiers::CTRL, Modifiers::SHIFT, Modifiers::COMMAND
                    ];

                    for option in options {
                        ui.selectable_value(
                            &mut self.screenshot_shortcut.modifiers,
                            option,
                            format!("{:?}", modifiers_options.get(&option).unwrap())
                        );
                    }
                });

                egui::ComboBox::from_label("Second screenshot button")
                    .selected_text(format!("{:?}", &self.screenshot_shortcut.key))
                    .show_ui(ui, |ui| {
                        let options: [Key; 4] = [
                            Key::A, Key::S, Key::Q, Key::W
                        ];
    
                        for option in &options {
                            ui.selectable_value(
                                &mut self.screenshot_shortcut.key,
                                option.clone(),
                                format!("{:?}", option)
                            );

                        }
                    });
                

                
                egui::ComboBox::from_label("First crop button")
                    .selected_text(format!("{:?}", modifiers_options.get(&self.crop_shortcut.modifiers).unwrap()))
                    .show_ui(ui, |ui| {
                        let options: [Modifiers; 4] = [
                            Modifiers::ALT, Modifiers::CTRL, Modifiers::SHIFT, Modifiers::COMMAND
                        ];

                        for option in options {
                            ui.selectable_value(
                                &mut self.crop_shortcut.modifiers,
                                option,
                                format!("{:?}", modifiers_options.get(&option).unwrap())
                            );
                        }
                    });

                egui::ComboBox::from_label("Second crop button")
                    .selected_text(format!("{:?}", &self.crop_shortcut.key))
                    .show_ui(ui, |ui| {
                        let options: [Key; 4] = [
                            Key::A, Key::S, Key::Q, Key::W
                        ];
    
                        for option in &options {
                            ui.selectable_value(
                                &mut self.crop_shortcut.key,
                                option.clone(),
                                format!("{:?}", option)
                            );

                        }
                    });

                if ui.button("Save").clicked() {
                    self.in_settings = false;
                }    
            });    
        }

        //Functions before cropping

        //MAIN CENTRAL PANEL
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::both().show(ui, |ui| {
                let s = &self.screenshot_built;
                let cropped_s = &self.cropped_screenshot_built;
                let scale_factor = self.get_current_screen().unwrap().display_info.scale_factor;

                match cropped_s {
                    Some(r) => {
                        if self.is_cropping {r.show_scaled(ui, 3.0);}
                        else {r.show_scaled(ui, 3.0/scale_factor);}
                    }
                    None => {
                        match s {
                            Some(r) => {
                                if self.is_cropping {r.show_scaled(ui, 1.0/scale_factor);}
                                else {r.show_scaled(ui, 0.9/scale_factor);}
                            }, 
                            None => {}
                        }
                    }
                }
            });
        });
    }
}


