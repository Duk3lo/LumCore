use crate::lum::config::github_config::RepositoryResource;
use crate::lum::config::paths;
use anyhow::{anyhow, Result};
use chrono::Local;
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
pub struct GhRelease {
    pub tag_name: String,
    pub assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
pub struct GhAsset {
    pub name: String,
    pub browser_download_url: String,
}

pub struct GitHubClient {
    client: Client,
    global_token: String,
}

impl GitHubClient {
    pub fn new(global_token: &str) -> Self {
        Self {
            client: Client::new(),
            global_token: global_token.to_string(),
        }
    }

    fn get_latest_release(&self, repo_slug: &str, custom_token: &str) -> Result<GhRelease> {
        let url = format!("https://api.github.com/repos/{}/releases/latest", repo_slug);
        
        let token = if !custom_token.is_empty() { custom_token } else { &self.global_token };
        let mut req = self.client.get(&url)
            .header(USER_AGENT, "LumCore-Rust")
            .header(ACCEPT, "application/vnd.github.v3+json");

        if !token.is_empty() {
            req = req.header(AUTHORIZATION, format!("Bearer {}", token));
        }

        let res = req.send()?.error_for_status()?;
        Ok(res.json()?)
    }

    pub fn download_and_replace(&self, resource: &mut RepositoryResource, mod_key: &str) -> Result<bool> {
        let release = self.get_latest_release(&resource.repo_slug, &resource.custom_token)?;
        
        let workspace = paths::workspace_dir().map_err(|e| anyhow!(e))?;
        let dest_dir = paths::resolve(&workspace, &resource.destination_path)
            .unwrap_or_else(|| PathBuf::from(&resource.destination_path));

        let local_file = resource.local_file_name.as_ref().map(|n| dest_dir.join(n));
        let file_exists = local_file.as_ref().map_or(false, |p| p.exists());

        if release.tag_name == resource.local_version_tag && file_exists {
            if resource.verify_file_integrity && !resource.last_verified_hash.is_empty() {
                let current_hash = self.calculate_hash(local_file.as_ref().unwrap())?;
                if current_hash == resource.last_verified_hash {
                    println!("[GitHub] El archivo local está íntegro y al día.");
                    return Ok(false);
                }
                println!("[GitHub Warning] El hash no coincide. Re-descargando...");
            } else {
                println!("[GitHub] El mod {} ya está en su última versión ({}).", resource.repo_slug, release.tag_name);
                return Ok(false);
            }
        }

        let target_asset = release.assets.into_iter()
            .find(|a| a.name.ends_with(".jar") || a.name.ends_with(".zip"))
            .ok_or_else(|| anyhow!("No se encontró archivo .jar o .zip en la versión {}", release.tag_name))?;

        println!("[GitHub] Descargando actualización: {}...", target_asset.name);

        let gh_dir = workspace.join("github");
        let downloads_dir = gh_dir.join("downloads");
        fs::create_dir_all(&downloads_dir)?;
        
        let temp_file_path = downloads_dir.join(&target_asset.name);

        let mut req = self.client.get(&target_asset.browser_download_url).header(USER_AGENT, "LumCore-Rust");
        let token = if !resource.custom_token.is_empty() { &resource.custom_token } else { &self.global_token };
        if !token.is_empty() { req = req.header(AUTHORIZATION, format!("Bearer {}", token)); }

        let mut response = req.send()?.error_for_status()?;
        let mut file = File::create(&temp_file_path)?;
        io::copy(&mut response, &mut file)?;

        let new_hash = if resource.verify_file_integrity {
            self.calculate_hash(&temp_file_path)?
        } else {
            String::new()
        };

        fs::create_dir_all(&dest_dir)?;
        if resource.keep_backup {
            if let Some(old_name) = &resource.local_file_name {
                let old_path = dest_dir.join(old_name);
                if old_path.exists() {
                    let backup_dir = gh_dir.join("backups").join(mod_key);
                    fs::create_dir_all(&backup_dir)?;
                    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
                    fs::copy(&old_path, backup_dir.join(format!("{}.backup_{}", old_name, timestamp)))?;
                }
            }
        }

        if let Some(old_name) = &resource.local_file_name {
            let _ = fs::remove_file(dest_dir.join(old_name));
        }

        fs::rename(&temp_file_path, dest_dir.join(&target_asset.name))?;

        resource.local_version_tag = release.tag_name;
        resource.local_file_name = Some(target_asset.name);
        resource.last_verified_hash = new_hash;

        println!("[GitHub] ✅ '{}' actualizado y movido a syncmods.", mod_key);
        Ok(true)
    }

    pub fn restore_latest_backup(&self, resource: &mut RepositoryResource, mod_key: &str) -> Result<()> {
        let workspace = paths::workspace_dir().map_err(|e| anyhow!(e))?;
        let backup_dir = workspace.join("github").join("backups").join(mod_key);
        
        if !backup_dir.exists() { return Err(anyhow!("No hay backups para '{}'", mod_key)); }

        let mut latest_file = None;
        let mut latest_time = std::time::UNIX_EPOCH;

        for entry in fs::read_dir(&backup_dir)? {
            let entry = entry?;
            let mod_time = entry.metadata()?.modified()?;
            if mod_time > latest_time {
                latest_time = mod_time;
                latest_file = Some(entry.path());
            }
        }

        if let Some(backup_path) = latest_file {
            let file_name = backup_path.file_name().unwrap().to_string_lossy().to_string();
            let original_name = if let Some(idx) = file_name.find(".backup_") { &file_name[..idx] } else { &file_name };

            let dest_dir = paths::resolve(&workspace, &resource.destination_path)
                .unwrap_or_else(|| PathBuf::from(&resource.destination_path));

            if let Some(curr) = &resource.local_file_name { let _ = fs::remove_file(dest_dir.join(curr)); }

            fs::copy(&backup_path, dest_dir.join(original_name))?;
            resource.local_file_name = Some(original_name.to_string());
            resource.local_version_tag = String::new();
            
            println!("[GitHub] ✅ Restaurado exitosamente: {}", original_name);
            Ok(())
        } else {
            Err(anyhow!("La carpeta de backups está vacía."))
        }
    }

    fn calculate_hash(&self, file_path: &Path) -> Result<String> {
        let mut file = File::open(file_path)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0; 8192];
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 { break; }
            hasher.update(&buffer[..count]);
        }
        Ok(format!("{:x}", hasher.finalize()))
    }
}