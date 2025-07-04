use eframe::emath::vec2;
use eframe::epaint::TextureHandle;
use egui::{Image, Ui, Widget};
use std::collections::HashMap;
use std::rc::Rc;

pub fn add_image(images: &Rc<HashMap<String, TextureHandle>>, ui: &mut Ui, file_name: &str) {
    let texture = images.get(file_name);
    if let Some(texture) = texture {
        Image::new(texture)
            .fit_to_exact_size(vec2(18.0, 18.0))
            .ui(ui);
    }
}

pub fn add_first_valid_image(
    images: &Rc<HashMap<String, TextureHandle>>,
    ui: &mut Ui,
    possible_file_names: Vec<String>,
) {
    for file_name in possible_file_names {
        let png_texture = images.get(&file_name);
        if let Some(texture) = png_texture {
            //test
            Image::new(texture)
                .fit_to_exact_size(vec2(18.0, 18.0))
                .ui(ui);
            break;
        }
    }
}
