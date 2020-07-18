use image::{DynamicImage, RgbaImage, Rgba, imageops::overlay, jpeg::{JPEGEncoder, PixelDensity, PixelDensityUnit}};
use magichawk::*;
use std::fs::File;
use std::io::Write;
use wkhtmltopdf::{Margin, PdfApplication, Size::Millimeters};

fn main() {


let IMAGE_PX_PER_CM :u16= ((IMAGE_HEIGHT as f64)/IMAGE_HEIGHT_CM).round() as u16;

let IMAGE_DPI : PixelDensity = PixelDensity{ density: (IMAGE_PX_PER_CM, IMAGE_PX_PER_CM), unit: PixelDensityUnit::Centimeters};

let FUDGE : f64 = 0.8;

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

    let mut buf: Vec<u8> = vec![];
    let mut encoder = JPEGEncoder::new_with_quality(&mut buf, 100);
    encoder.set_pixel_density(IMAGE_DPI);
    encoder.encode_image(&composed).unwrap();

    let html = format!(
        r#"<img src="data:image/jpeg;base64,{}">"#,
        base64::encode(&buf));

    File::create("./examplepage.html").unwrap().write_all(html.as_bytes()).unwrap();

        let mut pdf_app = PdfApplication::new().expect("Failed to init PDF application");
let mut pdfout = pdf_app.builder()
    // .margin(Margin{top:Millimeters(7), bottom:Millimeters(7), left:Millimeters(5), right:Millimeters(5)}).dpi(198)
       .build_from_html(String::new() + &html + &html + &html)
       .expect("failed to build pdf");

    pdfout.save("./examplepage.pdf").unwrap();
}