# Suggested Code Fixes

This document outlines the specific code changes required to address the critical issues identified in the codebase analysis.

## 1. Fix Critical Race Condition and Path Issues in Extraction

**File:** `src/extract.rs`

**Description:**
The current implementation creates a background thread for extraction but does not wait for it to finish. It also incorrectly constructs the destination path by concatenating the data directory twice. Finally, it uses the wrong extractor for `.7z` files.

**Suggested Fix:**

```rust
pub fn extract_file(archive_path: &str, extract_to: &Path) -> Result<(), Box<dyn Error>> {
    if let Some(extension) = Path::new(&archive_path).extension() {
        // Fix: Use the passed absolute path directly and append "extracted".
        // The calling code (main.rs) already provides a full path rooted in data_dir.
        let extract_to_owned = extract_to.join("extracted");
        let extract_to_str = extract_to_owned.to_str().ok_or("Invalid path")?;
        
        let archive_path_owned = archive_path.to_string();

        // Fix: Remove thread::spawn to make operation synchronous so the caller waits for completion.
        // Fix: Use extract_7z for "7z" extension.
        match extension.to_str().unwrap_or("") {
            "zip" => extract_zip(&archive_path_owned, extract_to_str)?,
            "rar" => extract_rar(&archive_path_owned, extract_to_str)?,
            "7z" => extract_7z(&archive_path_owned, extract_to_str)?, 
            ext => return Err(format!("Unsupported extension: {}", ext).into()),
        };
    }

    Ok(())
}
```

## 2. Safe API Key Handling

**File:** `src/profile_manager.rs`

**Description:**
The application panics if `STEAMGRIDDB_API_KEY` is not set.

**Suggested Fix:**

```rust
pub async fn download_image(
    title: &str,
    profile: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    // Fix: Return error instead of panicking with .expect()
    let client_key = env::var("STEAMGRIDDB_API_KEY")
        .map_err(|_| "STEAMGRIDDB_API_KEY environment variable not set")?;
        
    let client = Client::new(client_key);
    // ... rest of function
}

// Apply similar fix to search_steamgrid
```

## 3. Prevent Panic on Directory Access

**File:** `src/main.rs`

**Description:**
`main.rs` panics if `fs::read_dir` fails (e.g. permission denied).

**Suggested Fix:**

```rust
// Inside ui.on_mod(...)
            println!("Path: '{}'", path.display());
            ui_copy.set_progress(0.1);
            if path.exists() {
                // Fix: Handle read_dir error gracefully
                let entries = match fs::read_dir(&*path) {
                    Ok(x) => x,
                    Err(e) => {
                        eprintln!("Failed to read directory: {}", e);
                        if let Some(ui) = ui_handle.upgrade() {
                             ui.set_footer(SharedString::from(format!("Error: {}", e)));
                        }
                        return;
                    }
                };

                for entry in entries {
                    let Ok(entry) = entry else { continue };
                    // ... existing logic ...
```

## 4. Efficient and Non-Blocking Async Management

**Files:** `src/profile_manager.rs`, `src/main.rs`

**Description:**
The application creates a new Tokio `Runtime` for every async operation, which is inefficient. It also uses `block_on` on the main thread, freezing the UI.

**Suggested Fix (Shared Runtime):**

Create a helper to reuse the runtime.

```rust
// src/profile_manager.rs or a new utils module
use std::sync::OnceLock;
use tokio::runtime::Runtime;

pub fn get_runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| Runtime::new().expect("Failed to create runtime"))
}
```

**Suggested Fix (Non-blocking Callback):**

Update `main.rs` to spawn a thread for async tasks.

```rust
// src/main.rs

        ui.on_download_profile_image(move |title, search_game| {
            println!("Downloading image: {}, {}", title, search_game);
            let ui_weak = Arc::downgrade(&ui_copy);
            
            // Calculate paths here or clone required data
            let data_dir = data_dir().unwrap_or_default();
            let profile_path = data_dir.join("oxide/profiles").join(format!("{}.png", title));

            // Spawn a thread to offload the blocking task
            std::thread::spawn(move || {
                let rt = profile_manager::get_runtime();
                
                // Run the async task
                let result = rt.block_on(profile_manager::download_image(&search_game, &profile_path));

                // Update UI on the main thread
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak.upgrade() {
                        match result {
                            Ok(_) => {
                                if let Ok(image) = slint::Image::load_from_path(&profile_path) {
                                    ui.set_selected_cover_image(image);
                                }
                            }
                            Err(e) => {
                                println!("Failed to download image: {}", e);
                            }
                        }
                    }
                });
            });
        });
```

## 5. Correct Path Handling in FileManager

**File:** `src/file_manager.rs`

**Description:**
There is a potential issue with `path.join(profile_slash)` if `profile` is absolute. While `Path::join` handles absolute paths by replacing the base, explicit logic is safer and clearer.

**Suggested Fix:**

```rust
// src/file_manager.rs

pub fn copy_to_dir(...) {
    // ...
    // Determine the source directory for extraction artifacts
    let walk_dir = profile.join("extracted").join(start_point);
    
    // Check if it exists before reading
    if !walk_dir.exists() {
         eprintln!("Source directory does not exist: {}", walk_dir.display());
         return Err("Source directory missing".into());
    }
    // ...
}
```
