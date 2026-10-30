//! Transporte HTTP da sincronização na rede de casa (spec-apps.md, seções 3.3 e 3.4).
//!
//! - `GET  /sync/info`: identifica o servidor.
//! - `POST /sync/push`: o cliente envia um `ChangeSet`; resposta: `ApplyReport`.
//! - `GET  /sync/pull`: o servidor devolve um `ChangeSet` desde `X-Arca-Since`.
//!
//! Toda requisição leva `Authorization: Bearer <token>` e `X-Arca-Device: <dispositivo do cliente>`.

use serde::{Deserialize, Serialize};

pub const DEFAULT_PORT: u16 = 7377;
pub const HEADER_DEVICE: &str = "X-Arca-Device";
pub const HEADER_SINCE: &str = "X-Arca-Since";
/// Nome amigável do cliente ("Pixel 8"), só para exibição no servidor.
pub const HEADER_NAME: &str = "X-Arca-Name";

const META_DEVICE_NAME: &str = "device_name";

/// O que o QR code do servidor carrega.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pairing {
  /// Versão do formato.
  pub arca: u32,
  pub name: String,
  pub device: String,
  /// Endereços em que o servidor pode estar (um por interface de rede).
  pub urls: Vec<String>,
  pub token: String,
}

impl Pairing {
  pub fn to_json(&self) -> String {
    serde_json::to_string(self).unwrap_or_default()
  }

  pub fn from_json(text: &str) -> anyhow::Result<Self> {
    let pairing: Pairing = serde_json::from_str(text.trim())
      .map_err(|e| anyhow::anyhow!("Código de pareamento inválido: {e}"))?;
    if pairing.arca != 1 {
      anyhow::bail!("Código de pareamento de uma versão incompatível ({})", pairing.arca);
    }
    Ok(pairing)
  }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerInfo {
  pub name: String,
  pub device: String,
  pub version: String,
}

/// Nome deste computador, para identificar o servidor no celular.
pub fn host_name() -> String {
  std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_else(|_| "Arca".to_string())
}

/// Como este aparelho se apresenta ao servidor: o nome salvo (o app do celular grava o modelo)
/// ou o nome do computador.
pub fn device_name(db: &crate::persistence::Database) -> String {
  db.get_meta(META_DEVICE_NAME).ok().flatten().filter(|n| !n.is_empty()).unwrap_or_else(host_name)
}

pub fn set_device_name(db: &crate::persistence::Database, name: &str) -> anyhow::Result<()> {
  db.set_meta(META_DEVICE_NAME, name.trim())
}

#[cfg(feature = "server")]
pub mod server {
  use std::io::Read;
  use std::sync::atomic::{AtomicBool, Ordering};
  use std::sync::Arc;
  use std::time::Duration;

  use anyhow::{Context, Result};
  use tiny_http::{Header, Method, Request, Response};

  use super::*;
  use crate::dates;
  use crate::persistence::Database;
  use crate::sync::{self, ChangeSet};

  const MAX_BODY: u64 = 64 * 1024 * 1024;

  /// Adaptadores que quase nunca são a rede de casa (WSL, Hyper-V, VirtualBox, Docker, VPNs).
  fn looks_virtual(interface: &str) -> bool {
    let name = interface.to_lowercase();
    ["vethernet", "wsl", "hyper-v", "virtualbox", "vmware", "docker", "vbox", "tailscale", "zerotier", "loopback"]
      .iter()
      .any(|v| name.contains(v))
  }

  /// Endereços IPv4 privados desta máquina, para o QR code: os da rede física primeiro.
  pub fn lan_urls(port: u16) -> Vec<String> {
    let mut found: Vec<(bool, String)> = local_ip_address::list_afinet_netifas()
      .unwrap_or_default()
      .into_iter()
      .filter_map(|(interface, ip)| match ip {
        std::net::IpAddr::V4(v4) if v4.is_private() => Some((looks_virtual(&interface), format!("http://{v4}:{port}"))),
        _ => None,
      })
      .collect();
    found.sort();
    found.dedup_by(|a, b| a.1 == b.1);
    found.into_iter().map(|(_, url)| url).collect()
  }

  pub fn pairing(db: &Database, port: u16) -> Result<Pairing> {
    Ok(Pairing {
      arca: 1,
      name: host_name(),
      device: db.device_id(),
      urls: lan_urls(port),
      token: sync::server_token(db)?,
    })
  }

  /// O que aconteceu em cada requisição, para quem estiver mostrando o servidor (CLI ou app).
  #[derive(Debug, Clone)]
  pub enum Event {
    Pushed { client: String, name: Option<String>, report: sync::ApplyReport },
    Pulled { client: String, name: Option<String>, rows: usize },
    Rejected { reason: String },
  }

  /// Atende até `stop` virar `true`. `db` é uma conexão própria do servidor.
  pub fn serve(db: Database, bind: &str, port: u16, stop: Arc<AtomicBool>, mut on_event: impl FnMut(Event)) -> Result<()> {
    let server = tiny_http::Server::http((bind, port))
      .map_err(|e| anyhow::anyhow!("Não foi possível escutar em {bind}:{port}: {e}"))?;
    let token = sync::server_token(&db)?;

    while !stop.load(Ordering::Relaxed) {
      let Some(request) = server.recv_timeout(Duration::from_millis(300))? else { continue };
      let _ = db.rollover_if_needed(dates::today());
      let (response, event) = handle(&db, &token, request_parts(request));
      if let Some(event) = event {
        on_event(event);
      }
      response.send();
    }
    Ok(())
  }

  fn header(request: &Request, name: &str) -> Option<String> {
    request.headers().iter()
      .find(|h| h.field.as_str().as_str().eq_ignore_ascii_case(name))
      .map(|h| h.value.as_str().to_string())
  }

  struct Parts {
    request: Request,
    auth: Option<String>,
    device: Option<String>,
    name: Option<String>,
    since: String,
  }

  fn request_parts(request: Request) -> Parts {
    let auth = header(&request, "Authorization");
    let device = header(&request, HEADER_DEVICE);
    let name = header(&request, HEADER_NAME).filter(|n| !n.trim().is_empty());
    let since = header(&request, HEADER_SINCE).unwrap_or_default();
    Parts { request, auth, device, name, since }
  }

  struct Reply {
    request: Request,
    status: u16,
    body: String,
  }

  impl Reply {
    fn send(self) {
      let json = Header::from_bytes("Content-Type", "application/json").expect("header válido");
      let _ = self.request.respond(Response::from_string(self.body).with_status_code(self.status).with_header(json));
    }
  }

  fn error_body(message: &str) -> String {
    serde_json::json!({ "error": message }).to_string()
  }

  fn handle(db: &Database, token: &str, parts: Parts) -> (Reply, Option<Event>) {
    let Parts { mut request, auth, device, name, since } = parts;
    if auth.as_deref() != Some(format!("Bearer {token}").as_str()) {
      let reply = Reply { request, status: 401, body: error_body("token inválido; pareie de novo") };
      return (reply, Some(Event::Rejected { reason: "token inválido".into() }));
    }

    let method = request.method().clone();
    let path = request.url().split('?').next().unwrap_or("").to_string();
    let result: Result<(String, Option<Event>)> = (|| match (method, path.as_str()) {
      (Method::Get, "/sync/info") => {
        let info = ServerInfo { name: host_name(), device: db.device_id(), version: env!("CARGO_PKG_VERSION").into() };
        Ok((serde_json::to_string(&info)?, None))
      }
      (Method::Post, "/sync/push") => {
        let client = device.clone().context("falta o cabeçalho X-Arca-Device")?;
        let mut body = String::new();
        request.as_reader().take(MAX_BODY).read_to_string(&mut body)?;
        let set: ChangeSet = serde_json::from_str(&body).context("corpo do push inválido")?;
        let report = sync::apply(db, &set, false)?;
        Ok((serde_json::to_string(&report)?, Some(Event::Pushed { client, name: name.clone(), report })))
      }
      (Method::Get, "/sync/pull") => {
        let client = device.clone().context("falta o cabeçalho X-Arca-Device")?;
        let set = sync::changes_since(db, &since, Some(&client), true)?;
        let rows = set.projects.len() + set.tasks.len() + set.history.len() + set.suggestions.len() + set.tombstones.len();
        Ok((serde_json::to_string(&set)?, Some(Event::Pulled { client, name: name.clone(), rows })))
      }
      _ => anyhow::bail!("rota desconhecida: {path}"),
    })();

    match result {
      Ok((body, event)) => (Reply { request, status: 200, body }, event),
      Err(error) => {
        let message = format!("{error:#}");
        (Reply { request, status: 400, body: error_body(&message) }, Some(Event::Rejected { reason: message }))
      }
    }
  }
}

#[cfg(feature = "client")]
pub mod client {
  use std::time::Duration;

  use anyhow::{bail, Context, Result};
  use serde::de::DeserializeOwned;

  use super::*;
  use crate::persistence::Database;
  use crate::sync::{self, ApplyReport, ChangeSet, Peer};

  const MAX_BODY: u64 = 64 * 1024 * 1024;

  /// Cabeçalhos HTTP só aceitam ASCII visível.
  fn header_safe(name: &str) -> String {
    name.chars().map(|c| if c.is_ascii_graphic() || c == ' ' { c } else { '?' }).take(64).collect()
  }

  fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
      .timeout_global(Some(Duration::from_secs(20)))
      // Um endereço antigo do QR (outra rede, adaptador virtual) não pode travar a tentativa.
      .timeout_connect(Some(Duration::from_secs(3)))
      .http_status_as_error(false)
      .build()
      .into()
  }

  fn read<T: DeserializeOwned>(mut response: ureq::http::Response<ureq::Body>) -> Result<T> {
    let status = response.status().as_u16();
    if status != 200 {
      let body: serde_json::Value = response.body_mut().read_json().unwrap_or_default();
      let message = body.get("error").and_then(|e| e.as_str()).unwrap_or("erro desconhecido").to_string();
      bail!("servidor respondeu {status}: {message}");
    }
    Ok(response.body_mut().with_config().limit(MAX_BODY).read_json()?)
  }

  pub fn fetch_info(url: &str, token: &str) -> Result<ServerInfo> {
    let response = agent().get(format!("{url}/sync/info"))
      .header("Authorization", format!("Bearer {token}"))
      .call()
      .with_context(|| format!("sem resposta de {url}"))?;
    read(response)
  }

  /// Salva o servidor do QR code, usando o primeiro endereço que responder.
  pub fn pair(db: &Database, pairing: &Pairing) -> Result<Peer> {
    let mut errors = Vec::new();
    for url in &pairing.urls {
      match fetch_info(url, &pairing.token) {
        Ok(info) if info.device == pairing.device => {
          // O endereço que funcionou fica primeiro; os outros servem de reserva se o IP mudar.
          let mut urls = vec![url.clone()];
          urls.extend(pairing.urls.iter().filter(|u| *u != url).cloned());
          sync::save_peer(db, &pairing.device, &pairing.name, &urls.join(" "), &pairing.token)?;
          return sync::peers(db)?.into_iter().find(|p| p.device == pairing.device)
            .context("servidor pareado sumiu");
        }
        Ok(_) => errors.push(format!("{url}: outro servidor")),
        Err(error) => errors.push(format!("{url}: {error:#}")),
      }
    }
    bail!("Não consegui falar com o servidor ({}). Os dois aparelhos estão na mesma rede?", errors.join("; "))
  }

  #[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
  pub struct SyncReport {
    /// O que o servidor aplicou do que este dispositivo enviou.
    pub pushed: ApplyReport,
    /// O que este dispositivo aplicou do que recebeu.
    pub pulled: ApplyReport,
  }

  fn push_pull(db: &Database, peer: &Peer, url: &str) -> Result<SyncReport> {
    let device = db.device_id();
    let name = header_safe(&device_name(db));
    let auth = format!("Bearer {}", peer.token);

    let outgoing = sync::changes_since(db, &peer.last_pushed, Some(&peer.device), false)?;
    let pushed: ApplyReport = read(agent().post(format!("{url}/sync/push"))
      .header("Authorization", &auth)
      .header(HEADER_DEVICE, &device)
      .header(HEADER_NAME, &name)
      .send_json(&outgoing)?)?;

    let incoming: ChangeSet = read(agent().get(format!("{url}/sync/pull"))
      .header("Authorization", &auth)
      .header(HEADER_DEVICE, &device)
      .header(HEADER_NAME, &name)
      .header(HEADER_SINCE, &peer.last_pulled)
      .call()?)?;
    if incoming.device != peer.device {
      bail!("{url} agora é outro servidor; pareie de novo");
    }
    let pulled = sync::apply(db, &incoming, true)?;

    sync::update_peer_cursors(db, &peer.device, &incoming.cursor, &outgoing.cursor)?;
    Ok(SyncReport { pushed, pulled })
  }

  /// Sincroniza com um servidor pareado, tentando cada endereço conhecido.
  pub fn sync_with(db: &Database, peer: &Peer) -> Result<SyncReport> {
    let urls: Vec<&str> = peer.url.split_whitespace().collect();
    let mut errors = Vec::new();
    for (i, url) in urls.iter().enumerate() {
      match push_pull(db, peer, url) {
        Ok(report) => {
          if i > 0 {
            let mut reordered = vec![url.to_string()];
            reordered.extend(urls.iter().filter(|u| *u != url).map(|u| u.to_string()));
            sync::save_peer(db, &peer.device, &peer.name, &reordered.join(" "), &peer.token)?;
          }
          return Ok(report);
        }
        Err(error) => errors.push(format!("{url}: {error:#}")),
      }
    }
    bail!("Não consegui sincronizar com {} ({})", peer.name, errors.join("; "))
  }
}

#[cfg(all(test, feature = "server", feature = "client"))]
mod tests {
  use std::sync::atomic::{AtomicBool, Ordering};
  use std::sync::Arc;

  use super::*;
  use crate::persistence::{Database, NewTask, Status, USER};

  #[test]
  fn client_and_server_sync_over_http() {
    let dir = std::env::temp_dir().join(format!("arca-net-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let pc_path = dir.join("pc.db");
    let _ = std::fs::remove_file(&pc_path);
    let port = 40000 + (std::process::id() % 20000) as u16;

    let pc = Database::init(&pc_path).unwrap();
    pc.add_task(NewTask { title: "Do PC", daily: false, status: Status::Cart, due_date: None, project_id: None, created_by: USER }).unwrap();
    let mut pairing = server::pairing(&pc, port).unwrap();
    pairing.urls = vec![format!("http://127.0.0.1:{port}")];

    let stop = Arc::new(AtomicBool::new(false));
    let server_stop = Arc::clone(&stop);
    let handle = std::thread::spawn(move || server::serve(pc, "127.0.0.1", port, server_stop, |_| {}));
    std::thread::sleep(std::time::Duration::from_millis(200));

    let phone = Database::open_in_memory().unwrap();
    let peer = client::pair(&phone, &pairing).unwrap();
    let report = client::sync_with(&phone, &peer).unwrap();
    assert_eq!(report.pulled.applied, 1);
    assert_eq!(phone.get_tasks(Default::default()).unwrap()[0].title, "Do PC");

    let mut wrong = pairing.clone();
    wrong.token = "errado".into();
    assert!(client::pair(&Database::open_in_memory().unwrap(), &wrong).is_err());

    stop.store(true, Ordering::Relaxed);
    handle.join().unwrap().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
  }

  #[test]
  fn pairing_round_trips() {
    let pairing = Pairing { arca: 1, name: "PC".into(), device: "d".into(), urls: vec!["http://x:1".into()], token: "t".into() };
    assert_eq!(Pairing::from_json(&pairing.to_json()).unwrap(), pairing);
    assert!(Pairing::from_json("{}").is_err());
  }
}
