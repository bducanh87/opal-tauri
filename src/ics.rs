//! iCalendar (ICS) feeds: fetch, parse and expand recurring events.
//! Shared by the egui and Tauri editions.

use chrono::{Datelike, Duration as CDuration, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Weekday};
use std::time::Duration;

#[derive(Clone, Debug, serde::Serialize)]
pub struct Event {
    pub date: NaiveDate,
    /// Exclusive end day for multi-day events.
    pub end: NaiveDate,
    pub time: Option<NaiveTime>,
    pub title: String,
}

// ------------------------------------------------------------------ ICS --

pub fn fetch(url: &str) -> Result<Vec<Event>, String> {
    let url = if let Some(rest) = url.strip_prefix("webcal://") { format!("https://{rest}") } else { url.to_string() };
    let body = ureq::get(&url)
        .timeout(Duration::from_secs(20))
        .call()
        .map_err(|e| format!("Sync failed: {e}"))?
        .into_string()
        .map_err(|e| format!("Sync failed: {e}"))?;
    if !body.contains("BEGIN:VCALENDAR") {
        return Err("That address is not an iCalendar feed".into());
    }
    Ok(parse_ics(&body, Local::now().date_naive()))
}

fn unfold(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in text.split('\n') {
        let line = raw.trim_end_matches('\r');
        if (line.starts_with(' ') || line.starts_with('\t')) && !out.is_empty() {
            out.last_mut().unwrap().push_str(&line[1..]);
        } else {
            out.push(line.to_string());
        }
    }
    out
}

fn split_prop(line: &str) -> Option<(String, String, String)> {
    let colon = line.find(':')?;
    let (head, value) = (&line[..colon], &line[colon + 1..]);
    let mut parts = head.splitn(2, ';');
    let name = parts.next()?.to_ascii_uppercase();
    let params = parts.next().unwrap_or("").to_string();
    Some((name, params, value.to_string()))
}

/// (local date, local time if timed)
fn parse_when(params: &str, value: &str) -> Option<(NaiveDate, Option<NaiveTime>)> {
    let value = value.trim();
    if params.to_ascii_uppercase().contains("VALUE=DATE") && !value.contains('T') || value.len() == 8 {
        return NaiveDate::parse_from_str(value.get(..8)?, "%Y%m%d").ok().map(|d| (d, None));
    }
    let utc = value.ends_with('Z');
    let naive = NaiveDateTime::parse_from_str(value.trim_end_matches('Z'), "%Y%m%dT%H%M%S").ok()?;
    let local: NaiveDateTime = if utc {
        chrono::Utc.from_utc_datetime(&naive).with_timezone(&Local).naive_local()
    } else if let Some(tzid) = params.split(';').find_map(|p| p.strip_prefix("TZID=")) {
        match iana_zone(tzid.trim_matches('"')).parse::<chrono_tz::Tz>() {
            Ok(tz) => tz
                .from_local_datetime(&naive)
                .earliest()
                .map(|d| d.with_timezone(&Local).naive_local())
                .unwrap_or(naive),
            Err(_) => naive,
        }
    } else {
        naive
    };
    Some((local.date(), Some(local.time())))
}

/// Outlook writes Windows zone names (TZID:"SE Asia Standard Time").
/// Map the common ones to IANA names (CLDR windowsZones, primary zones).
fn iana_zone(name: &str) -> &str {
    match name {
        "UTC" | "Coordinated Universal Time" => "UTC",
        "GMT Standard Time" => "Europe/London",
        "Greenwich Standard Time" => "Atlantic/Reykjavik",
        "W. Europe Standard Time" => "Europe/Berlin",
        "Romance Standard Time" => "Europe/Paris",
        "Central Europe Standard Time" => "Europe/Budapest",
        "Central European Standard Time" => "Europe/Warsaw",
        "E. Europe Standard Time" => "Europe/Chisinau",
        "FLE Standard Time" => "Europe/Kiev",
        "GTB Standard Time" => "Europe/Bucharest",
        "Russian Standard Time" => "Europe/Moscow",
        "Turkey Standard Time" => "Europe/Istanbul",
        "Israel Standard Time" => "Asia/Jerusalem",
        "Arabian Standard Time" => "Asia/Dubai",
        "Arab Standard Time" => "Asia/Riyadh",
        "Iran Standard Time" => "Asia/Tehran",
        "Pakistan Standard Time" => "Asia/Karachi",
        "India Standard Time" => "Asia/Kolkata",
        "Nepal Standard Time" => "Asia/Kathmandu",
        "Bangladesh Standard Time" => "Asia/Dhaka",
        "Myanmar Standard Time" => "Asia/Yangon",
        "SE Asia Standard Time" => "Asia/Bangkok",
        "China Standard Time" => "Asia/Shanghai",
        "Singapore Standard Time" => "Asia/Singapore",
        "Taipei Standard Time" => "Asia/Taipei",
        "W. Australia Standard Time" => "Australia/Perth",
        "Tokyo Standard Time" => "Asia/Tokyo",
        "Korea Standard Time" => "Asia/Seoul",
        "AUS Eastern Standard Time" => "Australia/Sydney",
        "E. Australia Standard Time" => "Australia/Brisbane",
        "Cen. Australia Standard Time" => "Australia/Adelaide",
        "New Zealand Standard Time" => "Pacific/Auckland",
        "Hawaiian Standard Time" => "Pacific/Honolulu",
        "Alaskan Standard Time" => "America/Anchorage",
        "Pacific Standard Time" => "America/Los_Angeles",
        "US Mountain Standard Time" => "America/Phoenix",
        "Mountain Standard Time" => "America/Denver",
        "Central Standard Time" => "America/Chicago",
        "Eastern Standard Time" => "America/New_York",
        "Atlantic Standard Time" => "America/Halifax",
        "SA Pacific Standard Time" => "America/Bogota",
        "E. South America Standard Time" => "America/Sao_Paulo",
        "Argentina Standard Time" => "America/Buenos_Aires",
        "South Africa Standard Time" => "Africa/Johannesburg",
        "Egypt Standard Time" => "Africa/Cairo",
        other => other,
    }
}

fn unescape(s: &str) -> String {
    s.replace("\\n", " ").replace("\\N", " ").replace("\\,", ",").replace("\\;", ";").replace("\\\\", "\\")
}

fn weekday(code: &str) -> Option<Weekday> {
    Some(match code.trim_start_matches(|c: char| c == '+' || c == '-' || c.is_ascii_digit()) {
        "MO" => Weekday::Mon,
        "TU" => Weekday::Tue,
        "WE" => Weekday::Wed,
        "TH" => Weekday::Thu,
        "FR" => Weekday::Fri,
        "SA" => Weekday::Sat,
        "SU" => Weekday::Sun,
        _ => return None,
    })
}

pub fn add_months(d: NaiveDate, months: i32) -> Option<NaiveDate> {
    let total = d.year() * 12 + d.month0() as i32 + months;
    NaiveDate::from_ymd_opt(total.div_euclid(12), total.rem_euclid(12) as u32 + 1, d.day())
}

pub fn parse_ics(text: &str, today: NaiveDate) -> Vec<Event> {
    let window_start = today - CDuration::days(400);
    let window_end = today + CDuration::days(400);
    let mut events = Vec::new();
    let mut all: Vec<Vec<(String, String, String)>> = Vec::new();
    let mut cur: Option<Vec<(String, String, String)>> = None;
    for line in unfold(text) {
        if line == "BEGIN:VEVENT" {
            cur = Some(Vec::new());
            continue;
        }
        if line == "END:VEVENT" {
            if let Some(props) = cur.take() {
                all.push(props);
            }
            continue;
        }
        if let (Some(props), Some(p)) = (cur.as_mut(), split_prop(&line)) {
            props.push(p);
        }
    }
    // Instances moved or edited in the source calendar carry RECURRENCE-ID;
    // their original dates must be left out of the repeating series.
    let mut moved: std::collections::HashMap<String, Vec<NaiveDate>> = std::collections::HashMap::new();
    for props in &all {
        let uid = props.iter().find(|p| p.0 == "UID").map(|p| p.2.clone());
        let rid = props.iter().find(|p| p.0 == "RECURRENCE-ID").and_then(|p| parse_when(&p.1, &p.2));
        if let (Some(uid), Some((d, _))) = (uid, rid) {
            moved.entry(uid).or_default().push(d);
        }
    }
    for props in &all {
        let is_override = props.iter().any(|p| p.0 == "RECURRENCE-ID");
        let extra: &[NaiveDate] = if is_override {
            &[]
        } else {
            props
                .iter()
                .find(|p| p.0 == "UID")
                .and_then(|p| moved.get(&p.2))
                .map(Vec::as_slice)
                .unwrap_or(&[])
        };
        expand(props, extra, window_start, window_end, &mut events);
    }
    events.sort_by_key(|e| (e.date, e.time));
    events
}

/// nth weekday of a month (n = 1..5, or -1 for the last one).
fn nth_weekday(year: i32, month: u32, wd: Weekday, n: i32) -> Option<NaiveDate> {
    if n > 0 {
        let first = NaiveDate::from_ymd_opt(year, month, 1)?;
        let offset = (wd.num_days_from_monday() as i64 - first.weekday().num_days_from_monday() as i64).rem_euclid(7);
        let d = first + CDuration::days(offset + 7 * (n as i64 - 1));
        (d.month() == month).then_some(d)
    } else {
        let next = if month == 12 { NaiveDate::from_ymd_opt(year + 1, 1, 1)? } else { NaiveDate::from_ymd_opt(year, month + 1, 1)? };
        let last = next - CDuration::days(1);
        let back = (last.weekday().num_days_from_monday() as i64 - wd.num_days_from_monday() as i64).rem_euclid(7);
        let d = last - CDuration::days(back + 7 * (-n as i64 - 1));
        (d.month() == month).then_some(d)
    }
}

/// "2TU" -> (2, Tue); "TU" -> (0, Tue); "-1FR" -> (-1, Fri).
fn ordinal_weekday(code: &str) -> Option<(i32, Weekday)> {
    let split = code.find(|c: char| c.is_ascii_alphabetic())?;
    let n: i32 = if split == 0 { 0 } else { code[..split].parse().ok()? };
    Some((n, weekday(&code[split..])?))
}

fn expand(props: &[(String, String, String)], moved: &[NaiveDate], from: NaiveDate, to: NaiveDate, out: &mut Vec<Event>) {
    let get = |name: &str| props.iter().find(|p| p.0 == name);
    if get("STATUS").is_some_and(|s| s.2.eq_ignore_ascii_case("CANCELLED")) {
        return;
    }
    let Some((_, sp, sv)) = get("DTSTART") else { return };
    let Some((start, time)) = parse_when(sp, sv) else { return };
    let end = get("DTEND")
        .and_then(|(_, p, v)| parse_when(p, v))
        .map(|(d, t)| if t.is_some() && d > start { d + CDuration::days(1) } else { d })
        .unwrap_or(start + CDuration::days(1));
    let span = (end - start).num_days().max(1);
    let title = get("SUMMARY").map(|s| unescape(&s.2)).unwrap_or_else(|| "(No title)".into());
    let mut excluded: Vec<NaiveDate> = props
        .iter()
        .filter(|p| p.0 == "EXDATE")
        .flat_map(|p| p.2.split(',').filter_map(|v| parse_when(&p.1, v).map(|w| w.0)).collect::<Vec<_>>())
        .collect();
    excluded.extend_from_slice(moved);
    let mut push = |d: NaiveDate| {
        if d + CDuration::days(span) >= from && d <= to && !excluded.contains(&d) {
            out.push(Event { date: d, end: d + CDuration::days(span), time, title: title.clone() });
        }
    };
    let Some((_, _, rule)) = get("RRULE") else {
        push(start);
        return;
    };
    let mut freq = "";
    let mut interval = 1i64;
    let mut count: Option<u32> = None;
    let mut until: Option<NaiveDate> = None;
    let mut byday: Vec<(i32, Weekday)> = Vec::new();
    let mut bymonthday: Vec<i32> = Vec::new();
    for part in rule.split(';') {
        let Some((k, v)) = part.split_once('=') else { continue };
        match k {
            "FREQ" => freq = v,
            "INTERVAL" => interval = v.parse().unwrap_or(1).max(1),
            "COUNT" => count = v.parse().ok(),
            "UNTIL" => until = parse_when("", v).map(|w| w.0),
            "BYDAY" => byday = v.split(',').filter_map(ordinal_weekday).collect(),
            "BYMONTHDAY" => bymonthday = v.split(',').filter_map(|x| x.parse().ok()).collect(),
            _ => {}
        }
    }
    let last = until.unwrap_or(to).min(to);
    // COUNT counts every occurrence from DTSTART; the window only limits
    // what is shown. A step bound keeps a broken rule from spinning.
    let max = count.unwrap_or(u32::MAX);
    let mut n = 0u32;
    let mut emit = |d: NaiveDate, n: &mut u32| -> bool {
        if d > last || *n >= max {
            return false;
        }
        *n += 1;
        push(d);
        true
    };
    const STEPS: i64 = 40_000;
    match freq {
        "DAILY" => {
            let mut d = start;
            for _ in 0..STEPS {
                if !emit(d, &mut n) {
                    break;
                }
                d += CDuration::days(interval);
            }
        }
        "WEEKLY" => {
            let days: Vec<Weekday> = if byday.is_empty() { vec![start.weekday()] } else { byday.iter().map(|x| x.1).collect() };
            let mut week = start - CDuration::days(start.weekday().num_days_from_monday() as i64);
            'outer: for _ in 0..STEPS {
                for wd in 0..7u32 {
                    let d = week + CDuration::days(wd as i64);
                    if d < start || !days.iter().any(|x| x.num_days_from_monday() == wd) {
                        continue;
                    }
                    if !emit(d, &mut n) {
                        break 'outer;
                    }
                }
                week += CDuration::weeks(interval);
                if week > last {
                    break;
                }
            }
        }
        "MONTHLY" => {
            let base = start.with_day(1).unwrap();
            'months: for k in 0..STEPS / 30 {
                let Some(month) = add_months(base, (k * interval) as i32) else { continue };
                if month > last {
                    break;
                }
                let (y, m) = (month.year(), month.month());
                let mut dates: Vec<NaiveDate> = if !byday.is_empty() {
                    byday
                        .iter()
                        .flat_map(|&(nth, wd)| {
                            if nth != 0 {
                                nth_weekday(y, m, wd, nth).into_iter().collect::<Vec<_>>()
                            } else {
                                (1..=5).filter_map(|i| nth_weekday(y, m, wd, i)).collect()
                            }
                        })
                        .collect()
                } else if !bymonthday.is_empty() {
                    bymonthday
                        .iter()
                        .filter_map(|&md| {
                            if md > 0 {
                                NaiveDate::from_ymd_opt(y, m, md as u32)
                            } else {
                                nth_last_day(y, m, -md)
                            }
                        })
                        .collect()
                } else {
                    // Same day of month as DTSTART; months without it are skipped.
                    NaiveDate::from_ymd_opt(y, m, start.day()).into_iter().collect()
                };
                dates.sort();
                for d in dates {
                    if d < start {
                        continue;
                    }
                    if !emit(d, &mut n) {
                        break 'months;
                    }
                }
            }
        }
        "YEARLY" => {
            for k in 0..STEPS / 365 + 400 {
                let Some(d) = add_months(start, (k * interval * 12) as i32) else { continue };
                if !emit(d, &mut n) {
                    break;
                }
            }
        }
        _ => push(start),
    }
}

/// The n-th day counted from the end of a month (1 = last day).
fn nth_last_day(year: i32, month: u32, n: i32) -> Option<NaiveDate> {
    let next = if month == 12 { NaiveDate::from_ymd_opt(year + 1, 1, 1)? } else { NaiveDate::from_ymd_opt(year, month + 1, 1)? };
    let d = next - CDuration::days(n as i64);
    (d.month() == month).then_some(d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monthly_rules() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 9).unwrap();
        let ics = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:a\r\nDTSTART;VALUE=DATE:20260131\r\nRRULE:FREQ=MONTHLY;COUNT=4\r\nSUMMARY:End\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:b\r\nDTSTART;VALUE=DATE:20261110\r\nRRULE:FREQ=MONTHLY;BYDAY=2TU;COUNT=3\r\nSUMMARY:Second Tue\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:c\r\nDTSTART;VALUE=DATE:20190101\r\nRRULE:FREQ=DAILY\r\nSUMMARY:Daily\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:d\r\nDTSTART;VALUE=DATE:20261103\r\nRRULE:FREQ=WEEKLY;COUNT=3\r\nSUMMARY:Weekly\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:d\r\nRECURRENCE-ID;VALUE=DATE:20261110\r\nDTSTART;VALUE=DATE:20261111\r\nSUMMARY:Weekly\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let ev = parse_ics(ics, today);
        let dates = |t: &str| ev.iter().filter(|e| e.title == t).map(|e| e.date.to_string()).collect::<Vec<_>>();
        // Jan 31, then only months that have a 31st: Mar, May, Jul.
        assert_eq!(dates("End"), vec!["2026-01-31", "2026-03-31", "2026-05-31", "2026-07-31"]);
        assert_eq!(dates("Second Tue"), vec!["2026-11-10", "2026-12-08", "2027-01-12"]);
        assert!(dates("Daily").contains(&"2026-11-09".to_string()), "long-running daily events still show");
        assert_eq!(dates("Weekly"), vec!["2026-11-03", "2026-11-11", "2026-11-17"], "moved instance replaces its date");
    }

    #[test]
    fn parses_events_and_rules() {
        let ics = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20261109\r\nDTEND;VALUE=DATE:20261110\r\nSUMMARY:All day\\, really\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nDTSTART:20261110T090000\r\nDTEND:20261110T100000\r\nRRULE:FREQ=WEEKLY;COUNT=3;BYDAY=TU,TH\r\nSUMMARY:Stand\r\n  up\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let today = NaiveDate::from_ymd_opt(2026, 11, 9).unwrap();
        let ev = parse_ics(ics, today);
        assert_eq!(ev[0].title, "All day, really");
        assert!(ev[0].time.is_none());
        let stand: Vec<_> = ev.iter().filter(|e| e.title == "Stand up").map(|e| e.date).collect();
        assert_eq!(
            stand,
            vec![
                NaiveDate::from_ymd_opt(2026, 11, 10).unwrap(),
                NaiveDate::from_ymd_opt(2026, 11, 12).unwrap(),
                NaiveDate::from_ymd_opt(2026, 11, 17).unwrap()
            ]
        );
    }
}
