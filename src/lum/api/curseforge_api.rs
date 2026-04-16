use crate::lum::config::curseforge_config::CurseForgeResource;
use anyhow::{anyhow, Result};
use chrono::Local;
use md5::{Digest as Md5Digest, Md5};
use reqwest::blocking::Client;
use serde::Deserialize;
use sha1::Sha1;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct CfResponse<T> {
    data: T,
}

#[derive(Deserialize)]
pub struct CfMod {
    pub id: u32,
    pub name: String,
    #[serde(rename = "mainFileId")]
    pub main_file_id: u32,
}

#[derive(Deserialize)]
pub struct CfFile {
    pub id: u32,
    #[serde(rename = "fileName")]
    pub file_name: String,
    #[serde(rename = "downloadUrl")]
    pub download_url: Option<String>,
    pub hashes: Vec<CfHash>,
}

#[derive(Deserialize)]
pub struct CfHash {
    pub value: String,
    pub algo: u8,
}

pub struct CurseForgeClient {
    client: Client,
    api_key: String,
}

impl CurseForgeClient {
    pub fn new(api_key: &str) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.to_string(),
        }
    }

    fn get<T: serde::de::DeserializeOwned>(&self, endpoint: &str) -> Result<T> {
        let url = format!("https://api.curseforge.com/v1/{}", endpoint);

        let res = self
            .client
            .get(&url)
            .header("x-api-key", &self.api_key)
            .header("Accept", "application/json")
            .send()?
            .error_for_status()?;

        Ok(res.json()?)
    }

    pub fn get_mod_info(&self, project_id: u32) -> Result<CfMod> {
        let res: CfResponse<CfMod> = self.get(&format!("mods/{}", project_id))?;
        Ok(res.data)
    }

    pub fn get_file_info(&self, project_id: u32, file_id: u32) -> Result<CfFile> {
        let res: CfResponse<CfFile> = self.get(&format!("mods/{}/files/{}", project_id, file_id))?;
        Ok(res.data)
    }

    pub fn search_mod(&self, game_id: u32, query: &str) -> Result<Vec<CfMod>> {
        let endpoint = format!(
            "mods/search?gameId={}&searchFilter={}",
            game_id,
            urlencoding::encode(query)
        );
        let res: CfResponse<Vec<CfMod>> = self.get(&endpoint)?;
        Ok(res.data)
    }

    pub fn download_and_replace(
        &self,
        resource: &mut CurseForgeResource,
        mod_key: &str,
    ) -> Result<bool> {
        let mod_info = self.get_mod_info(resource.project_id)?;
        let file_info = self.get_file_info(resource.project_id, mod_info.main_file_id)?;

        let dest_dir = PathBuf::from(&resource.destination_path);
        let local_file = resource.local_file_name.as_ref().map(|n| dest_dir.join(n));

        if resource.local_file_id == file_info.id {
            if let Some(path) = &local_file {
                if path.exists() {
                    println!("[CurseForge] '{}' ya está en su última versión.", mod_key);
                    return Ok(false);
                }
            }
        }

        let download_url = file_info
            .download_url
            .ok_or_else(|| anyhow!("Distribución deshabilitada por el autor para {}", file_info.file_name))?;

        println!("[CurseForge] Descargando actualización: {}...", file_info.file_name);

        let downloads_dir = PathBuf::from("downloads");
        fs::create_dir_all(&downloads_dir)?;
        let temp_file_path = downloads_dir.join(&file_info.file_name);

        let mut response = self.client.get(&download_url).send()?.error_for_status()?;
        let mut file = File::create(&temp_file_path)?;
        io::copy(&mut response, &mut file)?;

        if resource.verify_file_integrity {
            self.verify_hash(&temp_file_path, &file_info.hashes)?;
        }

        fs::create_dir_all(&dest_dir)?;

        if resource.keep_backup {
            if let Some(old_name) = &resource.local_file_name {
                let old_path = dest_dir.join(old_name);
                if old_path.exists() {
                    let backup_dir = PathBuf::from("backups").join(mod_key);
                    fs::create_dir_all(&backup_dir)?;
                    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
                    let backup_path = backup_dir.join(format!("{}.backup_{}", old_name, timestamp));
                    fs::copy(&old_path, &backup_path)?;
                    println!("[CurseForge] Backup guardado en {:?}", backup_path);
                }
            }
        }

        if let Some(old_name) = &resource.local_file_name {
            let _ = fs::remove_file(dest_dir.join(old_name));
        }

        let final_path = dest_dir.join(&file_info.file_name);
        fs::rename(&temp_file_path, &final_path)?;

        resource.local_file_id = file_info.id;
        resource.local_file_name = Some(file_info.file_name.clone());

        println!("[CurseForge] ✅ '{}' actualizado correctamente.", mod_key);
        Ok(true)
    }

    pub fn restore_latest_backup(&self, resource: &mut CurseForgeResource, mod_key: &str) -> Result<()> {
        let backup_dir = PathBuf::from("backups").join(mod_key);
        
        if !backup_dir.exists() {
            return Err(anyhow!("No hay carpeta de backups para '{}'", mod_key));
        }

        let mut latest_file = None;
        let mut latest_time = std::time::UNIX_EPOCH;

        // Buscamos el archivo modificado más recientemente
        for entry in fs::read_dir(&backup_dir)? {
            let entry = entry?;
            let meta = entry.metadata()?;
            if meta.is_file() {
                let mod_time = meta.modified()?;
                if mod_time > latest_time {
                    latest_time = mod_time;
                    latest_file = Some(entry.path());
                }
            }
        }

        if let Some(backup_path) = latest_file {
            let file_name = backup_path.file_name().unwrap().to_string_lossy().to_string();
            
            // Extraemos el nombre original quitando ".backup_YYYYMMDD_HHMMSS"
            let original_name = if let Some(idx) = file_name.find(".backup_") {
                &file_name[..idx]
            } else {
                &file_name
            };

            let dest_dir = PathBuf::from(&resource.destination_path);
            fs::create_dir_all(&dest_dir)?;

            // Borramos el mod actual si existe
            if let Some(curr) = &resource.local_file_name {
                let _ = fs::remove_file(dest_dir.join(curr));
            }

            // Restauramos
            fs::copy(&backup_path, dest_dir.join(original_name))?;
            resource.local_file_name = Some(original_name.to_string());
            resource.local_file_id = 0; // Reseteamos ID para que actualice en el futuro

            println!("[CurseForge] ✅ Mod restaurado exitosamente desde backup: {}", original_name);
            Ok(())
        } else {
            Err(anyhow!("La carpeta de backups para '{}' está vacía.", mod_key))
        }
    }

    fn verify_hash(&self, file_path: &Path, hashes: &[CfHash]) -> Result<()> {
        let mut file = File::open(file_path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        for hash in hashes {
            match hash.algo {
                1 => {
                    let digest = format!("{:x}", Sha1::digest(&buffer));
                    if digest == hash.value { return Ok(()); }
                }
                2 => {
                    let digest = format!("{:x}", Md5::digest(&buffer));
                    if digest == hash.value { return Ok(()); }
                }
                _ => continue,
            }
        }

        Err(anyhow!("Verificación de Hash fallida. El archivo descargado está corrupto."))
    }
}