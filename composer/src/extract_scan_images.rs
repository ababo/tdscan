use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::io::Reader as ImageReader;
use image::ImageOutputFormat;
use structopt::StructOpt;

use crate::texture::{
    parse_color_into_vector3, BackgroundDetector, BackgroundParams, Vector2,
    Vector3,
};
use base::defs::{Error, ErrorKind, IntoResult, Result};
use base::fm;
use base::util::cli;
use base::util::fs;

#[derive(StructOpt)]
#[structopt(about = "Extract scan images")]
pub struct ExtractScanImagesCommand {
    #[structopt(flatten)]
    input: cli::FmInput,

    #[structopt(help = "Output image directory", long, short = "o")]
    output_dir: Option<PathBuf>,

    #[structopt(flatten)]
    pub background: BackgroundParams,

    #[structopt(
    help = "Color to highlight background with",
        long,
        parse(try_from_str = parse_color_into_vector3),
        default_value = "#ff0000"
    )]
    pub highlight_color: Vector3,
}

impl ExtractScanImagesCommand {
    pub fn run(&self) -> Result<()> {
        let mut reader = self.input.get()?;

        let output_dir =
            self.output_dir.as_deref().unwrap_or_else(|| ".".as_ref());

        extract_scan_images(
            reader.as_mut(),
            |p, d| fs::write_file(p, d),
            output_dir,
            &self.background,
            &self.highlight_color,
        )
    }
}

pub fn extract_scan_images<F: Fn(&Path, &[u8]) -> Result<()>>(
    reader: &mut dyn fm::Read,
    write_file: F,
    output_dir: &Path,
    background: &BackgroundParams,
    highlight_color: &Vector3,
) -> Result<()> {
    for n in 1.. {
        let rec = reader.read_record()?;
        if rec.is_none() {
            break;
        }

        if let Some(fm::record::Type::ScanFrame(frame)) = rec.unwrap().r#type {
            if let Some(mut image) = frame.image {
                if background.deviation > 0.0 {
                    image = highlight_background(
                        &image,
                        background,
                        highlight_color,
                    )?;
                }
                let ext = fm::image_type_extension(image.r#type());
                let filename =
                    output_dir.join(&n.to_string()).with_extension(ext);
                write_file(&filename, &image.data)?;
            }
        }
    }

    Ok(())
}

fn highlight_background(
    image: &fm::Image,
    params: &BackgroundParams,
    highlight_color: &Vector3,
) -> Result<fm::Image> {
    let err_fn = || "failed to decode frame image".to_string();
    let mut rgb = ImageReader::new(Cursor::new(&image.data))
        .with_guessed_format()
        .into_result(err_fn)?
        .decode()
        .map_err(|e| Error::with_source(ErrorKind::ImageError, err_fn(), e))?
        .into_rgb8();

    let detector = BackgroundDetector::new(&rgb, params);
    for i in 0..rgb.height() {
        for j in 0..rgb.width() {
            let uv = Vector2::new(
                i as f64 / rgb.height() as f64,
                j as f64 / rgb.width() as f64,
            );
            let pixel = rgb.get_pixel_mut(j, i);
            if detector.detect(uv) {
                pixel.0[0] = highlight_color[0] as u8;
                pixel.0[1] = highlight_color[1] as u8;
                pixel.0[2] = highlight_color[2] as u8;
            }
        }
    }

    let mut data = Cursor::new(Vec::new());
    rgb.write_to(&mut data, ImageOutputFormat::Png).unwrap();

    Ok(fm::Image {
        r#type: fm::image::Type::Png as i32,
        data: data.into_inner(),
    })
}
