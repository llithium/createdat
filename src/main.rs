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

    /// Use 12-hour time format instead of 24-hour
    #[arg(short, long)]
    twelve: bool,

    /// Rename all files, not just images
    #[arg(short, long)]
    all: bool,
}

fn main() {
    let start_time = SystemTime::now();
    let cli = Cli::parse();
    let renamed_folder = "./renamed";
    let accepted_formats = ["png", "jpg", "jpeg", "tiff", "webp", "heif"];
    let mut image_name = String::from("");
    let mut prefix = String::from("");

    if Path::new(renamed_folder).exists() {
        eprintln!(
            "Folder {} already exists",
            renamed_folder.strip_prefix("./").unwrap_or(renamed_folder)
        );
        return;
    }
    if let Err(err) = create_dir(renamed_folder) {
        eprintln!("Error creating directory: {}", err);
        return;
    }

    let files = match read_dir("./") {
        Ok(files) => files,
        Err(err) => {
            eprintln!("Error reading directory: {}", err);
            return;
        }
    };

    let mut images_renamed: u32 = 0;
    let mut total_images: u32 = 0;

    for file_result in files {
        let file = file_result.unwrap();
        let file_path = file.path();

        if file.metadata().unwrap().is_dir() {
            continue;
        }

        let file_name = file.file_name().into_string().unwrap();
        let file_extension = match file_name.starts_with('.') {
            true => file_name.strip_prefix('.').unwrap(),
            false => Path::new(&file_path)
                .extension()
                .and_then(OsStr::to_str)
                .unwrap_or_default(),
        };

        if !cli.all && !accepted_formats.contains(&file_extension) {
            continue;
        }
        total_images += 1;

        if let Some(entered_prefix) = cli.prefix.as_deref() {
            prefix =
                sanitize_filename::sanitize(String::from(entered_prefix).trim()).to_owned() + " "
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
        let image_modified_at_time = match cli.twelve {
            true => file_modified_at_date_time.format("%Y-%m-%d %I_%M_%S %p"),
            false => file_modified_at_date_time.format("%Y-%m-%d %H_%M_%S"),
        };

        let image_destination = format!(
            "{}/{}{}{}.{}",
            renamed_folder, prefix, image_name, image_modified_at_time, file_extension
        );

        if Path::new(&image_destination).exists() {
            eprintln!(
                "Duplicate date: {}{}{}.{} already exists",
                prefix, image_name, image_modified_at_time, file_extension
            );
            continue;
        };
        match fs::copy(file.path(), image_destination) {
            Ok(_) => images_renamed += 1,
            Err(err) => {
                eprintln!("{}", err);
                if let Err(err) = remove_dir(renamed_folder) {
                    eprintln!("{}", err)
                };
                return;
            }
        }
    }

    if images_renamed == 0 {
        if let Err(err) = remove_dir(renamed_folder) {
            eprintln!("{err}")
        };
        match cli.all {
            true => eprintln!("No files found"),
            false => eprintln!("No images or wrong image formats"),
        }
        return;
    }

    let end_time = start_time.elapsed().unwrap_or_else(|err| {
        eprintln!("Error calculating time{}", err);
        std::time::Duration::default()
    });
    match cli.all {
        true => println!(
            "{}/{} files renamed in {:?}",
            images_renamed, total_images, end_time
        ),
        false => println!(
            "{}/{} images renamed in {:?}",
            images_renamed, total_images, end_time
        ),
    }
}
