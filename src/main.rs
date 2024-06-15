#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use eframe::egui;
use egui_extras::RetainedImage;

use clap::{arg, command, Arg, Command, ArgMatches};
use image::{DynamicImage, GenericImageView};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::mem::size_of;
use std::{fs, io::Write, mem, path::PathBuf, ptr};
use eframe::egui::ColorImage;

use skia_safe::{AlphaType, Color4f, ColorType, EncodedImageFormat, ImageInfo, Paint, Surface};

#[derive(Debug)]
struct BruhError(&'static str);

impl Error for BruhError {}

impl Display for BruhError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[repr(C)]
struct BruhHeader {
    magic: u32,
    width: u32,
    height: u32,
}

const BRUH_MAGIC_NUMBER: u32 =
    'B' as u32 | ('R' as u32) << 8 | ('U' as u32) << 16 | ('H' as u32) << 24;

#[allow(dead_code)]
const fn assert_bruh_magic_num() {
    unsafe {
        let magic: *const u8 = &BRUH_MAGIC_NUMBER as *const u32 as *const u8;
        assert!(*magic == 'B' as u8);
        assert!(*magic.add(1) == 'R' as u8);
        assert!(*magic.add(2) == 'U' as u8);
        assert!(*magic.add(3) == 'H' as u8);
    }
}
const _: () = assert_bruh_magic_num();

impl From<&DynamicImage> for BruhHeader {
    fn from(img: &DynamicImage) -> Self {
        BruhHeader {
            magic: BRUH_MAGIC_NUMBER,
            width: img.width(),
            height: img.height(),
        }
    }
}

const BRUH_HEADER_SIZE: usize = size_of::<BruhHeader>();

impl BruhHeader {
    fn bytes(&self) -> &[u8; BRUH_HEADER_SIZE] {
        // could use some padding here for future additions
        unsafe { mem::transmute(self) }
    }

    unsafe fn from_raw(ptr: *const u8) -> Result<Self, Box<dyn Error>> {
        if ptr.is_null() {
            return Err(Box::new(BruhError("Null pointer to from_raw provided")));
        }


        let header: BruhHeader = ptr::read(ptr as *const BruhHeader);
        
        if header.magic != BRUH_MAGIC_NUMBER {
            return Err(Box::new(BruhError("File was not in BRUH format. (Header did not match magic number)")));
        }
        Ok(header)
    }
}

fn image_to_bruh(path: &PathBuf) -> Result<(), Box<dyn Error>> {
    let img = image::open(path)?;
    let mut header: BruhHeader = BruhHeader::from(&img);
    let mut data: Vec<u8> = Vec::new();

    for pixel in img.pixels() {
        // push RGBA in that order
        data.push(pixel.2 .0[0]);
        data.push(pixel.2 .0[1]);
        data.push(pixel.2 .0[2]);
        data.push(pixel.2 .0[3]);
    }

    let path_str = path.to_str().ok_or("Path did not contain valid unicode")?;

    let bruh_path = match path_str.rfind(".") {
        None => path_str.to_string() + ".bruh",
        Some(idx) => path_str[..idx].to_string() + ".bruh",
    };

    let mut file = File::create(bruh_path)?;

    file.write_all(header.bytes())?;
    file.write_all(&data)?;
    file.flush()?;

    Ok(())
}

fn get_bruh_image_data(path: &PathBuf) -> Result<(BruhHeader, Vec<u8>), Box<dyn Error>> {
    let mut contents: Vec<u8> = fs::read(path)?;
    let header = unsafe { BruhHeader::from_raw(contents.as_ptr())? };
    contents.drain(0..BRUH_HEADER_SIZE);

    Ok((header, contents))
}

// This is completely unused now because there wasn't even a way previously to convert from bruh back to png
#[allow(dead_code)]
fn bruh_to_png(path: &PathBuf) -> Result<(u32, u32), Box<dyn Error>> {
    let (header, contents) = get_bruh_image_data(path)?;
    let chunked_data = contents.chunks_exact(4);

    let info = ImageInfo::new(
        (header.width as i32, header.height as i32),
        ColorType::RGBA8888,
        AlphaType::Opaque,
        None,
    );

    let mut surface = Surface::new_raster(&info, None, None).unwrap();
    let canvas = surface.canvas();

    for (channels, x, y) in (0u32..)
        .zip(chunked_data)
        .map(|(i, channels)| (channels, i % header.width, i / header.width))
    {
        let color4f = Color4f::new(
            channels[0] as f32 / 255.0,
            channels[1] as f32 / 255.0,
            channels[2] as f32 / 255.0,
            channels[3] as f32 / 255.0,
        ); // could map this too but what the hell

        let paint = Paint::new(color4f, None);
        canvas.draw_point((x as f32, y as f32), &paint);
    }

    let image = surface.image_snapshot();

    if let Some(data) = image.encode(None, EncodedImageFormat::PNG, 100) {
        fs::write(TEMP_IMAGE_PATH, &*data).expect("Failed to write image data to file");
    }

    Ok((header.width, header.height))
}

const TEMP_IMAGE_PATH: &str = "temp.png";

const ARG_CONVERT: &str = "convert";
const ID_PATH: &str = "image_path";
fn main() -> Result<(), Box<dyn Error>> {
    let matches = command!()
        .version("1.0")
        .author("face-hh")
        .about("BRUH image format implementation and converter")
        .arg(Arg::new(ID_PATH).required(true).index(1))
        .subcommand(
            Command::new(ARG_CONVERT)
                .about("convert an image to BRUH format")
                .arg(Arg::new(ID_PATH).index(1).required(true)),
        )
        .subcommand_negates_reqs(true)
        .get_matches();

    match handle_matches(&matches) {
        Ok(_) => {}
        Err(e) => eprintln!("{e}"),
    }
    Ok(())
}

fn handle_matches(matches: &ArgMatches) -> Result<(), Box<dyn Error>> {
    if let Some(convert) = matches.subcommand_matches("convert") {
        let path_str = convert.get_one::<String>("image_path").unwrap(); // arg is required
        let path = PathBuf::from(path_str);

        match image_to_bruh(&path) {
            Ok(()) => println!("Successfully converted PNG to BRUH"),
            Err(_) => println!("Failed to convert PNG to BRUH"),
        }
    } else {
        let path_str: &String = matches.get_one(ID_PATH).unwrap(); // arg is required
        // don't require .bruh file extension because file extensions are not real
        let path = PathBuf::from(path_str);
        
        let (header, content) = get_bruh_image_data(&path)?;
        println!("Loading a BRUH image with dimensions: {} {}", header.width, header.height);
        let options = eframe::NativeOptions {
            resizable: false,
            initial_window_size: Some(egui::vec2(header.width as f32, header.height as f32)),
            ..Default::default()
        };

        let preview = ImagePreview::new_bruh_image(&header, &content);

        eframe::run_native("Image preview", options, Box::new(|_cc| Box::new(preview)))?;
    }
    Ok(())
}

struct ImagePreview {
    image: RetainedImage,
}

impl ImagePreview {
    fn new(path: &str) -> Result<Self, Box<dyn Error>> {
        let image_data = fs::read(path)?;

        fs::remove_file(path)?;

        Ok(Self {
            image: RetainedImage::from_image_bytes(path, &image_data).unwrap(),
        })
    }
    
    fn new_bruh_image(header: &BruhHeader, data: &[u8]) -> Self {
        let color_image = ColorImage::from_rgba_unmultiplied([header.width as usize, header.height as usize], data);
        Self {
            image: RetainedImage::from_color_image("image", color_image),
        }
    }
}

impl eframe::App for ImagePreview {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.image.show(ui);
        });
    }
}
