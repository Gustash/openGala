use std::path::PathBuf;

use confy::ConfyError;
use thiserror::Error;
use tokio::process::Command;

#[derive(Error, Debug)]
pub enum FreeCarnivalError {
    #[error("Could not find game in library")]
    GameNotFound,
    #[error("Failed to fetch latest build number. Cannot install")]
    LatestBuild,
    #[error("Some chunks failed verification. Failed to install game")]
    Verify,
    #[error("Your authentication is not valid")]
    Auth,
    #[error("Login failed: {0}")]
    Login(String),
    #[error("Failed to parse login response")]
    LoginParse,
    #[error("{0} is already installed")]
    AlreadyInstalled(String),
    #[error("{0} is not installed")]
    NotInstalled(String),
    #[error("Can't find or install build {version} for {slug}")]
    InstallBuild { version: String, slug: String },
    #[error("Failed to load {0} config: {1}")]
    LoadConfig(&'static str, ConfyError),
    #[error("Failed to save {0} config: {1}")]
    SaveConfig(&'static str, ConfyError),
    #[error("Failed to clear {0} config: {1}")]
    ClearConfig(&'static str, ConfyError),
    #[error("Failed to read password: {0}")]
    StdinPassword(std::io::Error),
    #[error("Request failed: {0}")]
    Request(reqwest::Error),
    #[error("Error in response body: {0}")]
    ResponseBody(reqwest::Error),
    #[error("Failed to save cookies: {0}")]
    SaveCookies(Box<dyn std::error::Error>),
    #[error("Failed to clear cookies: {0}")]
    ClearCookies(Box<dyn std::error::Error>),
    #[error("Failed to create directory: {0}")]
    CreateDir(std::io::Error),
    #[error("Failed to create file: {0}")]
    CreateFile(std::io::Error),
    #[error("Failed to write file: {0}")]
    WriteFile(std::io::Error),
    #[error("Failed to read file: {0}")]
    ReadFile(std::io::Error),
    #[error("Failed to delete directory: {0}")]
    RemoveDir(std::io::Error),
    #[error("Failed to delete file: {0}")]
    RemoveFile(std::io::Error),
    #[error("Failed to run '{0:?}': {1}")]
    Command(Command, std::io::Error),
    #[error("Failed to read game manifest: {0}")]
    ReadManifest(csv::Error),
    #[error("Could not find {0}: {1}")]
    FileNotFound(PathBuf, std::io::Error),
    #[error("Task failed to exit gracefully")]
    Task(tokio::task::JoinError),
}
