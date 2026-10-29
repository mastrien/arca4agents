use anyhow::{Context, Result};
use directories::ProjectDirs;
use std::fs;
use std::path::PathBuf;

pub fn get_db_path() -> Result<PathBuf> {
  // ARCA_DB aponta para outro banco (testes, ou um banco separado por ambiente).
  if let Some(custom) = std::env::var_os("ARCA_DB").filter(|v| !v.is_empty()) {
    let db_path = PathBuf::from(custom);
    if let Some(parent) = db_path.parent().filter(|p| !p.as_os_str().is_empty()) {
      fs::create_dir_all(parent)
        .context("Falha ao criar o diretório do banco indicado em ARCA_DB")?;
    }
    return Ok(db_path);
  }

  // ProjectDirs::from("qualificador", "organização", "nome_do_app")
  // ProjectDirs::from("com", "mastrien", "arca")
  let proj_dirs = ProjectDirs::from("com", "mastrien", "arca")
    .context("Não foi possível determinar o diretório de dados do sistema")?;

  let data_dir = proj_dirs.data_dir();

  fs::create_dir_all(data_dir)
    .context("Falha ao criar o diretório de dados da aplicação")?;

  let db_path = data_dir.join("arca.db");

  Ok(db_path)
}