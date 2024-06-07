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
use inquire::MultiSelect;
use mime_guess::Mime;
use owo_colors::OwoColorize;

/// Rename images with the date they were created
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Optional prefix for renamed images
    name: Option<String>,

    /// Choose which files to rename based on file extension
    #[arg(short, long)]
    interactive: bool,

    /// Keep image's original name as prefix
    #[arg(short, long)]
    keep: bool,

    /// Use 12-hour time format instead of 24-hour
    #[arg(short, long)]
    twelve: bool,

    /// Set the name of the folder for renamed images (default: Renamed)
    #[arg(short, long, value_name = "Name")]
    folder: Option<String>,

    /// Put custom name after the date
    #[arg(short, long)]
    suffix: bool,

    /// Rename all files, not just images
    #[arg(short, long)]
    all: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let start_time = SystemTime::now();
    let cli = Cli::parse();
    let renamed_folder: PathBuf = match cli.folder {
        Some(name) => PathBuf::from(sanitize_filename::sanitize(name.trim())),
        None => PathBuf::from("renamed"),
    };

    let mut image_name = String::from("");
    let mut name = String::from("");

    let files = match read_dir(current_dir()?) {
        Ok(files) => files,
        Err(err) => {
            eprintln!("Error reading directory: {}", err);
            return Err(err.into());
        }
    };

    let mut images_renamed: u32 = 0;
    let mut total_images: u32 = 0;

    let mut file_extension_options: Vec<String> = vec![];

    for file_result in files {
        let file = file_result?;
        let file_path = file.path();

        if file.metadata()?.is_dir() {
            continue;
        }

        let file_name = match file.file_name().into_string() {
            Ok(name_string) => name_string,
            Err(_) => {
                eprintln!(
                    "Error converting file name to string {:?}. File skipped",
                    file_path
                );
                continue;
            }
        };

        let file_extension = match file_name.starts_with('.') {
            true => match file_name.strip_prefix('.') {
                Some(extension) => extension,
                None => {
                    eprintln!(
                        "Error getting file extension from {}. File skipped",
                        file_name
                    );
                    continue;
                }
            },
            false => Path::new(&file_path)
                .extension()
                .and_then(OsStr::to_str)
                .unwrap_or_default(),
        };
        match file_extension_options.contains(&file_extension.to_owned()) {
            true => continue,
            false => file_extension_options.push(String::from(file_extension)),
        }
    }

    let extension_selections = match cli.interactive {
        true => {
            let ans =
                MultiSelect::new("Select file types to rename:", file_extension_options).prompt();

            match ans {
                Ok(selections) => selections,
                Err(err) => return Err(err.into()),
            }
        }
        false => vec![String::from("")],
    };

    let files = match read_dir(current_dir()?) {
        Ok(files) => files,
        Err(err) => {
            eprintln!("Error reading directory: {}", err);
            return Err(err.into());
        }
    };

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

    for file_result in files {
        let file = file_result?;
        let file_path = file.path();

        if file.metadata()?.is_dir() {
            continue;
        }

        let file_name = match file.file_name().into_string() {
            Ok(name_string) => name_string,
            Err(_) => {
                eprintln!(
                    "Error converting file name to string {:?}. File skipped",
                    file_path
                );
                continue;
            }
        };

        let file_extension = match file_name.starts_with('.') {
            true => match file_name.strip_prefix('.') {
                Some(extension) => extension,
                None => {
                    eprintln!(
                        "Error getting file extension from {}. File skipped",
                        file_name
                    );
                    continue;
                }
            },
            false => Path::new(&file_path)
                .extension()
                .and_then(OsStr::to_str)
                .unwrap_or_default(),
        };
        if cli.interactive && !extension_selections.contains(&file_extension.to_owned()) {
            continue;
        }
        if !cli.all
            && !cli.interactive
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
                "{}{} {} Skipped",
                "Duplicate creation time:".yellow(),
                file_modified_at_date_time
                    .format("%Y/%m/%d %H:%M:%S %z")
                    .yellow(),
                file_name
            );
            continue;
        };
        match fs::copy(file.path(), image_destination) {
            Ok(_) => images_renamed += 1,
            Err(err) => {
                eprintln!("{}", err);
                if let Err(err) = remove_dir(renamed_folder) {
                    eprintln!("Error copying files: {}", err)
                };
                return Err(err.into());
            }
        }
    }

    if images_renamed == 0 {
        if let Err(err) = remove_dir(renamed_folder) {
            eprintln!("{err}")
        };
        if cli.interactive {
            eprintln!("No files selected");
            return Ok(());
        }
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
    match cli.all || cli.interactive {
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
