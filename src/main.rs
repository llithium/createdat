use std::{
    ffi::OsStr,
    fs::{self, create_dir, read_dir, remove_dir},
    path::Path,
    time::SystemTime,
};

use chrono::{DateTime, Local};
use clap::Parser;

/// Rename images with the date they were created
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Optional prefix for renamed images
    prefix: Option<String>,

    /// Keep image's original name as prefix
    #[arg(short, long)]
    keep: bool,
}

fn main() {
    let start_time = SystemTime::now();
    let cli = Cli::parse();
    let renamed_folder = "./renamed";
    let accepted_formats = ["png", "jpg", "jpeg", "tiff", "webp", "heif"];
    let mut image_name = String::from("");
    let mut prefix = String::from("");

    if Path::new(renamed_folder).exists() {
        println!(
            "Folder {} already exists",
            renamed_folder.strip_prefix("./").unwrap()
        );
        return;
    }
    create_dir(renamed_folder).unwrap();

    let files = read_dir("./").unwrap();

    let mut images_renamed: u32 = 0;
    let mut total_images: u32 = 0;

    for file_result in files {
        let file = file_result.unwrap();
        let file_path = file.path();

        if file.metadata().unwrap().is_dir() {
            continue;
        }

        let file_name = file.file_name().into_string().unwrap();
        let file_extension = Path::new(&file_path)
            .extension()
            .and_then(OsStr::to_str)
            .unwrap_or_default();
        if !accepted_formats.contains(&file_extension) {
            continue;
        }
        total_images += 1;

        if let Some(entered_prefix) = cli.prefix.as_deref() {
            prefix = String::from(entered_prefix).trim().to_owned() + " "
        }
        if cli.keep {
            image_name = file_name
                .strip_suffix(&format!(".{}", file_extension))
                .unwrap_or_default()
                .to_string()
                + " "
        }

        let file_modified_at_system_time = file.metadata().unwrap().modified().unwrap();
        let file_modified_at_date_time: DateTime<Local> = file_modified_at_system_time.into();
        let image_modified_at_time = file_modified_at_date_time.format("%Y-%m-%d %H_%M_%S");

        let image_destination = format!(
            "{}/{}{}{}.{}",
            renamed_folder, prefix, image_name, image_modified_at_time, file_extension
        );

        if Path::new(&image_destination).exists() {
            println!(
                "Duplicate date: {}{}{}.{} already exists",
                prefix, image_name, image_modified_at_time, file_extension
            );
            continue;
        };
        fs::copy(file.path(), image_destination).unwrap();
        images_renamed += 1;
    }
    if total_images == 0 {
        remove_dir(renamed_folder).unwrap();
        println!("No images or wrong image formats");
    }

    let end_time = start_time.elapsed().unwrap();
    println!(
        "{}/{} images renamed in {:?}",
        images_renamed, total_images, end_time
    )
}
