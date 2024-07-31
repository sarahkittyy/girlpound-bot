use std::ops::Range;

use chrono::{Days, NaiveDate, NaiveDateTime};
use image::{codecs::png::PngEncoder, DynamicImage, ImageEncoder};
use plotters::{
    backend::{PixelFormat, RGBPixel},
    prelude::*,
};
use std::{fs::File, io::BufReader};

use common::Error;

use super::{Company, PriceHistory};

/// clamps the date range to a minimum and maximum
fn date_range(start: NaiveDateTime, end: NaiveDateTime) -> RangedDate<NaiveDate> {
    let diff = end - start;
    let end = if diff.num_days() < 7 {
        end.checked_add_days(Days::new(7 - diff.num_days().abs() as u64))
            .unwrap()
    } else if diff.num_days() >= 30 {
        start.checked_add_days(Days::new(30)).unwrap()
    } else {
        end
    };

    (start.date()..end.date()).into()
}

fn price_range(min: i32, max: i32) -> Range<i32> {
    min - 5..max + 5
}

pub async fn draw_stock_trends(
    company: &Company,
    mut log: Vec<PriceHistory>,
) -> Result<Vec<u8>, Error> {
    // fetch company logo image
    let logo = {
        let logo = company.logo.clone();
        tokio::task::spawn_blocking(|| -> Result<DynamicImage, Error> {
            Ok(image::load(
                BufReader::new(File::open(&logo)?),
                image::ImageFormat::from_path(logo)?,
            )?)
        })
        .await??
    }
    .resize_exact(640, 480, image::imageops::FilterType::Gaussian);

    let mut buf = vec![0; (640 * 480 * RGBPixel::PIXEL_SIZE) as usize];
    {
        // drawing init
        let root =
            BitMapBackend::<RGBPixel>::with_buffer_and_format(buf.as_mut_slice(), (640, 480))?
                .into_drawing_area();
        root.fill(&RGBColor(0x2F, 0x31, 0x36))?;

        let logo_elem: BitMapElement<_> = ((0, 0), logo).into();
        root.draw(&logo_elem)?;

        log.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        log.truncate(30);

        let min = log
            .iter()
            .min_by(|a, b| a.price.cmp(&b.price))
            .unwrap()
            .price;
        let max = log
            .iter()
            .max_by(|a, b| a.price.cmp(&b.price))
            .unwrap()
            .price;

        let start = log.last().unwrap().timestamp;
        let end = log.first().unwrap().timestamp;

        let title_style = TextStyle::from(("sans-serif", 28)).with_color(WHITE);
        let label_style = ("sans-serif", 14).with_color(WHITE);

        let date_range = date_range(start, end);
        let price_range = price_range(min, max);
        let mut chart = ChartBuilder::on(&root)
            .margin(40)
            .set_left_and_bottom_label_area_size(40)
            .caption(
                format!(
                    "{} @ CC {:.2} | {}",
                    company.tag, company.price, company.name
                ),
                title_style,
            )
            .x_label_area_size(30)
            .y_label_area_size(30)
            .build_cartesian_2d(date_range, price_range)?;
        chart
            .configure_mesh()
            .x_label_formatter(&|date| format!("{}", date.format("%m-%d")))
            .y_label_formatter(&|price| format!("CC {}", price))
            .label_style(label_style)
            .draw()?;

        chart.draw_series(
            LineSeries::new(
                log.iter()
                    .map(|ph: &PriceHistory| (ph.timestamp.date(), ph.price)),
                GREEN.filled(),
            )
            .point_size(4),
        )?;

        root.present()?;
    };
    let mut png_buf = vec![];
    let e = PngEncoder::new(&mut png_buf);
    e.write_image(&buf, 640, 480, image::ExtendedColorType::Rgb8)?;
    Ok(png_buf)
}
