use super::College;
use chrono::{Datelike, NaiveDate};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::OnceLock;

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
struct BellData {
    opec: Vec<(String, String)>,
    omaep: Vec<(String, String)>,
    special: BTreeMap<String, BTreeMap<String, Vec<(String, String)>>>,
}

static BELLS: OnceLock<BellData> = OnceLock::new();

fn bells() -> &'static BellData {
    BELLS.get_or_init(|| {
        serde_json::from_str(include_str!("../../../shared/bells.json"))
            .expect("shared/bells.json должен содержать корректную таблицу звонков")
    })
}

pub fn lesson_times(college: College, date: NaiveDate, pair: u8) -> Option<(String, String)> {
    if pair == 0 || pair > 6 {
        return None;
    }
    let college_key = match college {
        College::Opec => "opec",
        College::Omaep => "omaep",
    };
    let date_key = format_date(date);
    let data = bells();
    let special = data.special.get(college_key).and_then(|days| days.get(&date_key));
    let regular = match college {
        College::Opec => &data.opec,
        College::Omaep => &data.omaep,
    };
    special.unwrap_or(regular).get(pair as usize - 1).cloned()
}

pub fn parse_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%d.%m.%Y").ok()
}

pub fn format_date(value: NaiveDate) -> String {
    value.format("%d.%m.%Y").to_string()
}

pub fn human_date(value: NaiveDate) -> String {
    let weekdays = ["Пн", "Вт", "Ср", "Чт", "Пт", "Сб", "Вс"];
    let months = [
        "января", "февраля", "марта", "апреля", "мая", "июня", "июля", "августа",
        "сентября", "октября", "ноября", "декабря",
    ];
    format!(
        "{}, {} {}",
        weekdays[value.weekday().num_days_from_monday() as usize],
        value.day(),
        months[value.month0() as usize]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn special_bells_are_in_one_place() {
        assert_eq!(lesson_times(College::Opec, parse_date("01.09.2026").unwrap(), 1), Some(("13:00".into(), "13:50".into())));
        assert_eq!(lesson_times(College::Omaep, parse_date("02.11.2026").unwrap(), 2), Some(("10:00".into(), "11:40".into())));
    }

    #[test]
    fn human_date_is_russian() {
        assert_eq!(human_date(parse_date("03.11.2026").unwrap()), "Вт, 3 ноября");
    }
}
