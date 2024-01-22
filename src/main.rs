#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::{env, borrow::Cow, collections::HashMap, sync::Arc};
use minifb::{self, WindowOptions, ScaleMode};
use chrono::prelude::*;
use eframe::{egui::{self, Pos2, Key, Modifiers, KeyboardShortcut, Window, Frame, Context, Ui, Image}, App, epaint::{Color32, Stroke, Vec2, vec2, TextureHandle, TextureManager, mutex::RwLock, TextureId }};
use screenshots::Screen;
use std::{thread, time::Duration};
use arboard::{Clipboard, ImageData};
use image::{imageops::FilterType::Nearest, Rgba, ImageBuffer, RgbaImage};

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

pub struct Painting {
    /// in 0-1 normalized coordinates
    lines: Vec<Vec<Pos2>>,
    stroke: Stroke,
    save: bool,
}

impl Painting { 
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ui_control(&mut self, ui: &mut egui::Ui) -> egui::Response {
        ui.horizontal(|ui| {
            egui::stroke_ui(ui, &mut self.stroke, "Stroke");
            ui.separator();
            if ui.button("Clear Painting").clicked() {
                self.lines.clear();
            }
            ui.separator();
            if ui.button("Save edit").clicked() {
                self.save = true;
            }
        })
        .response
    }

    pub fn ui_content(&mut self, ui: &mut egui::Ui, texture: &TextureHandle, width: f32, height: f32) -> egui::Response {
   
        let (mut response, painter) =
            ui.allocate_painter(Vec2::new(width, height), egui::Sense::drag());

        let to_screen = egui::emath::RectTransform::from_to(
            egui::Rect::from_min_size(egui::Pos2::ZERO, response.rect.size()),
            response.rect,
        );

        let from_screen = to_screen.inverse();

        if self.lines.is_empty() {
            self.lines.push(vec![]);
        }

        let current_line = self.lines.last_mut().unwrap();

        painter.add(egui::Shape::image(
            texture.id(),
            egui::Rect::from_min_size(response.rect.min, egui::vec2(width, height)),
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1., 1.)),
            egui::Color32::WHITE)
        );

        if let Some(pointer_pos) = response.interact_pointer_pos() {
            let canvas_pos = from_screen * pointer_pos;
            if current_line.last() != Some(&canvas_pos) {
                current_line.push(canvas_pos);
                response.mark_changed();
            }
        } else if !current_line.is_empty() {
            self.lines.push(vec![]);
            response.mark_changed();
        }

        let shapes = self
            .lines
            .iter()
            .filter(|line| line.len() >= 2)
            .map(|line| {
                let points: Vec<egui::Pos2> = line.iter().map(|p| to_screen * *p).collect();
                egui::Shape::line(points, self.stroke)
            });

        painter.extend(shapes);

        return response;
    }

    fn name(&self) -> &'static str {
        "🖊 Painting"
    }

    fn show(&mut self, ctx: &Context, open: &mut bool, texture: &TextureHandle, width: f32, height: f32) {
        Window::new(self.name())
            .open(open)
            .default_size(vec2(width, height))
            .vscroll(true)
            .hscroll(true)
            .show(ctx, |ui| self.ui(ui, texture, width, height));
    }

    fn ui(&mut self, ui: &mut Ui, texture: &TextureHandle, width: f32, height: f32) {
        self.ui_control(ui);
        Frame::canvas(ui.style()).show(ui, |ui| {
            self.ui_content(ui, texture, width, height)
        });
    }
}

impl Default for Painting {
    fn default() -> Self {
        Self {
            lines: Default::default(),
            stroke: Stroke::new(1.0, Color32::from_rgb(25, 200, 100)),
            save: false,
        }
    }
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
    in_settings: bool,
    painting: Painting,
    is_painting: bool,
    texture: TextureHandle,
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
            in_settings: false,
            painting: Painting::new(),
            is_painting: false,
            texture: TextureHandle::new(Arc::new(RwLock::new(TextureManager::default())) , TextureId::default()),
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
        let mut scale_factor = current_screen.display_info.scale_factor;
        let mut compensation = 65.0;
        if scale_factor > 1.0 { compensation = 0.0 };

        println!("Cropping {:?}", current_screen);
        let width = ((self.crop_end_pos[0] - self.crop_start_pos[0]) * scale_factor) as u32;
        let height = ((self.crop_end_pos[1] - self.crop_start_pos[1]) * scale_factor) as u32;

        println!("{} {}", self.crop_start_pos[0], self.crop_start_pos[1]);
        println!("{} {}", self.crop_end_pos[0], self.crop_end_pos[1]);
        if self.cropped_screenshot_raw.is_some() {
            println!("{} {}", self.cropped_screenshot_raw.as_ref().unwrap().width(), self.cropped_screenshot_raw.as_ref().unwrap().height());
        }
        
        //let image = current_screen.capture_area(self.crop_start_pos[0] as i32, self.crop_start_pos[1] as i32, width as u32, height as u32).unwrap();
        //let image = imageops::crop_imm(&self.screenshot_raw, self.crop_start_pos[0] as i32, self.crop_start_pos[1] as i32, width as u32, height as u32);
        let mut image = ImageBuffer::new(width as u32, height as u32); 
        for y in 0..height {
            for x in 0..width {
                let original_x = (self.crop_start_pos[0] * scale_factor) as u32 + x;
                let original_y = (self.crop_start_pos[1] - compensation) as u32 + y;
    
                let mut pixel: [u8; 4] = Default::default();
                if self.cropped_screenshot_raw.is_some() {
                    if original_x < self.cropped_screenshot_raw.as_ref().unwrap().width() && original_y < self.cropped_screenshot_raw.as_ref().unwrap().height() {
                        pixel = self.cropped_screenshot_raw.as_ref().unwrap().get_pixel(original_x, original_y).0;
                    }    
                } else {
                    pixel = self.screenshot_raw.as_ref().unwrap().get_pixel(original_x, original_y).0;
                }
                image.put_pixel(x, y, Rgba(pixel));
            }
        }

        self.cropped_screenshot_raw = Some(RgbaImage::new(width, height));
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

    fn save_as_screenshot(&mut self, prefix: Option<String>) {
        let prefix = prefix.unwrap_or(format!("{}/rust_screenshot", &self.save_directory));
        match &self.cropped_screenshot_raw {
            Some(s) => s.save(format!("{}", prefix)).unwrap(),
            None => {
                match &self.screenshot_raw {
                    Some(s) => s.save(format!("{}", prefix)).unwrap(),
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

    fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: Rgba<u8>) {
        let dx = i32::abs(x1 as i32 - x0 as i32);
        let dy = -i32::abs(y1 as i32 - y0 as i32);
        let sx: i32 = if x0 < x1 { 1 } else { -1 };
        let sy: i32 = if y0 < y1 { 1 } else { -1 };

        let mut err = dx + dy;

        let mut x = x0;
        let mut y = y0;

        while x != x1 || y != y1 {
            // Disegna il pixel
            if self.cropped_screenshot_raw.is_some() {
                self.cropped_screenshot_raw.as_mut().unwrap().put_pixel(x as u32, y as u32, color);
            } else {
                self.screenshot_raw.as_mut().unwrap().put_pixel(x as u32, y as u32, color);
            }
            

            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }
}

impl App for MyApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {

        let my_top_frame = egui::containers::Frame {
            inner_margin: egui::style::Margin { left: 10., right: 10., top: 10., bottom: 10. },
            outer_margin: egui::style::Margin { left: 10., right: 10., top: 10., bottom: 10. },
            rounding: egui::Rounding { nw: 1.0, ne: 1.0, sw: 1.0, se: 1.0 },
            shadow: eframe::epaint::Shadow { extrusion: 1.0, color: Color32::from_rgb(183, 93, 105)},
            fill: Color32::from_rgb(73, 73, 73),
            stroke: egui::Stroke::new(2.0, Color32::from_rgb(48, 188, 237)),
        };
        //TOP PANEL;
        
        egui::TopBottomPanel::top("my_top_panel").frame(my_top_frame).show(ctx, |ui| {
            if !self.is_cropping && !self.is_painting {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("Rust-Screenshot").heading().strong().color(egui::Color32::from_rgb(255, 255, 255)));
                        ui.label("A cross-platform tool for screen-grabbing in Rust");
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Max), |ui| {
                        if ui.add_sized([180., 40.], egui::Button::new("⛭  EDIT SETTINGS")).clicked() {
                            self.in_settings = true;
                        };
                    });
                });
            }
            else {
                ui.horizontal(|ui| {
                    ui.label("Crop Screenshot");
                });
            }
        });


        let my_left_frame = egui::containers::Frame {
            inner_margin: egui::style::Margin { left: 10., right: 10., top: 10., bottom: 10. },
            outer_margin: egui::style::Margin { left: 10., right: 10., top: 10., bottom: 10. },
            rounding: egui::Rounding { nw: 1.0, ne: 1.0, sw: 1.0, se: 1.0 },
            shadow: eframe::epaint::Shadow { extrusion: 1.0, color: Color32::from_rgb(255, 93, 115)},
            fill: Color32::from_rgb(73, 73, 73),
            stroke: egui::Stroke::new(2.0, Color32::from_rgb(252, 81, 48)),
        };

        //LEFT PANEL (only if not cropping)
        egui::SidePanel::left("my_left_panel").resizable(false).default_width(290.).frame(my_left_frame).show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {

                ui.vertical_centered(|ui|{ui.label(egui::RichText::new("🖵  DISPLAY").heading().strong().color(egui::Color32::from_rgb(255, 255, 255)))});
                ui.add(egui::Separator::default());
                ui.add_space(10.0);

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

                ui.add_space(10.0);
                ui.add(egui::Separator::default());
                ui.vertical_centered(|ui|{ui.label(egui::RichText::new("📷  SCREENSHOT").heading().strong().color(egui::Color32::from_rgb(255, 255, 255)))});
                ui.add(egui::Separator::default());
                ui.add_space(10.0);

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
                if ui.add_sized([280., 40.], egui::Button::new("📷  TAKE A SCREENSHOT")).clicked() || ctx.input_mut(|i| i.consume_shortcut(&self.screenshot_shortcut)){
                    frame.set_visible(false);
                    self.is_taking = true;
                }

                ui.horizontal(|ui| {

                    if (ui.add_sized([140., 40.], egui::Button::new("✂  CROP SCREENSHOT")).clicked() || ctx.input_mut(|i| i.consume_shortcut(&self.crop_shortcut))) && self.check_screenshot() {
                        self.is_cropping = true;

                        let scale_factor = self.get_current_screen().unwrap().display_info.scale_factor as usize;
                        let mut width;
                        let mut height;
                        match &self.cropped_screenshot_raw {
                            Some(_c) => { 
                                width = self.cropped_screenshot_built.as_ref().unwrap().width().clone();
                                height = self.cropped_screenshot_built.as_ref().unwrap().height().clone(); 
                                if width >= self.get_current_screen().unwrap().display_info.width as usize|| height >= self.get_current_screen().unwrap().display_info.height as usize {
                                    width = width / scale_factor;
                                    height = height / scale_factor;
                                }
                            },
                            None => {
                                width = self.screenshot_built.as_ref().unwrap().width().clone();
                                height = self.screenshot_built.as_ref().unwrap().height().clone(); 
                                if width > self.get_current_screen().unwrap().display_info.width as usize|| height >= self.get_current_screen().unwrap().display_info.height as usize {
                                    width = width / scale_factor;
                                    height = height / scale_factor;
                                }
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
                                .unwrap_or_else(|e| {
                                    panic!("{}", e);}
                                );
                            
                        }
                    }

                    if ui.add_sized([140., 40.], egui::Button::new("🗙  CANCEL CROP")).clicked(){
                        self.cropped_screenshot_built = None;
                        self.cropped_screenshot_raw = None;
                    }

                });

                ui.add_space(10.0);
                ui.add(egui::Separator::default());
                ui.vertical_centered(|ui|{ui.label(egui::RichText::new("🗁  SAVE").heading().strong().color(egui::Color32::from_rgb(255, 255, 255)))});
                ui.add(egui::Separator::default());
                ui.add_space(10.0);

                if ui.add_sized([280., 20.], egui::Button::new("SELECT DEFAULT SAVE LOCATION")).clicked() {
                    let fd = rfd::FileDialog::new();
                    match fd.pick_folder() {
                        Some(path) => self.save_directory = path.into_os_string().into_string().unwrap(),
                        None => (),
                    }
                }
                ui.label(format!("Current folder: {}", &self.save_directory));

        

                if ui.add_sized([280., 40.], egui::Button::new("🗁  SAVE")).clicked() {
                    self.save_screenshot(None);
                }

                ui.horizontal(|ui| {
                    if ui.add_sized([140., 20.], egui::Button::new("SAVE AS")).clicked() {
                        //self.save_screenshot(Some( path.into_os_string().into_string().unwrap()))
                        let fd = rfd::FileDialog::new();
                        match fd.save_file() {
                            Some(path) => {self.save_as_screenshot(Some(path.into_os_string().into_string().unwrap())) },
                            None => (),
                        }
                    }

                    // Il crate screenshots restituisce oggetti di tipo ImageBuffer<Rgba<u8>, Vec<u8>>
                    // Il crate arboard restituisce oggetti di tipo ImageData { pub width: usize, pub height: usize, pub bytes: Cow<'a, [u8]>}
                    // C'è bisogno di convertire manualmente l'ImageBuffer in ImageData perchè non c'è un cast diretto
                    if ui.add_sized([140., 20.], egui::Button::new("COPY TO CLIPBOARD")).clicked() {

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
                });

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


                ui.add_space(10.0);
                ui.add(egui::Separator::default());
                ui.vertical_centered(|ui|{ui.label(egui::RichText::new("🕘  DELAY").heading().strong().color(egui::Color32::from_rgb(255, 255, 255)))});
                ui.add(egui::Separator::default());
                ui.add_space(10.0);

                ui.checkbox(&mut self.delay_enable, "Enable delay");
                
                ui.add_sized([280., 20.],
                    egui::DragValue::new(&mut self.delay)
                        .speed(0.1)
                        .clamp_range(0..=30)
                        .prefix("Timer: ")
                        .suffix(" seconds")
                );

                ui.add_space(10.0);
                ui.add(egui::Separator::default());
                ui.vertical_centered(|ui|{ui.label(egui::RichText::new("✏ PAINT").heading().strong().color(egui::Color32::from_rgb(255, 255, 255)))});
                ui.add(egui::Separator::default());
                ui.add_space(10.0);

                if ui.add_sized([280., 40.], egui::Button::new("Paint your image")).clicked() && self.check_screenshot() {

                    if self.cropped_screenshot_raw.is_some() {
                        let bg = self.cropped_screenshot_raw.as_ref().unwrap().as_flat_samples();
                        let size = [self.cropped_screenshot_raw.as_ref().unwrap().width() as usize, self.cropped_screenshot_raw.as_ref().unwrap().height() as usize];
                        let background = egui::ColorImage::from_rgba_premultiplied(size, bg.as_slice());
                        let texture = ctx.load_texture("Screen", background, Default::default());
                        self.texture = texture;

                        self.is_painting = true
                    } else {
                        let bg = self.screenshot_raw.as_ref().unwrap().as_flat_samples();
                        let size = [self.screenshot_raw.as_ref().unwrap().width() as usize, self.screenshot_raw.as_ref().unwrap().height() as usize];
                        let background = egui::ColorImage::from_rgba_premultiplied(size, bg.as_slice());
                        let texture = ctx.load_texture("Screen", background, Default::default());
                        self.texture = texture;

                        self.is_painting = true;
                    }
                }   
            });
        }); //End of left panel

        if self.is_painting {
            
            let width: f32;
            let height: f32;
            let scale_factor = self.get_current_screen().unwrap().display_info.scale_factor;

            if self.cropped_screenshot_raw.is_some() {
                width = self.cropped_screenshot_raw.as_ref().unwrap().width() as f32;
                height = self.cropped_screenshot_raw.as_ref().unwrap().height() as f32;
            } else {
                width = self.screenshot_raw.as_ref().unwrap().width() as f32;
                height = self.screenshot_raw.as_ref().unwrap().height() as f32;
            }
            self.painting.show(ctx, &mut self.is_painting, &self.texture, width/scale_factor, height/scale_factor);
        }

        if self.painting.save {
            self.is_painting = false;
            for line in self.painting.lines.clone() {
                if line.len() > 0 {
                    let color32 = self.painting.stroke.color;
                    let rgba_u8 = Rgba([
                        color32.r() as u8,
                        color32.g() as u8,
                        color32.b() as u8,
                        color32.a() as u8,
                    ]);
                    let scale_factor = self.get_current_screen().unwrap().display_info.scale_factor;

                    let first_range = 0..(line.len() - 2);
                    let second_range = 1..(line.len() - 1);
                    let thickness = self.painting.stroke.width as u32;

                    for (i,j) in first_range.zip(second_range) {
                        for k in 0..thickness {
                            let offset = (k as i32 - (thickness as i32 / 2)) as f32;
                            self.draw_line(
                                ((line[i][0] * scale_factor) + offset) as i32, 
                                ((line[i][1] * scale_factor) + offset) as i32,
                                ((line[j][0] * scale_factor) + offset) as i32, 
                                ((line[j][1] * scale_factor) + offset) as i32, 
                                rgba_u8);
                        }
                        
                    }
                }
            }

            self.screenshot_built = self.get_render_result();
            self.cropped_screenshot_built = self.get_cropped_render_result();
            self.painting.save = false;
        }

        if self.in_settings {
            let modifiers_options = HashMap::from([
                (Modifiers::ALT, "ALT"),
                (Modifiers::CTRL, "CTRL"),
                (Modifiers::SHIFT, "SHIFT"),
                (Modifiers::COMMAND, "COMMAND"),
            ]);

            egui::Window::new("Settings").show(ctx, |ui| {

                ui.label("Screenshot shortcut");
                ui.separator();

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
                
                ui.separator();
                ui.label("Crop shortcut");
                ui.separator();
                
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

                ui.separator();

                if ui.add_sized([140., 40.], egui::Button::new("SAVE")).clicked() {
                    self.in_settings = false;
                }
 
            });    
        }

        //MAIN CENTRAL PANEL
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::both().show(ui, |ui| {
                let s = &self.screenshot_built;
                let cropped_s = &self.cropped_screenshot_built;
                let scale_factor = self.get_current_screen().unwrap().display_info.scale_factor;

                match cropped_s {
                    Some(r) => {
                        if self.is_cropping {r.show_scaled(ui, 3.0/scale_factor);}
                        else {r.show_scaled(ui, 1.0/scale_factor);}
                    }
                    None => {
                        match s {
                            Some(r) => {
                                if self.is_cropping {r.show_scaled(ui, 1.0/scale_factor);}
                                else {r.show_scaled(ui, 0.8/scale_factor);}
                            }, 
                            None => {}
                        }
                    }
                }
            });
        });


        
        
    }
}


