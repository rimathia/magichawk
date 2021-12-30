extern crate printpdf;

use printpdf::image_crate::{imageops::overlay, DynamicImage, GenericImageView, RgbImage};
// imports the `image` library with the exact version that we are using
use printpdf::*;

use std::fs::File;
use std::io::{BufReader, BufWriter};

pub const IMAGE_WIDTH: u32 = 480;
pub const IMAGE_HEIGHT: u32 = 680;

pub const PAGE_WIDTH: u32 = 3 * IMAGE_WIDTH;
pub const PAGE_HEIGHT: u32 = 3 * IMAGE_HEIGHT;

pub const IMAGE_HEIGHT_CM: f64 = 8.7;
pub const IMAGE_WIDTH_CM: f64 = IMAGE_HEIGHT_CM * IMAGE_WIDTH as f64 / IMAGE_HEIGHT as f64;

pub const INCH_DIV_CM: f64 = 2.54;

pub const DPI: f64 = 300.0;
pub const DPCM: f64 = DPI / INCH_DIV_CM;

const A4_WIDTH: Mm = Mm(210.0);
const A4_HEIGHT: Mm = Mm(297.0);

fn test_bmp() {
    let (doc, page1, layer1) =
        PdfDocument::new("PDF_Document_title", A4_WIDTH, A4_HEIGHT, "Layer 1");
    let current_layer = doc.get_page(page1).get_layer(layer1);

    // currently, the only reliable file formats are bmp/jpeg/png
    // this is an issue of the image library, not a fault of printpdf
    let mut image_file = File::open("assets/img/BMP_test.bmp").expect("couldn't find BMP_test.bmp");
    let image =
        Image::try_from(image_crate::bmp::BmpDecoder::new(&mut image_file).unwrap()).unwrap();

    // translate x, translate y, rotate, scale x, scale y
    // by default, an image is optimized to 300 DPI (if scale is None)
    // rotations and translations are always in relation to the lower left corner
    image.add_to_layer(current_layer, ImageTransform::default());

    doc.save(&mut BufWriter::new(File::create("test_bmp.pdf").unwrap()))
        .unwrap();
}

fn test_png() {
    let (doc, page1, layer1) =
        PdfDocument::new("PDF_Document_title", A4_WIDTH, A4_HEIGHT, "Layer 1");
    let current_layer = doc.get_page(page1).get_layer(layer1);

    let image_file =
        File::open("assets/img/hho-18-bog-humbugs.png").expect("couldn't find png file");
    let png = image_crate::load(BufReader::new(image_file), image_crate::ImageFormat::Png)
        .expect("couldn't load png");

    println!("the png has size {:?}", png.dimensions());
    println!("the png has color {:?}", png.color());

    let without_alpha = DynamicImage::ImageRgb8(png.to_rgb8());

    let image = Image::from_dynamic_image(&without_alpha);

    image.add_to_layer(current_layer, ImageTransform::default());

    doc.save(&mut BufWriter::new(File::create("test_png.pdf").unwrap()))
        .unwrap();
}

fn test_jpg() {
    let (doc, page1, layer1) =
        PdfDocument::new("PDF_Document_title", A4_WIDTH, A4_HEIGHT, "Layer 1");
    let current_layer = doc.get_page(page1).get_layer(layer1);

    let image_file = File::open("assets/img/steam-vents.jpg").expect("couldn't find jpg file");
    let jpg = image_crate::load(BufReader::new(image_file), image_crate::ImageFormat::Jpeg)
        .expect("couldn't load jpg");

    println!("the jpg has size {:?}", jpg.dimensions());
    println!("the jpg has color {:?}", jpg.color());

    let without_alpha = DynamicImage::ImageRgb8(jpg.to_rgb8());

    let image = Image::from_dynamic_image(&without_alpha);

    let transform = ImageTransform {
        dpi: Some(DPI),
        translate_x: Some(Mm(10.0)),
        translate_y: Some(Mm(10.0)),
        scale_x: Some(IMAGE_WIDTH_CM / (IMAGE_WIDTH as f64) * DPCM),
        scale_y: Some(IMAGE_HEIGHT_CM / (IMAGE_HEIGHT as f64) * DPCM),
        rotate: None,
    };
    println!("the transform is {:?}", transform);
    image.add_to_layer(current_layer, transform);

    doc.save(&mut BufWriter::new(File::create("test_jpg.pdf").unwrap()))
        .unwrap();
}

fn test_rgb() {
    let (doc, page1, layer1) =
        PdfDocument::new("PDF_Document_title", A4_WIDTH, A4_HEIGHT, "Layer 1");
    let current_layer = doc.get_page(page1).get_layer(layer1);

    let image_file = File::open("assets/img/steam-vents.jpg").expect("couldn't find jpg file");
    let jpg = image_crate::load(BufReader::new(image_file), image_crate::ImageFormat::Jpeg)
        .expect("couldn't load jpg");

    println!("the jpg has size {:?}", jpg.dimensions());
    println!("the jpg has color {:?}", jpg.color());

    let without_alpha = jpg.to_rgb8();

    let mut custom_rgb = RgbImage::new(PAGE_WIDTH, PAGE_HEIGHT);
    for pixel in custom_rgb.pixels_mut() {
        pixel.0 = [255, 255, 255];
    }

    overlay(&mut custom_rgb, &without_alpha, 0, 0);
    overlay(
        &mut custom_rgb,
        &without_alpha,
        2 * IMAGE_WIDTH,
        IMAGE_HEIGHT,
    );
    overlay(
        &mut custom_rgb,
        &without_alpha,
        IMAGE_WIDTH,
        2 * IMAGE_HEIGHT,
    );

    let transform = ImageTransform {
        dpi: Some(DPI),
        translate_x: Some((A4_WIDTH - Mm(3.0 * IMAGE_WIDTH_CM * 10.0)) / 2.0),
        translate_y: Some((A4_HEIGHT - Mm(3.0 * IMAGE_HEIGHT_CM * 10.0)) / 2.0),
        scale_x: Some(IMAGE_WIDTH_CM / (IMAGE_WIDTH as f64) * DPCM),
        scale_y: Some(IMAGE_HEIGHT_CM / (IMAGE_HEIGHT as f64) * DPCM),
        rotate: None,
    };
    println!("the transform is {:?}", transform);

    let dynamic_image = DynamicImage::ImageRgb8(custom_rgb);

    Image::from_dynamic_image(&dynamic_image).add_to_layer(current_layer, transform);

    doc.save(&mut BufWriter::new(File::create("test_rgb.pdf").unwrap()))
        .unwrap();
}

fn main() {
    test_bmp();
    test_png();
    test_jpg();
    test_rgb();
}
