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
    let renamed_folder: PathBuf = match cli.folder.as_deref() {
        Some(name) => PathBuf::from(sanitize_filename::sanitize(name.trim())),
        None => PathBuf::from("renamed"),
    };

    let image_name = String::from("");
    let name = String::from("");

    let files = match read_dir(current_dir()?).await {
        Ok(files) => files,
        Err(err) => {
            eprintln!("{}{}", "Error reading directory: ".red(), err.red());
            return Err(err.into());
        }
    };
    let extension_selections = match cli.extension {
        true => match get_extensions(files).await {
            Ok(selections) => selections,
            Err(_) => vec![],
        },
        false => vec![],
    };

    let start_time = SystemTime::now();

    if !cli.preview {
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
                    remove_dir_all(renamed_folder.as_ref()).await.unwrap();
                    return Ok(());
                }
                1 => {
                    eprintln!(
                        "{} Duplicate creation time found. (Use '{}' or '{}' to include original unique names)",
                        "Error:".red(),
                        "-k".yellow(),
                        "--keep".yellow()
                    );
                    remove_dir_all(renamed_folder.as_ref()).await.unwrap();
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

            let file_name = match file.file_name().into_string() {
                Ok(name_string) => name_string,
                Err(_) => {
                    eprintln!(
                        "{}{:?}{}",
                        "Error converting file name to string ".red(),
                        file_path.red(),
                        ". File skipped".red()
                    );
                    return;
                }
            };

            let file_extension = match file_name.starts_with('.') {
                true => match file_name.strip_prefix('.') {
                    Some(extension) => extension,
                    None => {
                        eprintln!(
                            "{}{}{}",
                            "Error getting file extension from ".red(),
                            file_name.red(),
                            ". File skipped".red()
                        );
                        return;
                    }
                },
                false => Path::new(&file_path)
                    .extension()
                    .and_then(OsStr::to_str)
                    .unwrap_or_default(),
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
                name = match cli.front {
                    true => {
                        " ".to_owned()
                            + &sanitize_filename::sanitize(String::from(entered_prefix).trim())
                                .to_owned()
                    }
                    false => match cli.suffix {
                        true => {
                            " ".to_owned()
                                + &sanitize_filename::sanitize(String::from(entered_prefix).trim())
                        }
                        false => {
                            sanitize_filename::sanitize(String::from(entered_prefix).trim())
                                .to_owned()
                                + " "
                        }
                    },
                }
            }
            if cli.keep {
                image_name = match cli.front {
                    true => {
                        " ".to_owned()
                            + file_name
                                .strip_suffix(&format!(".{}", file_extension))
                                .unwrap_or_default()
                    }
                    false => {
                        file_name
                            .strip_suffix(&format!(".{}", file_extension))
                            .unwrap_or_default()
                            .to_string()
                            + " "
                    }
                }
            }

            let file_modified_at_system_time = file.metadata().await.unwrap().modified().unwrap();
            let file_modified_at_date_time: DateTime<Local> = file_modified_at_system_time.into();
            let image_modified_at_time = match cli.date {
                true => file_modified_at_date_time.format("%Y-%m-%d"),
                false => match cli.twelve {
                    true => file_modified_at_date_time.format("%Y-%m-%d %I_%M_%S %p"),
                    false => file_modified_at_date_time.format("%Y-%m-%d %H_%M_%S"),
                },
            };

            let image_destination = match cli.suffix {
                true => match cli.front {
                    true => PathBuf::from(format!(
                        "{}/{}{}{}.{}",
                        renamed_folder.to_str().unwrap_or_default(),
                        image_modified_at_time,
                        image_name,
                        name.trim_end(),
                        file_extension
                    )),

                    false => PathBuf::from(format!(
                        "{}/{}{}{}.{}",
                        renamed_folder.to_str().unwrap_or_default(),
                        image_name,
                        image_modified_at_time,
                        name.trim_end(),
                        file_extension
                    )),
                },
                false => match cli.front {
                    true => PathBuf::from(format!(
                        "{}/{}{}{}.{}",
                        renamed_folder.to_str().unwrap_or_default(),
                        image_modified_at_time,
                        name,
                        image_name,
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
                },
            };

            if cli.preview {
                println!("{:?}", image_destination);
                return;
            }

            if Path::new(&image_destination).exists() {
                *duplicate.lock().await += 1;
                return;
            }
            if *duplicate.lock().await > 0 {
                return;
            }

            const MAX_RETRIES: usize = 3;
            const RETRY_DELAY_MS: u64 = 100;
            let mut attempt = 0;

            loop {
                let result = fs::copy(file.path(), image_destination.clone()).await;
                match result {
                    Ok(_) => {
                        *images_renamed.lock().await += 1;
                        break;
                    }
                    Err(_) => {
                        // eprintln!("{}{}. Retrying...", "Error copying file: ".yellow(), err);
                        attempt += 1;
                        if attempt >= MAX_RETRIES {
                            eprintln!(
                                "{}{}",
                                "Max retries reached. Skipping file: ".red(),
                                file_name.red()
                            );
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(RETRY_DELAY_MS)).await;
                    }
                }
            }
        });
        tasks.push(task)
    }
    for task in tasks {
        task.await.unwrap();
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
        if let Err(err) = remove_dir(renamed_folder.as_ref()).await {
            eprintln!("{err}")
        };
        if cli.extension {
            eprintln!("No files selected");
            return Ok(());
        }
        match cli.all {
            true => eprintln!("No files found"),
            false => {
                eprintln!(
                    "No images or wrong image formats. (Use '{}' or '{}' to rename any files found)","-a".yellow(),"--all".yellow()
                )
            }
        }
        return Ok(());
    }

    let end_time = start_time.elapsed().unwrap_or_else(|err| {
        eprintln!("Error calculating time{}", err);
        std::time::Duration::default()
    });
    match cli.all || cli.extension {
        true => Ok(println!(
            "{}/{} Files renamed in {:?}",
            images_renamed, total_images, end_time
        )),
        false => Ok(println!(
            "{}/{} Images renamed in {:?}",
            images_renamed, total_images, end_time
        )),
    }
}

async fn get_extensions(mut files: ReadDir) -> Result<Vec<String>, inquire::InquireError> {
    let mut file_extension_options: Vec<String> = vec![];
    while let Ok(Some(file)) = files.next_entry().await {
        let file_path = file.path();

        if file.metadata().await.unwrap().is_dir() {
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
