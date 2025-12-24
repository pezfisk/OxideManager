# Flatpak Persistent Directory Access

## Problem
When published on Flathub, the application was unable to persistently access user-selected directories. Using `rfd` (Rust File Dialog) with xdg-portal gave only ephemeral access, meaning the app would lose access to selected directories after closing.

## Solution
Replaced `rfd` with `ashpd` (Async Handler for Portal Desktop), which properly implements the xdg-desktop-portal protocol with persistent access support.

### Changes Made

1. **Cargo.toml**: Replaced `rfd = "0.15.2"` with `ashpd = "0.12.0"`

2. **src/main.rs**: 
   - Added `ashpd::desktop::file_chooser::OpenFileRequest` import
   - Created `pick_directory_persistent()` async function that:
     - Uses `OpenFileRequest` with `directory(true)` for folder selection
     - Returns paths from file:// URIs
     - Automatically grants persistent access through the portal
   - Updated both `on_request_archive_path` and `on_request_game_path` callbacks to use the new portal-based picker

3. **dev.invrs.oxide.yaml**: 
   - Removed `--filesystem=home` finish-arg
   - App now requests only the specific directories users select, improving security

## How It Works

The xdg-desktop-portal file chooser automatically grants persistent access to directories selected by the user. This access is stored by the portal and remains available across app launches without needing broad filesystem permissions.

### Benefits

- ✅ **Persistent Access**: Directories remain accessible after app restart
- ✅ **Better Security**: No need for `--filesystem=home` permission
- ✅ **User Control**: Users explicitly grant access to specific directories
- ✅ **Portal-Native**: Works seamlessly with Flatpak's security model
- ✅ **No finish-args**: Achieves the goal without adding filesystem finish-args

## Technical Details

When the user selects a directory through `OpenFileRequest`:
1. The portal shows the native file picker dialog
2. User selects a directory
3. Portal grants the app persistent access to that directory
4. Access is stored in the portal's permission store
5. Subsequent app launches can access the same directory without re-prompting

This is the recommended approach for Flatpak applications that need persistent file/directory access.
