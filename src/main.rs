use std::{
    env::current_dir,
    error::Error,
    ffi::OsStr,
    fs::{self, create_dir_all, read_dir, remove_dir},
    path::{Path, PathBuf},
    str::FromStr,
    time::SystemTime,
};

use chrono::{DateTime, Local};
use clap::Parser;
use mime_guess::Mime;
use owo_colors::OwoColorize;

/// Rename images with the date they were created
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Optional prefix for renamed images
    name: Option<String>,

    /// Put custom name after the date
    #[arg(short, long)]
    suffix: bool,

    /// Keep image's original name as prefix
    #[arg(short, long)]
    keep: bool,

    /// Use 12-hour time format instead of 24-hour
    #[arg(short, long)]
    twelve: bool,

    /// Rename all files, not just images
    #[arg(short, long)]
    all: bool,

    /// Set the name of the folder for renamed images (default: Renamed)
    #[arg(short, long, value_name = "Name")]
    folder: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let start_time = SystemTime::now();
    let cli = Cli::parse();
    let renamed_folder: PathBuf = match cli.folder {
        Some(name) => name,
        None => PathBuf::from("renamed"),
    };

    let mut image_name = String::from("");
    let mut name = String::from("");

    if renamed_folder.exists() {
        match renamed_folder.read_dir()?.next().is_none() {
            true => (),
            false => {
                eprintln!(
                    "Folder {} already exists",
                    renamed_folder.to_str().unwrap_or_default()
                );
                return Ok(());
            }
        }
    }
    if let Err(err) = create_dir_all(renamed_folder.clone()) {
        eprintln!("Error creating directory: {}", err);
        return Err(err.into());
    }

    let files = match read_dir(current_dir()?) {
        Ok(files) => files,
        Err(err) => {
            eprintln!("Error reading directory: {}", err);
            return Err(err.into());
        }
    };

    let mut images_renamed: u32 = 0;
    let mut total_images: u32 = 0;

    for file_result in files {
        let file = file_result?;
        let file_path = file.path();

        if file.metadata()?.is_dir() {
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

        if !cli.all
            && !mime_guess::from_path(&file_path)
                .first()
                .unwrap_or(Mime::from_str("Unknown/Unknown")?)
                .to_string()
                .starts_with("image")
        {
            continue;
        }
        total_images += 1;

        if let Some(entered_prefix) = cli.name.as_deref() {
            name = sanitize_filename::sanitize(String::from(entered_prefix).trim()).to_owned() + " "
        }
        if cli.keep {
            image_name = file_name
                .strip_suffix(&format!(".{}", file_extension))
                .unwrap_or_default()
                .to_string()
                + " "
        }

        let file_modified_at_system_time = file.metadata()?.modified()?;
        let file_modified_at_date_time: DateTime<Local> = file_modified_at_system_time.into();
        let image_modified_at_time = match cli.twelve {
            true => file_modified_at_date_time.format("%Y-%m-%d %I_%M_%S %p"),
            false => file_modified_at_date_time.format("%Y-%m-%d %H_%M_%S"),
        };

        let image_destination = match cli.suffix {
            true => PathBuf::from(format!(
                "{}/{}{} {}.{}",
                renamed_folder.to_str().unwrap_or_default(),
                image_name,
                image_modified_at_time,
                name.trim_end(),
                file_extension
            )),
            false => PathBuf::from(format!(
                "{}/{}{}{}.{}",
                renamed_folder.to_str().unwrap_or_default(),
                name,
                image_name,
                image_modified_at_time,
                file_extension
            )),
        };

        if Path::new(&image_destination).exists() {
            eprintln!(
                "{}{}{}",
                "Duplicate creation time:".yellow(),
                file_modified_at_date_time
                    .format("%Y/%m/%d %H:%M:%S %z")
                    .yellow(),
                " File skipped".yellow()
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
                return Err(err.into());
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
        return Ok(());
    }

    let end_time = start_time.elapsed().unwrap_or_else(|err| {
        eprintln!("Error calculating time{}", err);
        std::time::Duration::default()
    });
    match cli.all {
        true => Ok(println!(
            "{}/{} files renamed in {:?}",
            images_renamed, total_images, end_time
        )),
        false => Ok(println!(
            "{}/{} images renamed in {:?}",
            images_renamed, total_images, end_time
        )),
    }
}
