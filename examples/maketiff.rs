use image::{imageops::overlay, DynamicImage, Rgba, RgbaImage};
use magichawk::*;
use std::convert::TryFrom;
use std::fs::File;
use std::io::Write;
use tiff::{
    encoder::{colortype::RGBA8, DirectoryEncoder, ImageEncoder, Rational, TiffEncoder},
    tags::ResolutionUnit,
};

fn main() {
    let IMAGE_PX_PER_CM: u16 = ((IMAGE_HEIGHT as f64) / IMAGE_HEIGHT_CM).round() as u16;

    //let IMAGE_DPI: PixelDensity = PixelDensity {
    //    density: (IMAGE_PX_PER_CM, IMAGE_PX_PER_CM),
    //    unit: PixelDensityUnit::Centimeters,
    //};

    let FUDGE: f64 = 0.8;

    let get_image = |url: &str| {
        image::load_from_memory_with_format(
            &reqwest::blocking::get(url).unwrap().bytes().unwrap(),
            image::ImageFormat::Jpeg,
        )
        .unwrap()
    };

    let zilortha = get_image(
        "https://api.scryfall.com/cards/named?fuzzy=zilortha&version=border_crop&format=image",
    );
    let basri = get_image(
        "https://api.scryfall.com/cards/named?fuzzy=basri+ket&version=border_crop&format=image",
    );

    let images: Vec<DynamicImage> = vec![
        zilortha.clone(),
        zilortha.clone(),
        basri.clone(),
        basri.clone(),
        basri.clone(),
        basri.clone(),
        basri.clone(),
        zilortha.clone(),
        zilortha.clone(),
    ];

    let white_pixel = Rgba::<u8>([255, 255, 255, 255]);
    let mut composed = RgbaImage::from_pixel(3 * IMAGE_WIDTH, 3 * IMAGE_HEIGHT, white_pixel);

    let mut pos_hor = 0;
    let mut pos_ver = 0;

    for im in images.iter() {
        overlay(
            &mut composed,
            im,
            pos_hor * IMAGE_WIDTH,
            pos_ver * IMAGE_HEIGHT,
        );
        pos_hor += 1;
        if pos_hor == 3 {
            pos_hor = 0;
            pos_ver += 1;
        }
    }

    let buffer = composed.into_raw();

    let f = File::create("./example.tiff").unwrap();
    let mut encoder = TiffEncoder::new(f).unwrap();
    for _ in 0..2 {
        let mut page1 = encoder.new_image::<RGBA8>(PAGE_WIDTH, PAGE_HEIGHT).unwrap();
        page1.resolution(
            ResolutionUnit::Centimeter,
            Rational {
                n: ((IMAGE_HEIGHT as f64) / IMAGE_HEIGHT_CM).round() as u32,
                d: 1,
            },
        );
        let mut idx = 0;
        while page1.next_strip_sample_count() > 0 {
            let sample_count = page1.next_strip_sample_count() as usize;
            page1.write_strip(&buffer[idx..idx + sample_count]).unwrap();
            idx += sample_count;
        }
        page1.finish().unwrap();
    }
    // let mut directory = TiffEncoder::new_directory(&mut encoder).unwrap();
    // let mut image = encoder
    //     .new_image::<RGBA8>(3 * IMAGE_WIDTH, 3 * IMAGE_HEIGHT)
    //     .unwrap();
    // let mut idx = 0;
    // while image.next_strip_sample_count() > 0 {
    //     let sample_count = usize::try_from(image.next_strip_sample_count()).unwrap();
    //     image.write_strip(&buffer[idx..idx + sample_count]).unwrap();
    //     idx += sample_count;
    // }
    // image.finish();
    // directory.finish();
}
