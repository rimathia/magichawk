use printpdf::image_crate::DynamicImage;
use printpdf::{Image, ImageTransform, Mm, PdfDocument};

use crate::IMAGE_HEIGHT;
use crate::IMAGE_WIDTH;

use crate::IMAGE_HEIGHT_CM;
use crate::IMAGE_WIDTH_CM;

const A4_WIDTH: Mm = Mm(210.0);
const A4_HEIGHT: Mm = Mm(297.0);

const INCH_DIV_CM: f32 = 2.54;
const DPI: f32 = 300.0;
const DPCM: f32 = DPI / INCH_DIV_CM;

pub fn page_images_to_pdf<I>(it: I) -> Option<Vec<u8>>
where
    I: Iterator<Item = DynamicImage>,
{
    let (doc, page1, layer1) = PdfDocument::new("Proxies", A4_WIDTH, A4_HEIGHT, "Layer 1");

    let transform = ImageTransform {
        dpi: Some(DPI),
        translate_x: Some((A4_WIDTH - Mm(3.0 * IMAGE_WIDTH_CM * 10.0)) / 2.0),
        translate_y: Some((A4_HEIGHT - Mm(3.0 * IMAGE_HEIGHT_CM * 10.0)) / 2.0),
        scale_x: Some(IMAGE_WIDTH_CM / (IMAGE_WIDTH as f32) * DPCM),
        scale_y: Some(IMAGE_HEIGHT_CM / (IMAGE_HEIGHT as f32) * DPCM),
        rotate: None,
    };

    for (i, im) in it.enumerate() {
        if i > 0 {
            let (added_page, added_layer) = doc.add_page(A4_WIDTH, A4_HEIGHT, "Layer 1");
            let current_layer = doc.get_page(added_page).get_layer(added_layer);
            Image::from_dynamic_image(&im).add_to_layer(current_layer.clone(), transform);
        } else {
            let current_layer = doc.get_page(page1).get_layer(layer1);
            Image::from_dynamic_image(&im).add_to_layer(current_layer.clone(), transform);
        }
    }
    doc.save_to_bytes().ok()
}
