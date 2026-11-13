use anyhow::{bail, Result};
use chrono::{Datelike, Duration, Local, NaiveDate, NaiveDateTime};

pub const DATE_FORMAT: &str = "%Y-%m-%d";
pub const DATETIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

pub fn today() -> NaiveDate {
  Local::now().date_naive()
}

pub fn now() -> NaiveDateTime {
  Local::now().naive_local()
}

/// Aceita `AAAA-MM-DD`, `DD/MM/AAAA`, `hoje`, `amanhã` e deslocamentos curtos como `+3d` ou `+2w`.
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

  if let Ok(date) = NaiveDate::parse_from_str(&s, DATE_FORMAT) {
    return Ok(date);
  }

  NaiveDate::parse_from_str(&s, "%d/%m/%Y")
    .map_err(|_| anyhow::anyhow!("Prazo inválido: '{input}'. Use AAAA-MM-DD, DD/MM/AAAA, hoje, amanhã ou +Nd"))
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

pub fn validate_birthday(month: u32, day: u32, year: Option<i32>, today: NaiveDate) -> Result<()> {
  if !(1..=12).contains(&month) || NaiveDate::from_ymd_opt(2024, month, day).is_none() {
    bail!("Data de aniversário inválida: {day:02}/{month:02}");
  }
  if let Some(y) = year {
    if y < 1900 || y > today.year() {
      bail!("Ano de nascimento inválido: {y} (esperado entre 1900 e {})", today.year());
    }
    if NaiveDate::from_ymd_opt(y, month, day).is_none() {
      bail!("Data de aniversário inválida: {day:02}/{month:02}/{y}");
    }
  }
  Ok(())
}

/// Aceita `DD/MM`, `DD/MM/AAAA` ou `AAAA-MM-DD`.
pub fn parse_birthday_date(input: &str, today: NaiveDate) -> Result<(u32, u32, Option<i32>)> {
  let s = input.trim();
  if let Ok(date) = NaiveDate::parse_from_str(s, DATE_FORMAT) {
    let (m, d, y) = (date.month(), date.day(), date.year());
    validate_birthday(m, d, Some(y), today)?;
    return Ok((m, d, Some(y)));
  }

  let parts: Vec<&str> = s.split('/').collect();
  let (day, month, year) = match parts.as_slice() {
    [d, m] => (
      d.parse::<u32>().map_err(|_| anyhow::anyhow!("Data de aniversário inválida: '{input}'. Use DD/MM, DD/MM/AAAA ou AAAA-MM-DD"))?,
      m.parse::<u32>().map_err(|_| anyhow::anyhow!("Data de aniversário inválida: '{input}'. Use DD/MM, DD/MM/AAAA ou AAAA-MM-DD"))?,
      None,
    ),
    [d, m, y] if y.len() == 4 => (
      d.parse::<u32>().map_err(|_| anyhow::anyhow!("Data de aniversário inválida: '{input}'. Use DD/MM, DD/MM/AAAA ou AAAA-MM-DD"))?,
      m.parse::<u32>().map_err(|_| anyhow::anyhow!("Data de aniversário inválida: '{input}'. Use DD/MM, DD/MM/AAAA ou AAAA-MM-DD"))?,
      Some(y.parse::<i32>().map_err(|_| anyhow::anyhow!("Ano inválido em '{input}'"))?),
    ),
    _ => bail!("Data de aniversário inválida: '{input}'. Use DD/MM, DD/MM/AAAA ou AAAA-MM-DD"),
  };

  validate_birthday(month, day, year, today)?;
  Ok((month, day, year))
}

fn birthday_in_year(year: i32, month: u32, day: u32) -> NaiveDate {
  NaiveDate::from_ymd_opt(year, month, day)
    .unwrap_or_else(|| NaiveDate::from_ymd_opt(year, 2, 28).unwrap())
}

pub fn next_birthday(month: u32, day: u32, today: NaiveDate) -> NaiveDate {
  let this_year = birthday_in_year(today.year(), month, day);
  if this_year >= today {
    this_year
  } else {
    birthday_in_year(today.year() + 1, month, day)
  }
}

pub fn turning_age(birth_year: Option<i32>, next_date: NaiveDate) -> Option<u32> {
  birth_year.and_then(|y| (next_date.year() >= y).then(|| (next_date.year() - y) as u32))
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
    assert_eq!(parse_due("05/11/2026", today).unwrap(), d("2026-11-05"));
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

  #[test]
  fn parses_and_computes_birthdays() {
    let today = d("2026-11-13");
    assert_eq!(parse_birthday_date("15/03", today).unwrap(), (3, 15, None));
    assert_eq!(parse_birthday_date("5/3/1995", today).unwrap(), (3, 5, Some(1995)));
    assert_eq!(parse_birthday_date("1995-03-05", today).unwrap(), (3, 5, Some(1995)));
    assert_eq!(parse_birthday_date("29/02", today).unwrap(), (2, 29, None));
    assert!(parse_birthday_date("29/02/2023", today).is_err());
    assert!(parse_birthday_date("31/04", today).is_err());
    assert!(parse_birthday_date("15/03/2030", today).is_err());

    assert_eq!(next_birthday(11, 13, today), d("2026-11-13"));
    assert_eq!(next_birthday(12, 1, today), d("2026-12-01"));
    assert_eq!(next_birthday(3, 15, today), d("2027-03-15"));
    assert_eq!(next_birthday(2, 29, d("2026-01-10")), d("2026-02-28"));
    assert_eq!(next_birthday(2, 29, d("2024-01-10")), d("2024-02-29"));

    assert_eq!(turning_age(Some(1995), d("2027-03-15")), Some(32));
    assert_eq!(turning_age(None, d("2027-03-15")), None);
  }
}
