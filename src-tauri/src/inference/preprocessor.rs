use std::path::Path;

use image::{DynamicImage, ImageReader, imageops::FilterType};
use ndarray::Array4;

use crate::inference::InferenceResult;

const TARGET_WIDTH: u32 = 384;
const TARGET_HEIGHT: u32 = 384;
const MEAN: f32 = 0.5;
const STD: f32 = 0.5;

#[allow(dead_code)]
pub fn preprocess_image_path(image_path: &Path) -> InferenceResult<Array4<f32>> {
    let image = ImageReader::open(image_path)?.decode()?;
    preprocess_dynamic_image(&image)
}

pub fn preprocess_dynamic_image(image: &DynamicImage) -> InferenceResult<Array4<f32>> {
    let resized = image
        .resize_exact(TARGET_WIDTH, TARGET_HEIGHT, FilterType::CatmullRom)
        .to_rgb8();

    let mut pixel_values =
        Array4::<f32>::zeros((1, 3, TARGET_HEIGHT as usize, TARGET_WIDTH as usize));

    for (x, y, pixel) in resized.enumerate_pixels() {
        let [r, g, b] = pixel.0;
        let y = y as usize;
        let x = x as usize;
        pixel_values[[0, 0, y, x]] = normalize_channel(r);
        pixel_values[[0, 1, y, x]] = normalize_channel(g);
        pixel_values[[0, 2, y, x]] = normalize_channel(b);
    }

    Ok(pixel_values)
}

#[inline]
fn normalize_channel(value: u8) -> f32 {
    (value as f32 / 255.0 - MEAN) / STD
}
