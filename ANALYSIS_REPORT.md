# OxideManager Codebase Analysis Report

## Executive Summary
This report details the findings from a comprehensive analysis of the OxideManager codebase. The analysis identified critical functional bugs, code quality issues, and potential stability problems. The most critical issue is a race condition in the file extraction process that likely renders the core "install mod" functionality broken.

## 1. Critical Bugs & Functional Issues

### 1.1 Race Condition in Archive Extraction
**Severity**: **Critical**
**Location**: `src/extract.rs` (lines 19-31), `src/main.rs` (lines 112-125)
**Description**: The `extract_file` function spawns a detached thread to perform the extraction but returns `Ok(())` immediately to the caller. The `main.rs` logic assumes extraction is synchronous and immediately proceeds to copy files from the extraction target. Since the extraction thread is running in the background (or hasn't even started writing files yet), the copy operation will fail to find the files, resulting in an incomplete or failed installation without clear error feedback.
**Recommendation**: Remove `thread::spawn` from `extract_file` to make it synchronous, or implement a proper async/callback mechanism to await completion before proceeding.

### 1.2 Incorrect Path Construction in Extraction
**Severity**: **High**
**Location**: `src/extract.rs` (line 17)
**Description**: The `extract_file` function constructs the destination path `extract_to_owned` by concatenating `data_dir` with the passed `extract_to` path. However, the `extract_to` path passed from `main.rs` is already an absolute path (constructed using `data_dir` in `main.rs`). This results in a malformed, duplicated path (e.g., `/home/user/.../home/user/...`), causing extraction to fail or write to the wrong location.
**Recommendation**: Use the `extract_to` path directly as it is already absolute, or refactor `main.rs` to pass a relative path.

### 1.3 Incorrect 7z Handling
**Severity**: **High**
**Location**: `src/extract.rs` (lines 25-27)
**Description**: The handler for `.7z` files incorrectly calls `extract_zip` instead of the available `extract_7z` function. The `zip` crate does not support 7z archives, so this will fail.
**Recommendation**: Change the function call to `extract_7z` for the "7z" match arm.

### 1.4 Unhandled API Key Missing
**Severity**: **Medium**
**Location**: `src/profile_manager.rs` (line 209)
**Description**: The application uses `.expect()` when retrieving `STEAMGRIDDB_API_KEY`. If this environment variable is missing, the application will panic and crash.
**Recommendation**: Handle the missing environment variable gracefully. Return an error or disable the cover image download functionality instead of crashing.

### 1.5 Panic on Directory Access
**Severity**: **Medium**
**Location**: `src/main.rs` (line 102)
**Description**: The application explicitly panics (`panic!("Failed to read directory")`) if `fs::read_dir` fails. This can happen due to permissions or other FS issues, leading to a poor user experience.
**Recommendation**: Handle the error by showing a message in the UI and logging the error, rather than crashing.

## 2. Code Quality & Technical Debt

### 2.1 Blocking I/O on UI Thread
**Location**: `src/profile_manager.rs`, `src/main.rs`
**Description**: Functions like `load_cover_image` and `search_steamgrid` use `rt.block_on()` to execute async code. When called from the main UI thread, this blocks the entire application interface until the network operation completes.
**Recommendation**: Integrate a proper async runtime (like `tokio`) with the application lifecycle, or offload these tasks to background threads and update the UI via thread-safe handles.

### 2.2 Inefficient Runtime Management
**Location**: `src/profile_manager.rs`
**Description**: A new Tokio `Runtime` is created (`Runtime::new()`) inside multiple functions (`load_cover_image`, `search_steamgrid`). Creating a runtime is an expensive operation.
**Recommendation**: Initialize a single global `Runtime` or shared `Arc<Runtime>` at application startup and reuse it.

### 2.3 Hardcoded Paths and Strings
**Location**: Global
**Description**: Paths like `"oxide/profiles"`, `".temp"`, and filenames are hardcoded as string literals throughout the codebase. This makes maintenance, refactoring, and cross-platform support difficult.
**Recommendation**: Define these as constants in a configuration module.

### 2.4 Fragile Path Manipulation
**Location**: `src/file_manager.rs`, `src/extract.rs`
**Description**: The code frequently uses string formatting (`format!`) and string splitting to manipulate file paths. This is error-prone and may not handle edge cases or different OS path separators correctly.
**Recommendation**: consistently use `std::path::Path` and `std::path::PathBuf` methods (`join`, `parent`, `file_name`) for all path operations.

### 2.5 Recursive Stack Overflow Risk
**Location**: `src/file_manager.rs` (`copy_to_dir`)
**Description**: The `copy_to_dir` function is recursive without a depth limit. While likely fine for typical game mods, deep directory structures could cause a stack overflow.
**Recommendation**: Consider using an iterative approach (e.g., using the `walkdir` crate) or ensure recursion depth is safe.

## 3. Security Analysis

### 3.1 Zip Slip Mitigation
**Observation**: The `extract_zip` function correctly uses `file.enclosed_name()` to prevent "Zip Slip" vulnerabilities (writing outside the target directory). This is a good practice.

### 3.2 Symlink Handling
**Location**: `src/file_manager.rs`
**Observation**: The code handles symlinks but blindly follows or removes them in some cases. While mitigated by the scope of the app, care should be taken when processing untrusted archives containing symlinks.

## 4. Missing Error Handling

**General Observation**:
There is a widespread pattern of swallowing errors or printing them to stdout without informing the user.
- `extract_file` returns `Ok` even if the extraction thread fails (because it's detached and errors aren't propagated).
- `copy_to_dir` continues or returns partial success in some branches.
- `unwrap_or` is often used to mask errors with default values (like empty strings or paths), which leads to confusing behavior later in the execution flow (e.g., operations on empty paths).

**Recommendation**: Implement a robust error propagation strategy. Use `Result` consistently and bubble errors up to the UI layer to display meaningful error messages to the user.
