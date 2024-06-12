use std::sync::Arc;

use crate::cli::Cli;
use crate::config::GalaConfig;
use crate::shared::errors::FreeCarnivalError;
use crate::{api::auth, config::InstalledConfig};
use api::GalaClient;
use clap::Parser;
use cli::Commands;
use config::{CookieConfig, LibraryConfig, UserConfig};
use constants::DEFAULT_BASE_INSTALL_PATH;
use reqwest_cookie_store::CookieStoreMutex;
use shared::models::api::{LoginResult, SyncResult};

mod api;
mod cli;
mod config;
mod constants;
mod helpers;
mod shared;
mod utils;

#[tokio::main]
async fn main() -> Result<(), FreeCarnivalError> {
    let args = Cli::parse();

    let CookieConfig(cookie_store) = CookieConfig::load()?;
    let cookie_store = Arc::new(CookieStoreMutex::new(cookie_store));
    let client = reqwest::Client::with_gala(cookie_store.clone());

    if args.needs_sync() {
        println!("Syncing library...");
        let result = api::auth::sync(&client)
            .await?
            .ok_or(FreeCarnivalError::Auth)?;
        save_user_info(&result);
    }

    match args.command {
        Commands::Login { email, password } => {
            let password = match password {
                Some(password) => password,
                None => rpassword::prompt_password("Password: ")
                    .map_err(FreeCarnivalError::StdinPassword)?,
            };

            let LoginResult { status, message } = auth::login(&client, &email, &password)
                .await?
                .ok_or(FreeCarnivalError::LoginParse)?;

            if status != "success" {
                return Err(FreeCarnivalError::Login(message));
            }

            let result = api::auth::sync(&client)
                .await?
                .ok_or(FreeCarnivalError::Auth)?;
            save_user_info(&result);
        }
        Commands::Logout => {
            UserConfig::clear()?;
            LibraryConfig::clear()?;
            cookie_store
                .lock()
                .map_err(|err| FreeCarnivalError::ClearCookies(err.to_string().into()))?
                .clear();
        }
        Commands::Library => {
            let library = LibraryConfig::load()?;
            for product in library.collection {
                println!("{}", product);
            }
        }
        Commands::Install {
            slug,
            version,
            path,
            base_path,
            os,
            install_opts,
        } => {
            let mut installed = InstalledConfig::load()?;
            if installed.contains_key(&slug) && !install_opts.info {
                return Err(FreeCarnivalError::AlreadyInstalled(slug));
            }

            let install_path = match (path, base_path) {
                (Some(path), _) => path,
                (None, Some(base_path)) => base_path.join(&slug),
                (None, None) => DEFAULT_BASE_INSTALL_PATH.join(&slug),
            };

            let library = LibraryConfig::load()?;

            // TODO: Move to function
            let selected_version = match version {
                Some(version) => {
                    let product = library
                        .collection
                        .iter()
                        .find(|p| p.slugged_name == slug)
                        .ok_or(FreeCarnivalError::GameNotFound)?;
                    let product_version = product
                        .version
                        .iter()
                        .find(|v| {
                            v.version == version
                                && match &os {
                                    Some(target) => v.os == *target,
                                    None => true,
                                }
                        })
                        .ok_or(FreeCarnivalError::InstallBuild {
                            version,
                            slug: slug.clone(),
                        })?;

                    Some(product_version)
                }
                None => None,
            };

            match utils::install(
                client.clone(),
                &slug,
                &install_path,
                install_opts,
                selected_version,
                os,
            )
            .await?
            {
                (info, Some(install_info)) => {
                    println!("{}", info);

                    installed.insert(slug, install_info);
                    installed.store()?;
                }
                (info, None) => {
                    println!("{}", info);
                }
            };
        }
        Commands::Uninstall { slug, keep } => {
            let mut installed = InstalledConfig::load().expect("Failed to load installed");
            let install_info = installed
                .remove(&slug)
                .ok_or(FreeCarnivalError::NotInstalled(slug.clone()))?;

            if !keep {
                utils::uninstall(&install_info.install_path).await?;
            }
            installed.store()?;
            println!(
                "{slug} uninstalled successfuly. {} was {}.",
                install_info.install_path.display(),
                if keep { "not removed" } else { "removed" }
            );
        }
        Commands::ListUpdates => {
            let installed = InstalledConfig::load()?;
            let library = LibraryConfig::load()?;

            let available_updates = utils::check_updates(library, installed).await;

            if available_updates.is_empty() {
                println!("No available updates");
                return Ok(());
            }

            for (slug, latest_version) in available_updates {
                println!("{slug} has an update -> {latest_version}");
            }
        }
        Commands::Update {
            slug,
            version,
            install_opts,
        } => {
            let mut installed = InstalledConfig::load()?;
            let install_info = installed
                .remove(&slug)
                .ok_or(FreeCarnivalError::NotInstalled(slug.clone()))?;
            let library = LibraryConfig::load().expect("Failed to load library");

            let selected_version = match version {
                Some(version) => {
                    let product = library
                        .collection
                        .iter()
                        .find(|p| p.slugged_name == slug)
                        .ok_or(FreeCarnivalError::GameNotFound)?;
                    let product_version = product
                        .version
                        .iter()
                        .find(|v| v.version == version)
                        .ok_or(FreeCarnivalError::InstallBuild {
                            version,
                            slug: slug.clone(),
                        })?;

                    Some(product_version)
                }
                None => None,
            };

            match utils::update(
                client.clone(),
                &library,
                &slug,
                install_opts,
                &install_info,
                selected_version,
            )
            .await?
            {
                (info, Some(install_info)) => {
                    println!("{}", info);
                    installed.insert(slug, install_info);
                    installed
                        .store()
                        .expect("Failed to update installed config");
                }
                (info, None) => {
                    println!("{}", info);
                }
            };
        }
        Commands::Launch {
            slug,
            #[cfg(not(target_os = "windows"))]
            wine,
            #[cfg(not(target_os = "windows"))]
            wine_prefix,
            #[cfg(not(target_os = "windows"))]
            no_wine,
            wrapper,
        } => {
            let installed = InstalledConfig::load()?;
            let library = LibraryConfig::load()?;
            let install_info = installed
                .get(&slug)
                .ok_or(FreeCarnivalError::NotInstalled(slug.clone()))?;
            let product = library
                .collection
                .iter()
                .find(|p| p.slugged_name == slug)
                .ok_or(FreeCarnivalError::GameNotFound)?;

            match utils::launch(
                &client,
                product,
                install_info,
                #[cfg(not(target_os = "windows"))]
                no_wine,
                #[cfg(not(target_os = "windows"))]
                wine,
                #[cfg(not(target_os = "windows"))]
                wine_prefix,
                wrapper,
            )
            .await?
            {
                Some(status) => {
                    println!("Process exited with: {}", status);
                }
                None => {
                    println!("Failed to launch {slug}");
                }
            };
        }
        Commands::Info { slug } => {
            let library = LibraryConfig::load()?;
            let product = library
                .collection
                .iter()
                .find(|p| p.slugged_name == slug)
                .ok_or(FreeCarnivalError::GameNotFound)?;

            let installed = InstalledConfig::load()?;
            let install_info = installed.get(&slug);

            println!(
                "Available Versions:\n{}",
                product
                    .version
                    .iter()
                    .map(|v| format!("\n{}", v))
                    .collect::<Vec<String>>()
                    .join("\n")
            );
        }
        Commands::Verify { slug } => {
            let installed = InstalledConfig::load()?;
            let install_info = installed
                .get(&slug)
                .ok_or(FreeCarnivalError::NotInstalled(slug.clone()))?;

            if utils::verify(&slug, install_info).await? {
                println!("{slug} passed verification.");
            } else {
                println!("{slug} is corrupted. Please reinstall.");
            }
        }
    };

    drop(client);
    let cookie_store = Arc::try_unwrap(cookie_store).unwrap();
    let cookie_store = cookie_store
        .into_inner()
        .map_err(|err| FreeCarnivalError::SaveCookies(err.into()))?;
    CookieConfig(cookie_store).store()?;

    Ok(())
}

fn save_user_info(
    SyncResult {
        user_config,
        library_config,
    }: &SyncResult,
) {
    user_config.store().expect("Failed to save user config");
    library_config
        .store()
        .expect("Failed to save library config");
}
