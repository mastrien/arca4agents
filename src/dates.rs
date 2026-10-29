use anyhow::{bail, Result};
use chrono::{Duration, Local, NaiveDate, NaiveDateTime};

pub const DATE_FORMAT: &str = "%Y-%m-%d";
pub const DATETIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

pub fn today() -> NaiveDate {
  Local::now().date_naive()
}

pub fn now() -> NaiveDateTime {
  Local::now().naive_local()
}

/// Aceita `AAAA-MM-DD`, `hoje`, `amanhã` e deslocamentos curtos como `+3d` ou `+2w`.
pub fn parse_due(input: &str, today: NaiveDate) -> Result<NaiveDate> {
  let s = input.trim().to_lowercase();

  match s.as_str() {
    "hoje" | "today" => return Ok(today),
    "amanhã" | "amanha" | "tomorrow" => return Ok(today + Duration::days(1)),
    _ => {}
  }

  if let Some(offset) = s.strip_prefix('+') {
    let (number, unit) = offset.split_at(offset.len().saturating_sub(1));
    let days_per_unit = match unit {
      "d" => 1,
      "w" | "s" => 7,
      _ => bail!("Prazo inválido: '{input}'. Use +Nd ou +Nw (ex.: +3d)"),
    };
    let n: i64 = number.parse()
      .map_err(|_| anyhow::anyhow!("Prazo inválido: '{input}'. Use +Nd ou +Nw (ex.: +3d)"))?;
    return Ok(today + Duration::days(n * days_per_unit));
  }

  NaiveDate::parse_from_str(&s, DATE_FORMAT)
    .map_err(|_| anyhow::anyhow!("Prazo inválido: '{input}'. Use AAAA-MM-DD, hoje, amanhã ou +Nd"))
}

/// "hoje", "amanhã", "em 3 dias", "passou há 2 dias".
pub fn humanize_due(due: NaiveDate, today: NaiveDate) -> String {
  let days = (due - today).num_days();
  match days {
    0 => "hoje".to_string(),
    1 => "amanhã".to_string(),
    d if d > 1 => format!("em {d} dias"),
    -1 => "passou há 1 dia".to_string(),
    d => format!("passou há {} dias", -d),
  }
}

/// "agora há pouco", "há 5 min", "há 5h", "há 3 dias".
pub fn humanize_ago(then: NaiveDateTime, now: NaiveDateTime) -> String {
  let minutes = (now - then).num_minutes().max(0);
  if minutes < 1 {
    "agora há pouco".to_string()
  } else if minutes < 60 {
    format!("há {minutes} min")
  } else if minutes < 60 * 24 {
    format!("há {}h", minutes / 60)
  } else {
    let days = minutes / (60 * 24);
    if days == 1 { "há 1 dia".to_string() } else { format!("há {days} dias") }
  }
}

pub fn parse_datetime(s: &str) -> Option<NaiveDateTime> {
  NaiveDateTime::parse_from_str(s, DATETIME_FORMAT).ok()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn d(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, DATE_FORMAT).unwrap()
  }

  #[test]
  fn parses_due_formats() {
    let today = d("2026-10-29");
    assert_eq!(parse_due("2026-11-05", today).unwrap(), d("2026-11-05"));
    assert_eq!(parse_due("hoje", today).unwrap(), today);
    assert_eq!(parse_due("Amanhã", today).unwrap(), d("2026-10-30"));
    assert_eq!(parse_due("+3d", today).unwrap(), d("2026-11-01"));
    assert_eq!(parse_due("+2w", today).unwrap(), d("2026-11-12"));
    assert!(parse_due("+xd", today).is_err());
    assert!(parse_due("+3", today).is_err());
    assert!(parse_due("ontem", today).is_err());
    assert!(parse_due("2026-13-01", today).is_err());
  }

  #[test]
  fn humanizes_due() {
    let today = d("2026-10-29");
    assert_eq!(humanize_due(d("2026-10-29"), today), "hoje");
    assert_eq!(humanize_due(d("2026-10-30"), today), "amanhã");
    assert_eq!(humanize_due(d("2026-11-01"), today), "em 3 dias");
    assert_eq!(humanize_due(d("2026-10-28"), today), "passou há 1 dia");
    assert_eq!(humanize_due(d("2026-10-27"), today), "passou há 2 dias");
  }

  #[test]
  fn humanizes_ago() {
    let now = parse_datetime("2026-10-29 12:00:00").unwrap();
    assert_eq!(humanize_ago(parse_datetime("2026-10-29 11:59:30").unwrap(), now), "agora há pouco");
    assert_eq!(humanize_ago(parse_datetime("2026-10-29 11:55:00").unwrap(), now), "há 5 min");
    assert_eq!(humanize_ago(parse_datetime("2026-10-29 07:00:00").unwrap(), now), "há 5h");
    assert_eq!(humanize_ago(parse_datetime("2026-10-26 12:00:00").unwrap(), now), "há 3 dias");
  }
}
