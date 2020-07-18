use image::{DynamicImage, RgbaImage, Rgba, imageops::overlay, jpeg::{JPEGEncoder, PixelDensity, PixelDensityUnit}};
use magichawk::*;
use std::fs::File;

fn main() {


let IMAGE_PX_PER_CM :u16= ((IMAGE_HEIGHT as f64)/IMAGE_HEIGHT_CM).round() as u16;

let IMAGE_DPI : PixelDensity = PixelDensity{ density: (IMAGE_PX_PER_CM, IMAGE_PX_PER_CM), unit: PixelDensityUnit::Centimeters};

    let get_image = |url: &str|{
image::load_from_memory_with_format(
    &reqwest::blocking::get(url).unwrap().bytes().unwrap(),
    image::ImageFormat::Jpeg,
)
.unwrap()};

    let zilortha = get_image("https://api.scryfall.com/cards/named?fuzzy=zilortha&version=border_crop&format=image");
    let basri = get_image("https://api.scryfall.com/cards/named?fuzzy=basri+ket&version=border_crop&format=image");

    let images : Vec<DynamicImage> = vec![zilortha.clone(), zilortha.clone(), basri.clone(), basri.clone(), basri.clone(), basri.clone(), basri.clone(), zilortha.clone(), zilortha.clone()];

    let white_pixel = Rgba::<u8>([255,255, 255, 255]);
    let mut composed = RgbaImage::from_pixel(3*IMAGE_WIDTH, 3*IMAGE_HEIGHT, white_pixel);

    let mut pos_hor = 0;
    let mut pos_ver = 0;

    for im in images.iter() {
        overlay(&mut composed, im, pos_hor * IMAGE_WIDTH, pos_ver * IMAGE_HEIGHT);
        pos_hor += 1;
        if pos_hor == 3 {
            pos_hor = 0;
            pos_ver += 1;
        }
    }

    let mut outputfile = File::create("./examplepage.jpg").unwrap();
    let mut encoder = JPEGEncoder::new_with_quality(&mut outputfile, 100);
    encoder.set_pixel_density(IMAGE_DPI);
    encoder.encode_image(&composed).unwrap();
}