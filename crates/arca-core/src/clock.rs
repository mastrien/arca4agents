//! Relógio lógico híbrido (HLC) para a sincronização.
//!
//! Formato: `<ms:013>:<contador:05>:<dispositivo>`. Larguras fixas tornam a comparação de
//! texto igual à ordem temporal, e o dispositivo desempata. O relógio nunca anda para trás:
//! nem se o relógio do sistema voltar, nem depois de ver um relógio maior vindo de outro aparelho.

use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct Hlc {
  last_ms: u64,
  counter: u32,
  device: String,
}

fn now_ms() -> u64 {
  SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

pub fn format(ms: u64, counter: u32, device: &str) -> String {
  format!("{ms:013}:{counter:05}:{device}")
}

/// `(ms, contador, dispositivo)` de um relógio, se estiver no formato esperado.
pub fn parse(clock: &str) -> Option<(u64, u32, &str)> {
  let mut parts = clock.splitn(3, ':');
  let ms = parts.next()?.parse().ok()?;
  let counter = parts.next()?.parse().ok()?;
  let device = parts.next()?;
  Some((ms, counter, device))
}

/// Dispositivo que fez a escrita carimbada com este relógio.
pub fn device_of(clock: &str) -> Option<&str> {
  parse(clock).map(|(_, _, device)| device)
}

impl Hlc {
  pub fn new(device: impl Into<String>) -> Self {
    Self { last_ms: 0, counter: 0, device: device.into() }
  }

  pub fn device(&self) -> &str {
    &self.device
  }

  pub fn set_device(&mut self, device: impl Into<String>) {
    self.device = device.into();
  }

  /// Próximo relógio para uma escrita local.
  pub fn tick(&mut self) -> String {
    self.tick_at(now_ms())
  }

  fn tick_at(&mut self, now: u64) -> String {
    if now > self.last_ms {
      self.last_ms = now;
      self.counter = 0;
    } else {
      self.counter += 1;
    }
    format(self.last_ms, self.counter, &self.device)
  }

  /// Registra um relógio visto (local, salvo no banco, ou remoto) para nunca gerar um menor.
  pub fn observe(&mut self, clock: &str) {
    if let Some((ms, counter, _)) = parse(clock)
      && (ms, counter) > (self.last_ms, self.counter) {
        self.last_ms = ms;
        self.counter = counter;
      }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn ticks_are_monotonic_and_sortable() {
    let mut hlc = Hlc::new("a");
    let first = hlc.tick_at(1000);
    let same_ms = hlc.tick_at(1000);
    let clock_went_back = hlc.tick_at(500);
    let later = hlc.tick_at(2000);
    assert!(first < same_ms && same_ms < clock_went_back && clock_went_back < later, "{first} {same_ms} {clock_went_back} {later}");
    assert_eq!(first, "0000000001000:00000:a");
  }

  #[test]
  fn observing_a_remote_clock_moves_forward() {
    let mut hlc = Hlc::new("a");
    hlc.tick_at(1000);
    let remote = format(5000, 3, "b");
    hlc.observe(&remote);
    let next = hlc.tick_at(1200);
    assert!(next > remote, "{next} > {remote}");
    assert_eq!(device_of(&next), Some("a"));
  }

  #[test]
  fn parses_clocks() {
    assert_eq!(parse("0000000001000:00002:dev-1"), Some((1000, 2, "dev-1")));
    assert_eq!(parse("lixo"), None);
  }
}
