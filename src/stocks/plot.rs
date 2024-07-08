use image::{codecs::png::PngEncoder, ImageEncoder};
use plotters::{
    backend::{PixelFormat, RGBPixel},
    prelude::*,
};

use crate::Error;

use super::{Company, PriceHistory};

pub fn draw_stock_trends(company: &Company, log: &Vec<PriceHistory>) -> Result<Vec<u8>, Error> {
    let mut buf = vec![0; (640 * 480 * RGBPixel::PIXEL_SIZE) as usize];
    {
        // drawing init
        let root =
            BitMapBackend::<RGBPixel>::with_buffer_and_format(buf.as_mut_slice(), (640, 480))?
                .into_drawing_area();
        root.fill(&RGBColor(0x2F, 0x31, 0x36))?;

        let min = log
            .iter()
            .min_by(|a, b| a.price.cmp(&b.price))
            .unwrap()
            .price as f32;
        let max = log
            .iter()
            .max_by(|a, b| a.price.cmp(&b.price))
            .unwrap()
            .price as f32;
        let total = (max - min) as f32;

        let start = log
            .iter()
            .min_by(|a, b| a.timestamp.cmp(&b.timestamp))
            .unwrap()
            .timestamp;
        let end = log
            .iter()
            .max_by(|a, b| a.timestamp.cmp(&b.timestamp))
            .unwrap()
            .timestamp;

        let range: RangedDateTime<_> = (start..end).into();
        let mut chart = ChartBuilder::on(&root)
            .margin(3)
            .x_label_area_size(30)
            .y_label_area_size(30)
            .build_cartesian_2d(range, min..max)?;
        chart
            .configure_mesh()
            .x_label_formatter(&|date| format!("{}", date.date()))
            .label_style(WHITE.into_text_style(&root))
            .draw()?;

        // finish drawing
        root.present()?;
    };
    let mut png_buf = vec![];
    let e = PngEncoder::new(&mut png_buf);
    e.write_image(&buf, 640, 480, image::ExtendedColorType::Rgb8)?;
    Ok(png_buf)
}
