use crate::{AppWindow, ProfileData, SearchResults, profile_manager};
use dirs::data_dir;
use dotenv::dotenv;
use image::io::Reader as ImageReader;
use ini::Ini;
use reqwest::get;
use slint::{Image, ModelRc, VecModel};
use std::{
    env,
    error::Error,
    fs::{self},
    io::Cursor,
    path::PathBuf,
    rc::Rc,
    sync::Arc,
    thread,
    time::Instant,
};
use steamgriddb_api::Client;
use steamgriddb_api::query_parameters::QueryType::Grid;
use tokio::runtime::Runtime;

pub fn save_data<'a>(
    title: &'a str,
    temp_path: &str,
    path_profile: &str,
    //cover_image: &str,
    //year: &str,
) -> Result<(), Box<dyn Error>> {
    let data_dir = data_dir().unwrap_or_else(|| {
        println!("Failed to get data directory");
        PathBuf::new()
    });

    let mut profile = {
        let mut path = data_dir;
        path.push("oxide/profiles");
        path
    };

    let mut data = Ini::new();

    //let cover_image = get_cover_image(title)?;
    data.with_section(Some("profile"))
        .set("title", title)
        .set("temp_path", temp_path)
        .set("path_profile", path_profile)
        .set("cover_image", "")
        .set("year", "");

    if !profile.exists() {
        fs::create_dir(&profile)?;
    }

    profile.push(PathBuf::from(format!("{}.ini", title)));

    data.write_to_file(&profile)?;

    println!("Saving data");
    Ok(())
}

pub fn reload_profiles(ui: &Arc<AppWindow>) -> Result<(), Box<dyn Error>> {
    let data_dir = data_dir().unwrap_or_else(|| {
        println!("Failed to get data directory");
        PathBuf::new()
    });

    let profile_path = {
        let mut path = data_dir;
        path.push("oxide/profiles");
        path
    };

    println!("profile_path: {}", profile_path.display());

    if !profile_path.exists() {
        let _ = fs::create_dir_all(&profile_path);
    }

    let start = Instant::now();

    let mut profiles = Vec::new();

    for entry in fs::read_dir(&profile_path)? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        if let Ok(conf) = Ini::load_from_file(&path) {
            if let Some(section) = conf.section(Some("profile")) {
                let title = section.get("title").unwrap_or("Unknown").to_string();
                let try_image = section.get("cover_image").unwrap_or("Unknown").to_string();
                let cover_image = load_cover_image(try_image, title.clone())?;

                let profile_data = ProfileData {
                    cover_image,
                    title: title.into(),
                    year: section.get("year").unwrap_or("Unknown").to_string().into(),
                    path_to_profile: section
                        .get("path_profile")
                        .unwrap_or("Not found?")
                        .to_string()
                        .into(),
                    temp_path: section
                        .get("temp_path")
                        .unwrap_or("Not found?")
                        .to_string()
                        .into(),
                    name: section.get("name").unwrap_or("Unknown").to_string().into(),
                };

                profiles.push(profile_data);
            }
        }
    }

    println!(
        "Time to reload profile data: {:.2}ms",
        start.elapsed().as_millis()
    );

    let profiles_model = Rc::new(VecModel::from(profiles.clone()));
    let profiles_model_rc = ModelRc::from(profiles_model);
    println!("profiles: {}", profiles.len());

    ui.set_profiles(profiles_model_rc);

    Ok(())
}

fn load_cover_image(path: String, title: String) -> Result<Image, Box<dyn Error>> {
    println!("Trying image in ini file: {}", path);
    if PathBuf::from(&path).exists() {
        let image = slint::Image::load_from_path(&PathBuf::from(&path))?;
        return Ok(image);
    }

    println!("Trying image with same name: {}", title);
    let data_dir = data_dir().unwrap_or_else(|| {
        println!("Failed to get data directory");
        PathBuf::new()
    });

    let profile = {
        let mut path = data_dir;
        let profile_path = PathBuf::from(format!("oxide/profiles/{}.png", title));
        path.push(profile_path);
        path
    };

    if PathBuf::from(&profile).exists() {
        println!("Image found!");
        let image = slint::Image::load_from_path(&PathBuf::from(&profile))?;
        return Ok(image);
    } else {
        let rt = Runtime::new()?;

        if !path.is_empty() {
            println!("Trying image from steamgriddb: {}", title);
            match rt.block_on(profile_manager::get_cover_image(&title)) {
                Ok(..) => {
                    return Ok(slint::Image::load_from_path(&PathBuf::from(&profile))?);
                }
                Err(e) => {
                    println!("Error: {}", e);
                }
            }
        }
    }

    let notfound_path = if std::env::var("FLATPAK_ID").is_ok() {
        "/app/share/oxide/notfound.png"
    } else {
        "notfound.png"
    };

    println!("Falling back to notfound.png");
    if let Ok(image) = slint::Image::load_from_path(&PathBuf::from(notfound_path)) {
        Ok(image)
    } else {
        Err("Failed to load image".into())
    }
}

pub async fn get_cover_image(title: &str) -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = data_dir().unwrap_or_else(|| {
        println!("Failed to get data directory");
        PathBuf::new()
    });

    let profile = {
        let mut path = data_dir;
        let profile_path = PathBuf::from(format!("oxide/profiles/{}.png", title));
        path.push(profile_path);
        path
    };

    println!("profile: {}", profile.display());

    if !profile.exists() {
        download_image(title, &profile).await?;
    }
    Ok(())
}

pub async fn download_image(
    title: &str,
    profile: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let client_key = env::var("STEAMGRIDDB_API_KEY").expect("STEAMGRIDDB_API_KEY not set");
    let client = Client::new(client_key);
    let games = client.search(title).await?;
    let first_game = games.iter().next().ok_or("No games found")?;

    if first_game.name != title {
        return Err(format!("Game not found: {}", title).into());
    }

    assert_eq!(title, first_game.name);
    let images = client.get_images_for_id(first_game.id, &Grid(None)).await?;
    let first_image = images.iter().next().ok_or("No images found")?;

    println!("Getting image from: {:?}", first_image.url);

    let response = get(&first_image.url).await?;

    if !response.status().is_success() {
        return Err(format!(
            "Failed to download image: HTTP status code {}",
            response.status()
        )
        .into());
    }

    let image_bytes = response.bytes().await?;

    println!("Image data downloaded.");

    let cursor = Cursor::new(image_bytes);
    let reader = ImageReader::new(cursor).with_guessed_format()?;

    let img = reader.decode()?;

    println!("Image decoded successfully.");

    if profile.exists() {
        fs::remove_file(&profile)?;
    }

    img.save(profile)?;

    println!("Image saved");

    Ok(())
}

pub async fn search_steamgrid(
    title: &str,
    search: &str,
    ui: &Arc<AppWindow>,
) -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let client_key = match env::var("STEAMGRIDDB_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            println!("STEAMGRIDDB_API_KEY not set");
            return Err("STEAMGRIDDB_API_KEY not set".into());
        }
    };

    let mut results = Vec::new();

    let data_dir = data_dir().unwrap_or_else(|| {
        println!("Failed to get data directory");
        PathBuf::new()
    });

    let profile = {
        let mut path = data_dir;
        let profile_path = PathBuf::from(format!("oxide/profiles/{}.png", title));
        path.push(profile_path);
        path
    };

    println!("profile: {}", profile.display());

    let client = Client::new(client_key);
    let games = client.search(search).await?;
    let first_game = games.iter().next().ok_or("No games found")?;

    for game in &games {
        println!("Found game: {}", game.name);

        let result_data = SearchResults {
            SearchSteam: game.name.clone().into(),
        };

        results.push(result_data.clone());
    }

    println!("Games: {}", games.len());

    let results_model = Rc::new(VecModel::from(results.clone()));
    let results_model_rc = ModelRc::from(results_model);

    ui.set_SearchResults(results_model_rc);

    Ok(())
}
