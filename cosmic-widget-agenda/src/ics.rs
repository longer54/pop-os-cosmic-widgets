//! Pobieranie i parsowanie prywatnego feedu iCal (ICS) z Kalendarza Google
//! (Ustawienia kalendarza -> "Integracja kalendarza" -> "Tajny adres w
//! formacie iCal"). Własny, minimalny parser RFC5545 — obsługuje odfoldowanie
//! linii, VEVENT, DTSTART/DTEND (Z, TZID, DATE), oraz podstawowy RRULE
//! (FREQ=DAILY/WEEKLY/MONTHLY/YEARLY, INTERVAL, COUNT, UNTIL, BYDAY) i EXDATE.
//! To wystarcza dla typowych wydarzeń osobistego kalendarza (spotkania
//! cykliczne, urodziny, przypomnienia) — nie jest to pełna implementacja
//! RFC5545 (pomija np. BYMONTHDAY/BYSETPOS).

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, NaiveDateTime, TimeZone, Timelike, Weekday};
use chrono_tz::Tz;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct AgendaEvent {
    pub summary: String,
    pub start: DateTime<Local>,
    pub end: DateTime<Local>,
    pub all_day: bool,
    pub location: Option<String>,
}

pub async fn fetch_agenda(url: String, days_ahead: i64) -> Result<Vec<AgendaEvent>, String> {
    if url.trim().is_empty() {
        return Err("brak ics_url w configu".into());
    }
    let raw = fetch_ics(url).await?;
    Ok(parse_and_expand(&raw, days_ahead))
}

async fn fetch_ics(url: String) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status().as_u16()));
    }
    resp.text().await.map_err(|e| e.to_string())
}

// =========================================================================
// Parsowanie ICS
// =========================================================================

#[derive(Debug, Clone)]
struct Prop {
    params: Vec<(String, String)>,
    value: String,
}

impl Prop {
    fn param(&self, key: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

struct RawEvent {
    summary: String,
    dtstart: Option<Prop>,
    dtend: Option<Prop>,
    rrule: Option<String>,
    exdates: Vec<Prop>,
    location: Option<String>,
}

fn unfold(raw: &str) -> Vec<String> {
    let normalized = raw.replace("\r\n", "\n");
    let mut out: Vec<String> = Vec::new();
    for line in normalized.split('\n') {
        if (line.starts_with(' ') || line.starts_with('\t')) && !out.is_empty() {
            if let Some(last) = out.last_mut() {
                last.push_str(&line[1..]);
            }
        } else if !line.trim().is_empty() {
            out.push(line.to_string());
        }
    }
    out
}

fn parse_prop_line(line: &str) -> Option<(String, Prop)> {
    let colon = line.find(':')?;
    let head = &line[..colon];
    let value = line[colon + 1..].to_string();
    let mut parts = head.split(';');
    let name = parts.next()?.trim().to_uppercase();
    let mut params = Vec::new();
    for p in parts {
        if let Some(eq) = p.find('=') {
            params.push((p[..eq].trim().to_uppercase(), p[eq + 1..].trim().to_string()));
        }
    }
    Some((name, Prop { params, value }))
}

fn unescape_text(s: &str) -> String {
    s.replace("\\n", " ")
        .replace("\\N", " ")
        .replace("\\,", ",")
        .replace("\\;", ";")
        .replace("\\\\", "\\")
}

fn parse_events(lines: &[String]) -> Vec<RawEvent> {
    let mut events = Vec::new();
    let mut cur: Option<RawEvent> = None;
    for line in lines {
        if line.eq_ignore_ascii_case("BEGIN:VEVENT") {
            cur = Some(RawEvent {
                summary: String::new(),
                dtstart: None,
                dtend: None,
                rrule: None,
                exdates: Vec::new(),
                location: None,
            });
            continue;
        }
        if line.eq_ignore_ascii_case("END:VEVENT") {
            if let Some(ev) = cur.take() {
                events.push(ev);
            }
            continue;
        }
        let Some(ev) = cur.as_mut() else { continue };
        let Some((name, prop)) = parse_prop_line(line) else { continue };
        match name.as_str() {
            "SUMMARY" => ev.summary = unescape_text(&prop.value),
            "DTSTART" => ev.dtstart = Some(prop),
            "DTEND" => ev.dtend = Some(prop),
            "RRULE" => ev.rrule = Some(prop.value),
            "EXDATE" => ev.exdates.push(prop),
            "LOCATION" => ev.location = Some(unescape_text(&prop.value)),
            _ => {}
        }
    }
    events
}

/// Zwraca (data/czas w czasie lokalnym, czy to wydarzenie całodniowe).
fn parse_datetime(prop: &Prop) -> Option<(DateTime<Local>, bool)> {
    let value = prop.value.trim();
    let is_date_only = prop
        .param("VALUE")
        .map(|v| v.eq_ignore_ascii_case("DATE"))
        .unwrap_or(false)
        || (value.len() == 8 && value.chars().all(|c| c.is_ascii_digit()));

    if is_date_only {
        let d = NaiveDate::parse_from_str(value, "%Y%m%d").ok()?;
        let dt = Local.from_local_datetime(&d.and_hms_opt(0, 0, 0)?).single()?;
        return Some((dt, true));
    }

    if let Some(base) = value.strip_suffix('Z') {
        let ndt = NaiveDateTime::parse_from_str(base, "%Y%m%dT%H%M%S").ok()?;
        let utc = chrono::Utc.from_utc_datetime(&ndt);
        return Some((utc.with_timezone(&Local), false));
    }

    let ndt = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S").ok()?;
    if let Some(tz_name) = prop.param("TZID") {
        if let Ok(tz) = Tz::from_str(tz_name) {
            if let Some(in_tz) = tz.from_local_datetime(&ndt).single() {
                return Some((in_tz.with_timezone(&Local), false));
            }
        }
    }
    let dt = Local.from_local_datetime(&ndt).single()?;
    Some((dt, false))
}

fn same_instant(a: DateTime<Local>, b: DateTime<Local>) -> bool {
    (a - b).num_seconds().abs() < 60
}

// =========================================================================
// RRULE (podzbiór RFC5545 wystarczający dla typowych wydarzeń osobistych)
// =========================================================================

#[derive(Debug, Clone, Copy)]
enum Freq {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

struct Rule {
    freq: Freq,
    interval: i64,
    count: Option<u32>,
    until: Option<DateTime<Local>>,
    byday: Vec<Weekday>,
}

fn weekday_from_code(s: &str) -> Option<Weekday> {
    match s {
        "MO" => Some(Weekday::Mon),
        "TU" => Some(Weekday::Tue),
        "WE" => Some(Weekday::Wed),
        "TH" => Some(Weekday::Thu),
        "FR" => Some(Weekday::Fri),
        "SA" => Some(Weekday::Sat),
        "SU" => Some(Weekday::Sun),
        _ => None,
    }
}

fn parse_until(v: &str) -> Option<DateTime<Local>> {
    if let Some(base) = v.strip_suffix('Z') {
        let ndt = NaiveDateTime::parse_from_str(base, "%Y%m%dT%H%M%S").ok()?;
        return Some(chrono::Utc.from_utc_datetime(&ndt).with_timezone(&Local));
    }
    if v.len() == 8 {
        let d = NaiveDate::parse_from_str(v, "%Y%m%d").ok()?;
        return Local.from_local_datetime(&d.and_hms_opt(0, 0, 0)?).single();
    }
    let ndt = NaiveDateTime::parse_from_str(v, "%Y%m%dT%H%M%S").ok()?;
    Local.from_local_datetime(&ndt).single()
}

fn parse_rrule(s: &str) -> Option<Rule> {
    let mut freq = None;
    let mut interval = 1i64;
    let mut count = None;
    let mut until = None;
    let mut byday = Vec::new();
    for part in s.split(';') {
        let mut kv = part.splitn(2, '=');
        let k = kv.next()?.trim().to_uppercase();
        let v = kv.next()?.trim();
        match k.as_str() {
            "FREQ" => {
                freq = match v {
                    "DAILY" => Some(Freq::Daily),
                    "WEEKLY" => Some(Freq::Weekly),
                    "MONTHLY" => Some(Freq::Monthly),
                    "YEARLY" => Some(Freq::Yearly),
                    _ => None,
                }
            }
            "INTERVAL" => interval = v.parse().unwrap_or(1),
            "COUNT" => count = v.parse().ok(),
            "UNTIL" => until = parse_until(v),
            "BYDAY" => {
                for d in v.split(',') {
                    let code = d.trim_start_matches(|c: char| c.is_ascii_digit() || c == '-' || c == '+');
                    if let Some(w) = weekday_from_code(code) {
                        byday.push(w);
                    }
                }
            }
            _ => {}
        }
    }
    Some(Rule {
        freq: freq?,
        interval: interval.max(1),
        count,
        until,
        byday,
    })
}

const MAX_ITERATIONS: u32 = 3660;

fn add_months(dt: DateTime<Local>, months: i64) -> Option<DateTime<Local>> {
    let total_month0 = dt.month0() as i64 + months;
    let year = dt.year() as i64 + total_month0.div_euclid(12);
    let month0 = total_month0.rem_euclid(12);
    let month = (month0 + 1) as u32;
    let last_day = last_day_of_month(year as i32, month);
    let day = dt.day().min(last_day);
    let naive_date = NaiveDate::from_ymd_opt(year as i32, month, day)?;
    let naive_dt = naive_date.and_time(dt.time());
    Local.from_local_datetime(&naive_dt).single()
}

fn last_day_of_month(year: i32, month: u32) -> u32 {
    let (ny, nm) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    NaiveDate::from_ymd_opt(ny, nm, 1)
        .and_then(|d| d.pred_opt())
        .map(|d| d.day())
        .unwrap_or(28)
}

fn expand_weekly_byday(dtstart: DateTime<Local>, rule: &Rule, win_end: DateTime<Local>) -> Vec<DateTime<Local>> {
    let mut out = Vec::new();
    let mut days_sorted = rule.byday.clone();
    days_sorted.sort_by_key(|w| w.num_days_from_monday());
    if days_sorted.is_empty() {
        return out;
    }
    let interval = rule.interval.max(1);
    let start_monday = dtstart.date_naive() - Duration::days(dtstart.weekday().num_days_from_monday() as i64);
    let mut n: u32 = 0;
    let mut week_index: i64 = 0;
    let mut iterations: u32 = 0;

    loop {
        let week_monday = start_monday + Duration::days(7 * week_index * interval);
        if week_monday > win_end.date_naive() {
            break;
        }
        for wd in &days_sorted {
            iterations += 1;
            if iterations > MAX_ITERATIONS {
                return out;
            }
            let day = week_monday + Duration::days(wd.num_days_from_monday() as i64);
            let Some(naive_dt) = day.and_hms_opt(dtstart.hour(), dtstart.minute(), dtstart.second()) else {
                continue;
            };
            let Some(occ) = Local.from_local_datetime(&naive_dt).single() else {
                continue;
            };
            if occ < dtstart {
                continue;
            }
            if let Some(until) = rule.until {
                if occ > until {
                    return out;
                }
            }
            n += 1;
            if let Some(c) = rule.count {
                if n > c {
                    return out;
                }
            }
            if occ <= win_end {
                out.push(occ);
            }
        }
        week_index += 1;
    }
    out
}

fn expand_stepped(dtstart: DateTime<Local>, rule: &Rule, win_end: DateTime<Local>) -> Vec<DateTime<Local>> {
    let mut out = Vec::new();
    let mut cursor = dtstart;
    let mut n: u32 = 0;
    let mut iterations: u32 = 0;
    loop {
        iterations += 1;
        if iterations > MAX_ITERATIONS {
            break;
        }
        if let Some(until) = rule.until {
            if cursor > until {
                break;
            }
        }
        n += 1;
        if let Some(c) = rule.count {
            if n > c {
                break;
            }
        }
        if cursor > win_end {
            break;
        }
        out.push(cursor);
        let next = match rule.freq {
            Freq::Daily => Some(cursor + Duration::days(rule.interval)),
            Freq::Weekly => Some(cursor + Duration::days(7 * rule.interval)),
            Freq::Monthly => add_months(cursor, rule.interval),
            Freq::Yearly => add_months(cursor, rule.interval * 12),
        };
        match next {
            Some(c) => cursor = c,
            None => break,
        }
    }
    out
}

fn expand_rrule(dtstart: DateTime<Local>, rule: &Rule, win_end: DateTime<Local>) -> Vec<DateTime<Local>> {
    if matches!(rule.freq, Freq::Weekly) && !rule.byday.is_empty() {
        expand_weekly_byday(dtstart, rule, win_end)
    } else {
        expand_stepped(dtstart, rule, win_end)
    }
}

// =========================================================================
// Złożenie: ICS -> lista wydarzeń w oknie [dziś 00:00, dziś+days_ahead)
// =========================================================================

fn parse_and_expand(raw: &str, days_ahead: i64) -> Vec<AgendaEvent> {
    let lines = unfold(raw);
    let raw_events = parse_events(&lines);

    let now = Local::now();
    let win_start = match now.date_naive().and_hms_opt(0, 0, 0) {
        Some(ndt) => Local.from_local_datetime(&ndt).single().unwrap_or(now),
        None => now,
    };
    let win_end = win_start + Duration::days(days_ahead.max(1)) - Duration::seconds(1);

    let mut out = Vec::new();
    for ev in &raw_events {
        let Some(dtstart_prop) = &ev.dtstart else { continue };
        let Some((start, all_day)) = parse_datetime(dtstart_prop) else { continue };
        let end = ev
            .dtend
            .as_ref()
            .and_then(parse_datetime)
            .map(|(e, _)| e)
            .unwrap_or_else(|| start + if all_day { Duration::days(1) } else { Duration::hours(1) });
        let duration = end - start;
        let exdates: Vec<DateTime<Local>> = ev
            .exdates
            .iter()
            .filter_map(|p| parse_datetime(p).map(|(d, _)| d))
            .collect();

        if let Some(rrule_str) = &ev.rrule {
            if let Some(rule) = parse_rrule(rrule_str) {
                for occ_start in expand_rrule(start, &rule, win_end) {
                    if exdates.iter().any(|ex| same_instant(*ex, occ_start)) {
                        continue;
                    }
                    let occ_end = occ_start + duration;
                    if occ_end > win_start && occ_start <= win_end {
                        out.push(AgendaEvent {
                            summary: ev.summary.clone(),
                            start: occ_start,
                            end: occ_end,
                            all_day,
                            location: ev.location.clone(),
                        });
                    }
                }
                continue;
            }
        }

        if end > win_start && start <= win_end {
            out.push(AgendaEvent {
                summary: ev.summary.clone(),
                start,
                end,
                all_day,
                location: ev.location.clone(),
            });
        }
    }

    out.sort_by_key(|e| e.start);
    out
}
