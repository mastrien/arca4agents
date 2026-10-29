use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Normaliza um caminho para comparar pastas vinculadas a projetos:
/// absoluto, com `/` como separador, sem `.`/`..` e sem barra final.
/// No Windows também aceita o formato do Git Bash (`/c/pasta`) e ignora maiúsculas,
/// pois o sistema de arquivos não as diferencia.
pub fn normalize(input: &str) -> Result<String> {
  let windows = cfg!(windows);

  let raw = if windows { msys_to_windows(input) } else { input.to_string() };
  let mut absolute: PathBuf = std::path::absolute(Path::new(&raw))
    .with_context(|| format!("Não foi possível resolver o caminho '{input}'"))?;

  // No Android/Termux, /sdcard e /storage/emulated/0 são o mesmo lugar via symlink.
  if !windows
    && let Ok(canonical) = absolute.canonicalize() {
      absolute = canonical;
    }

  Ok(normalize_absolute(&absolute.to_string_lossy(), windows))
}

/// Parte pura (testável em qualquer SO) da normalização de um caminho já absoluto.
fn normalize_absolute(path: &str, windows: bool) -> String {
  let mut s = path.to_string();
  if windows {
    s = s.replace('\\', "/");
    if let Some(rest) = s.strip_prefix("//?/") {
      s = rest.to_string();
    }
    s = s.to_lowercase();
  }

  let (prefix, rest) = if let Some(rest) = s.strip_prefix("//") {
    ("//".to_string(), rest.to_string())
  } else if windows && s.len() >= 2 && s.as_bytes()[1] == b':' {
    (format!("{}/", &s[..2]), s[2..].to_string())
  } else if let Some(rest) = s.strip_prefix('/') {
    ("/".to_string(), rest.to_string())
  } else {
    (String::new(), s.clone())
  };

  let mut parts: Vec<&str> = Vec::new();
  for component in rest.split('/') {
    match component {
      "" | "." => {}
      ".." => { parts.pop(); }
      c => parts.push(c),
    }
  }

  format!("{}{}", prefix, parts.join("/"))
}

/// `/c/Users/x` → `C:/Users/x` (formato do Git Bash/MSYS).
fn msys_to_windows(input: &str) -> String {
  let bytes = input.as_bytes();
  let is_msys_drive = bytes.len() >= 2
    && bytes[0] == b'/'
    && bytes[1].is_ascii_alphabetic()
    && (bytes.len() == 2 || bytes[2] == b'/');

  if is_msys_drive {
    format!("{}:/{}", bytes[1] as char, input.get(3..).unwrap_or(""))
  } else {
    input.to_string()
  }
}

/// `cwd` está dentro de `folder` (ou é a própria pasta)? Ambos já normalizados.
pub fn is_within(folder: &str, cwd: &str) -> bool {
  if cwd == folder {
    return true;
  }
  if folder.ends_with('/') {
    return cwd.starts_with(folder);
  }
  cwd.len() > folder.len() && cwd.starts_with(folder) && cwd.as_bytes()[folder.len()] == b'/'
}

/// Escolhe a pasta vinculada mais específica (prefixo mais longo) que contém `cwd`.
pub fn most_specific<'a, T>(cwd: &str, folders: &'a [(String, T)]) -> Option<&'a T> {
  folders.iter()
    .filter(|(folder, _)| is_within(folder, cwd))
    .max_by_key(|(folder, _)| folder.len())
    .map(|(_, value)| value)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn normalizes_windows_paths() {
    assert_eq!(normalize_absolute(r"C:\Root_Lab\Arca\", true), "c:/root_lab/arca");
    assert_eq!(normalize_absolute(r"C:\root_lab\.\x\..\arca", true), "c:/root_lab/arca");
    assert_eq!(normalize_absolute(r"\\?\C:\root_lab", true), "c:/root_lab");
    assert_eq!(normalize_absolute(r"C:\", true), "c:/");
    assert_eq!(normalize_absolute(r"\\server\share\dir", true), "//server/share/dir");
  }

  #[test]
  fn normalizes_unix_paths() {
    assert_eq!(normalize_absolute("/data/data/com.termux/files/home/proj/", false), "/data/data/com.termux/files/home/proj");
    assert_eq!(normalize_absolute("/home/User/a/../b", false), "/home/User/b");
    assert_eq!(normalize_absolute("/", false), "/");
  }

  #[test]
  fn converts_msys_drive() {
    assert_eq!(msys_to_windows("/c/root_lab"), "c:/root_lab");
    assert_eq!(msys_to_windows("/c"), "c:/");
    assert_eq!(msys_to_windows("/cdrom/x"), "/cdrom/x");
  }

  #[test]
  fn matches_folders() {
    assert!(is_within("c:/root_lab/arca", "c:/root_lab/arca"));
    assert!(is_within("c:/root_lab/arca", "c:/root_lab/arca/src"));
    assert!(!is_within("c:/root_lab/arca", "c:/root_lab/arca4agents"));
    assert!(is_within("c:/", "c:/anything"));
    assert!(is_within("/", "/home"));
  }

  #[test]
  fn picks_longest_prefix() {
    let folders = vec![
      ("c:/root_lab".to_string(), 1),
      ("c:/root_lab/arca4agents".to_string(), 2),
      ("c:/root_lab/arca".to_string(), 3),
    ];
    assert_eq!(most_specific("c:/root_lab/arca4agents/src", &folders), Some(&2));
    assert_eq!(most_specific("c:/root_lab/arca/src", &folders), Some(&3));
    assert_eq!(most_specific("c:/root_lab/other", &folders), Some(&1));
    assert_eq!(most_specific("d:/x", &folders), None);
  }
}
