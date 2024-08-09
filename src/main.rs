use std::{
    env::current_dir,
    error::Error,
    ffi::OsStr,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
    time::{Duration, SystemTime},
};

use chrono::{DateTime, Local};
use clap::Parser;
use inquire::{
    ui::{RenderConfig, Styled},
    MultiSelect,
};
use mime_guess::Mime;
use owo_colors::OwoColorize;
use tokio::{
    fs::{self, create_dir_all, read_dir, remove_dir, remove_dir_all, ReadDir},
    sync::{Mutex, Semaphore},
    task::JoinHandle,
};
static PERMITS: Semaphore = Semaphore::const_new(15);

/// Rename images with the date they were created
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Optional prefix for renamed images
    name: Option<String>,

    /// Choose which files to rename based on file extension
    #[arg(short, long)]
    extension: bool,

    /// Put date in front of filename
    #[arg(short, long)]
    front: bool,

    /// Keep image's original name as prefix
    #[arg(short, long)]
    keep: bool,

    /// Use 12-hour time format instead of 24-hour
    #[arg(short, long)]
    twelve: bool,

    /// Date without time
    #[arg(short, long)]
    date: bool,

    /// Set the name of the folder for renamed images (default: Renamed)
    #[arg(short = 'F', long, value_name = "Name")]
    folder: Option<String>,

    /// Put custom name after the date
    #[arg(short, long)]
    suffix: bool,

    /// Preview the name format of renamed files
    #[arg(short, long)]
    preview: bool,

    /// Rename all files, not just images
    #[arg(short, long)]
    all: bool,
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    inquire::set_global_render_config(get_render_config());
    let cli = Arc::new(Cli::parse());
    let renamed_folder: PathBuf = if let Some(name) = cli.folder.as_deref() {
        PathBuf::from(sanitize_filename::sanitize(name.trim()))
    } else {
        PathBuf::from("renamed")
    };

    let image_name = String::new();
    let name = String::new();

    let files = match read_dir(current_dir()?).await {
        Ok(files) => files,
        Err(err) => {
            eprintln!("{}{}", "Error reading directory: ".red(), err.red());
            return Err(err.into());
        }
    };
    let extension_selections = if cli.extension {
        if let Ok(selections) = get_extensions(files).await {
            selections
        } else {
            vec![]
        }
    } else {
        vec![]
    };

    let start_time = SystemTime::now();

    if !cli.preview {
        if renamed_folder.exists() && renamed_folder.read_dir()?.next().is_some() {
            {
                eprintln!(
                    "Folder {} already exists",
                    renamed_folder.to_str().unwrap_or_default()
                );
                return Ok(());
            }
        }
        if let Err(err) = create_dir_all(renamed_folder.clone()).await {
            eprintln!("{}{}", "Error creating directory: ".red(), err.red());
            return Err(err.into());
        }
    }
    let renamed_folder = Arc::new(renamed_folder);
    let cli_clone = Arc::clone(&cli);
    let renamed_folder_clone = Arc::clone(&renamed_folder);
    let duplicate: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));

    let (images_renamed, total_images, _duplicate) = match copy_files(
        cli_clone,
        renamed_folder_clone,
        extension_selections,
        name,
        image_name,
        duplicate,
    )
    .await
    {
        Ok(result) => {
            if cli.preview {
                return Ok(());
            }
            match result.2 {
                0 => result,
                count if count > 1 => {
                    eprintln!(
                        "{} Duplicate creation times found. (Use '{}' or '{}' to include original unique names)",
                        "Error:".red(),
                        "-k".yellow(),
                        "--keep".yellow()
                    );
                    remove_dir_all(renamed_folder.as_ref()).await?;
                    return Ok(());
                }
                1 => {
                    eprintln!(
                        "{} Duplicate creation time found. (Use '{}' or '{}' to include original unique names)",
                        "Error:".red(),
                        "-k".yellow(),
                        "--keep".yellow()
                    );
                    remove_dir_all(renamed_folder.as_ref()).await?;
                    return Ok(());
                }
                _ => return Ok(()),
            }
        }
        Err(err) => return Err(err),
    };
    let renamed_folder = Arc::clone(&renamed_folder);
    let cli = Arc::clone(&cli);

    print_summary(
        start_time,
        images_renamed,
        total_images,
        renamed_folder,
        cli,
    )
    .await
}

async fn copy_files(
    cli: Arc<Cli>,
    renamed_folder: Arc<PathBuf>,
    extension_selections: Vec<String>,
    name: String,
    image_name: String,
    duplicate: Arc<Mutex<u32>>,
) -> Result<(u32, u32, u32), Box<dyn Error>> {
    let mut files = match read_dir(current_dir()?).await {
        Ok(files) => files,
        Err(err) => {
            eprintln!("{}{}", "Error reading directory: ".red(), err.red());
            return Err(err.into());
        }
    };

    let total_images: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));
    let images_renamed: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));
    let mut tasks: Vec<JoinHandle<()>> = vec![];

    while let Ok(Some(file)) = files.next_entry().await {
        let total_images = Arc::clone(&total_images);
        let images_renamed = Arc::clone(&images_renamed);
        let renamed_folder = Arc::clone(&renamed_folder);
        let duplicate = Arc::clone(&duplicate);
        let cli = Arc::clone(&cli);
        let extension_selections = extension_selections.clone();
        let mut name = name.clone();
        let mut image_name = image_name.clone();
        let task = tokio::task::spawn(async move {
            let _permit = PERMITS.acquire().await.unwrap();
            let file_path = file.path();

            if file.metadata().await.unwrap().is_dir() {
                return;
            }

            let Ok(file_name) = file.file_name().into_string() else {
                eprintln!(
                    "{}{:?}{}",
                    "Error converting file name to string ".red(),
                    file_path.red(),
                    ". File skipped".red()
                );
                return;
            };

            let mut dotfile = false;
            let file_extension = if file_name.starts_with('.') {
                if let Some(extension) = file_name.strip_prefix('.') {
                    dotfile = true;
                    extension
                } else {
                    {
                        eprintln!(
                            "{}{}{}",
                            "Error getting file extension from ".red(),
                            file_name.red(),
                            ". File skipped".red()
                        );
                        return;
                    }
                }
            } else {
                Path::new(&file_path)
                    .extension()
                    .and_then(OsStr::to_str)
                    .unwrap_or_default()
            };
            if cli.extension && !extension_selections.contains(&file_extension.to_owned()) {
                return;
            }
            if !cli.all
                && !cli.extension
                && !mime_guess::from_path(&file_path)
                    .first()
                    .unwrap_or(Mime::from_str("Unknown/Unknown").unwrap())
                    .to_string()
                    .starts_with("image")
            {
                return;
            }
            *total_images.lock().await += 1;

            if let Some(entered_prefix) = cli.name.as_deref() {
                name = if cli.front {
                    " ".to_owned()
                        + &sanitize_filename::sanitize(String::from(entered_prefix).trim()).clone()
                } else if cli.suffix {
                    " ".to_owned()
                        + &sanitize_filename::sanitize(String::from(entered_prefix).trim())
                } else {
                    sanitize_filename::sanitize(String::from(entered_prefix).trim()).clone() + " "
                }
            }
            if cli.keep {
                image_name = if cli.front {
                    " ".to_owned()
                        + file_name
                            .strip_suffix(&format!(".{file_extension}"))
                            .unwrap_or_default()
                } else if dotfile {
                    file_name
                        .strip_suffix(&format!(".{file_extension}"))
                        .unwrap_or_default()
                        .to_string()
                } else {
                    file_name
                        .strip_suffix(&format!(".{file_extension}"))
                        .unwrap_or_default()
                        .to_string()
                        + " "
                }
            }

            let file_modified_at_system_time = file.metadata().await.unwrap().modified().unwrap();
            let file_modified_at_date_time: DateTime<Local> = file_modified_at_system_time.into();
            let image_modified_at_time = if cli.date {
                file_modified_at_date_time.format("%Y-%m-%d")
            } else if cli.twelve {
                file_modified_at_date_time.format("%Y-%m-%d %I_%M_%S %p")
            } else {
                file_modified_at_date_time.format("%Y-%m-%d %H_%M_%S")
            };

            let image_destination = if cli.suffix {
                if cli.front {
                    PathBuf::from(format!(
                        "{}/{}{}{}.{}",
                        renamed_folder.to_str().unwrap_or_default(),
                        image_modified_at_time,
                        image_name,
                        name.trim_end(),
                        file_extension
                    ))
                } else {
                    PathBuf::from(format!(
                        "{}/{}{}{}.{}",
                        renamed_folder.to_str().unwrap_or_default(),
                        image_name,
                        image_modified_at_time,
                        name.trim_end(),
                        file_extension
                    ))
                }
            } else if cli.front {
                PathBuf::from(format!(
                    "{}/{}{}{}.{}",
                    renamed_folder.to_str().unwrap_or_default(),
                    image_modified_at_time,
                    name,
                    image_name,
                    file_extension
                ))
            } else {
                PathBuf::from(format!(
                    "{}/{}{}{}.{}",
                    renamed_folder.to_str().unwrap_or_default(),
                    name,
                    image_name,
                    image_modified_at_time,
                    file_extension
                ))
            };

            if cli.preview {
                println!("{image_destination:?}");
                return;
            }

            if Path::new(&image_destination).exists() {
                *duplicate.lock().await += 1;
                return;
            }
            if *duplicate.lock().await > 0 {
                return;
            }

            let max_retries: u8 = 3;
            let retry_delay_ms: u64 = 100;
            let mut attempt: u8 = 0;

            loop {
                let copy_result = fs::copy(file.path(), image_destination.clone()).await;
                if copy_result.is_ok() {
                    *images_renamed.lock().await += 1;
                    break;
                } else {
                    // eprintln!("{}{}. Retrying...", "Error copying file: ".yellow(), err);
                    attempt += 1;
                    if attempt >= max_retries {
                        eprintln!(
                            "{}{}",
                            "Max retries reached. Skipping file: ".red(),
                            file_name.red()
                        );
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(retry_delay_ms)).await;
                }
            }
        });
        tasks.push(task);
    }
    for task in tasks {
        task.await?;
    }

    let result = (
        *images_renamed.lock().await,
        *total_images.lock().await,
        *duplicate.lock().await,
    );
    Ok(result)
}

async fn print_summary(
    start_time: SystemTime,
    images_renamed: u32,
    total_images: u32,
    renamed_folder: Arc<PathBuf>,
    cli: Arc<Cli>,
) -> Result<(), Box<dyn Error>> {
    let renamed_folder = Arc::clone(&renamed_folder);
    let cli = Arc::clone(&cli);
    if images_renamed == 0 {
        remove_dir(renamed_folder.as_ref()).await?;
        if cli.extension {
            eprintln!("No files selected");
            return Ok(());
        }
        if cli.all {
            eprintln!("No files found");
        } else {
            eprintln!(
                "No images or wrong image formats. (Use '{}' or '{}' to rename any files found)",
                "-a".yellow(),
                "--all".yellow()
            );
        }
        return Ok(());
    }

    let end_time = start_time.elapsed().unwrap_or_else(|err| {
        eprintln!("Error calculating time{err}");
        std::time::Duration::default()
    });
    if cli.all || cli.extension {
        Ok(println!(
            "{}/{} Files renamed in {:?}",
            images_renamed, total_images, end_time
        ))
    } else {
        Ok(println!(
            "{}/{} Images renamed in {:?}",
            images_renamed, total_images, end_time
        ))
    }
}

async fn get_extensions(mut files: ReadDir) -> Result<Vec<String>, inquire::InquireError> {
    let mut file_extension_options: Vec<String> = vec![];
    while let Ok(Some(file)) = files.next_entry().await {
        let file_path = file.path();

        if file.metadata().await?.is_dir() {
            continue;
        }

        let Ok(file_name) = file.file_name().into_string() else {
            eprintln!(
                "Error converting file name to string {:?}. File skipped",
                file_path
            );
            continue;
        };

        let file_extension = if file_name.starts_with('.') {
            if let Some(extension) = file_name.strip_prefix('.') {
                extension
            } else {
                eprintln!(
                    "Error getting file extension from {}. File skipped",
                    file_name
                );
                continue;
            }
        } else {
            Path::new(&file_path)
                .extension()
                .and_then(OsStr::to_str)
                .unwrap_or_default()
        };
        if file_extension_options.contains(&file_extension.to_owned()) {
            continue;
        } else {
            file_extension_options.push(String::from(file_extension));
        }
    }

    match MultiSelect::new("Select file types to rename:", file_extension_options).prompt() {
        Ok(selections) => Ok(selections),
        Err(err) => Err(err),
    }
}

fn get_render_config() -> RenderConfig<'static> {
    RenderConfig::<'_> {
        unselected_checkbox: Styled::new("○"),
        selected_checkbox: Styled::new("●"),
        ..Default::default()
    }
}
