use super::*;
use chrono::NaiveDate;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

pub fn parse_available(input: &str) -> Result<BTreeMap<String, String>, serde_json::Error> {
    let value: serde_json::Value = serde_json::from_str(input)?;
    let mut dates = BTreeMap::new();
    if let Some(object) = value.as_object() {
        for (key, value) in object {
            if parse_date(key).is_some() {
                if let Some(hash) = value.as_str() {
                    dates.insert(key.clone(), hash.to_owned());
                }
            }
        }
    }
    Ok(dates)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AvailableChanges {
    pub new_dates: Vec<String>,
    pub changed_dates: Vec<String>,
}

pub fn available_changes(
    old: &BTreeMap<String, String>,
    new: &BTreeMap<String, String>,
    today: NaiveDate,
) -> AvailableChanges {
    let mut result = AvailableChanges::default();
    for (date, hash) in new {
        let Some(parsed) = parse_date(date) else { continue };
        if parsed < today {
            continue;
        }
        match old.get(date) {
            None => result.new_dates.push(date.clone()),
            Some(previous) if previous != hash && previous != date => {
                result.changed_dates.push(date.clone())
            }
            _ => {}
        }
    }
    result
}

pub fn normalize_room(room: &str) -> String {
    let trimmed = room.trim();
    let lower = trimmed.to_lowercase();
    let compact: String = lower.chars().filter(|ch| ch.is_alphanumeric()).collect();
    let without_room = compact.contains("безауд") || compact == "беза";
    let short_room = compact
        .strip_prefix("ба")
        .is_some_and(|suffix| suffix.chars().all(|ch| ch.is_ascii_digit()));
    if without_room || short_room {
        "без ауд.".into()
    } else if compact == "дис" {
        "ДИС".into()
    } else {
        trimmed.into()
    }
}

pub fn apply_overlays(day: &mut Day) {
    for (group_name, lessons) in day.ov.overlay.clone() {
        let Some(group) = day.groups.iter_mut().find(|group| group.name == group_name) else {
            continue;
        };
        for (pair, subgroups) in lessons {
            let Ok(pair) = pair.parse::<u8>() else { continue };
            let Some(lesson) = group.lessons.iter_mut().find(|lesson| lesson.pair == pair) else {
                continue;
            };
            for (pg, patch) in subgroups {
                let Ok(pg) = pg.parse::<u8>() else { continue };
                let Some(subgroup) = lesson.subgroups.iter_mut().find(|subgroup| subgroup.pg == pg) else {
                    continue;
                };
                if let Some(value) = patch.discipline {
                    subgroup.discipline = value;
                }
                if let Some(value) = patch.teacher {
                    subgroup.teacher = value;
                }
                if let Some(value) = patch.room {
                    subgroup.room = value;
                }
                subgroup.changed = true;
            }
        }
    }

    for (group_name, lessons) in day.ov.extra.clone() {
        let Some(group) = day.groups.iter_mut().find(|group| group.name == group_name) else {
            continue;
        };
        for (pair, extra) in lessons {
            let Ok(pair) = pair.parse::<u8>() else { continue };
            let subgroup = Subgroup {
                pg: 0,
                discipline: extra.discipline,
                teacher: extra.teacher,
                room: extra.room,
                changed: true,
                lesson_type: None,
                extra: true,
            };
            if let Some(lesson) = group.lessons.iter_mut().find(|lesson| lesson.pair == pair) {
                lesson.subgroups.push(subgroup);
                lesson.subgroups.sort_by_key(|subgroup| subgroup.pg);
            } else {
                group.lessons.push(Lesson { pair, subgroups: vec![subgroup] });
            }
        }
        group.lessons.sort_by_key(|lesson| lesson.pair);
    }

    for group in &mut day.groups {
        if day.ov.hidden.iter().any(|name| name == &group.name) {
            group.lessons.clear();
        }
    }
}

fn homework_for(day: &Day, group: &str, pair: u8) -> Vec<HomeworkEntry> {
    let mut entries: Vec<_> = day
        .ov
        .hw
        .get(group)
        .and_then(|pairs| pairs.get(&pair.to_string()))
        .into_iter()
        .flat_map(|subgroups| subgroups.iter())
        .filter_map(|(pg, homework)| {
            Some(HomeworkEntry {
                pg: pg.parse().ok()?,
                text: homework.text.clone(),
                by_role: homework.by_role.clone(),
            })
        })
        .collect();
    entries.sort_by_key(|entry| entry.pg);
    entries
}

fn lesson_view(day: &Day, group: &str, lesson: &Lesson, teachers: &[String]) -> Option<LessonView> {
    let mut subgroups = lesson.subgroups.clone();
    if !teachers.is_empty() {
        subgroups.retain(|subgroup| {
            let teacher = subgroup.teacher.to_lowercase();
            teachers.iter().any(|selected| teacher.contains(&selected.to_lowercase()))
        });
    }
    if subgroups.is_empty() {
        return None;
    }
    for subgroup in &mut subgroups {
        subgroup.room = normalize_room(&subgroup.room);
    }
    Some(LessonView {
        pair: lesson.pair,
        subgroups,
        homework: homework_for(day, group, lesson.pair),
    })
}

fn group_view(
    day: &Day,
    group: &GroupDay,
    teachers: &[String],
) -> GroupView {
    let hidden = day.ov.hidden.iter().any(|name| name == &group.name);
    let status = day
        .ov
        .statuses
        .get(&group.name)
        .cloned()
        .or_else(|| group.status.clone());
    let lessons = group
        .lessons
        .iter()
        .filter_map(|lesson| lesson_view(day, &group.name, lesson, teachers))
        .collect();
    GroupView {
        name: group.name.clone(),
        status: if hidden { Some("Занятий нет".into()) } else { status },
        hidden,
        lessons,
    }
}

pub fn project_day(raw: &Day, selected_groups: &[String], selected_teachers: &[String]) -> DayView {
    let mut day = raw.clone();
    apply_overlays(&mut day);
    let mut views = vec![];
    let mut used = BTreeSet::new();

    for name in selected_groups {
        if let Some(group) = day.groups.iter().find(|group| &group.name == name) {
            views.push(group_view(&day, group, &[]));
        } else {
            let hidden = day.ov.hidden.iter().any(|hidden| hidden == name);
            views.push(GroupView {
                name: name.clone(),
                status: if hidden { Some("Занятий нет".into()) } else { day.ov.statuses.get(name).cloned() },
                hidden,
                lessons: vec![],
            });
        }
        used.insert(name.clone());
    }

    if !selected_teachers.is_empty() {
        for group in day.groups.iter().filter(|group| !used.contains(&group.name)) {
            let view = group_view(&day, group, selected_teachers);
            if !view.lessons.is_empty() {
                views.push(view);
            }
        }
    }

    let visible: BTreeSet<_> = views.iter().map(|group| group.name.clone()).collect();
    let mut announcements = vec![];
    for (group, items) in &day.ov.announcements {
        if visible.contains(group) {
            announcements.extend(items.iter().map(|item| AnnouncementView {
                group: group.clone(),
                text: item.text.clone(),
                pair: item.pair,
                by_role: item.by_role.clone(),
            }));
        }
    }

    DayView {
        date: day.date,
        week: day.week,
        day: day.day,
        groups: views,
        announcements,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LessonKey {
    pub group: String,
    pub pair: u8,
    pub pg: u8,
    pub extra: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldChange {
    pub field: &'static str,
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Removed,
    Modified,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LessonChange {
    pub key: LessonKey,
    pub kind: ChangeKind,
    pub fields: Vec<FieldChange>,
    pub discipline: String,
    pub extra: bool,
}

fn flatten(view: &DayView) -> BTreeMap<LessonKey, Subgroup> {
    let mut result = BTreeMap::new();
    for group in &view.groups {
        for lesson in &group.lessons {
            for subgroup in &lesson.subgroups {
                result.insert(
                    LessonKey {
                        group: group.name.clone(),
                        pair: lesson.pair,
                        pg: subgroup.pg,
                        extra: subgroup.extra,
                    },
                    subgroup.clone(),
                );
            }
        }
    }
    result
}

pub fn diff_days(before: &DayView, after: &DayView) -> Vec<LessonChange> {
    let old = flatten(before);
    let new = flatten(after);
    let mut changes = vec![];
    for (key, previous) in &old {
        match new.get(key) {
            None => changes.push(LessonChange {
                key: key.clone(),
                kind: ChangeKind::Removed,
                fields: vec![],
                discipline: previous.discipline.clone(),
                extra: previous.extra,
            }),
            Some(current) => {
                let mut fields = vec![];
                for (field, before, after) in [
                    ("дисциплина", &previous.discipline, &current.discipline),
                    ("преподаватель", &previous.teacher, &current.teacher),
                    ("аудитория", &previous.room, &current.room),
                ] {
                    if before != after {
                        fields.push(FieldChange {
                            field,
                            before: before.clone(),
                            after: after.clone(),
                        });
                    }
                }
                if !fields.is_empty() {
                    changes.push(LessonChange {
                        key: key.clone(),
                        kind: ChangeKind::Modified,
                        fields,
                        discipline: current.discipline.clone(),
                        extra: current.extra,
                    });
                }
            }
        }
    }
    for (key, current) in &new {
        if !old.contains_key(key) {
            changes.push(LessonChange {
                key: key.clone(),
                kind: ChangeKind::Added,
                fields: vec![],
                discipline: current.discipline.clone(),
                extra: current.extra,
            });
        }
    }
    changes.sort_by(|left, right| left.key.cmp(&right.key));
    changes
}

fn pairs_word(count: usize) -> String {
    let mod100 = count % 100;
    let mod10 = count % 10;
    if mod10 == 1 && mod100 != 11 {
        format!("{count} пара")
    } else if (2..=4).contains(&mod10) && !(12..=14).contains(&mod100) {
        format!("{count} пары")
    } else {
        format!("{count} пар")
    }
}

pub fn new_day_notification(date: &str, view: &DayView, college: College) -> Option<NotificationEvent> {
    let parsed = parse_date(date)?;
    let mut lines = vec![];
    for group in &view.groups {
        if group.hidden {
            lines.push(format!("{}: занятий нет", group.name));
        } else if group.lessons.is_empty() {
            lines.push(format!("{}: пар нет", group.name));
        } else {
            let first = group.lessons.iter().map(|lesson| lesson.pair).min().unwrap_or(1);
            let start = lesson_times(college, parsed, first).map(|times| times.0).unwrap_or_default();
            lines.push(format!("{}: {}, с {}", group.name, pairs_word(group.lessons.len()), start));
        }
    }
    if lines.is_empty() {
        lines.push("Для выбранных групп и преподавателей пар нет".into());
    }
    Some(NotificationEvent {
        date: date.into(),
        title: format!("{} · Новое расписание", human_date(parsed)),
        body: lines.join("\n"),
    })
}

pub fn changes_notification(date: &str, changes: &[LessonChange]) -> Option<NotificationEvent> {
    if changes.is_empty() {
        return None;
    }
    let parsed = parse_date(date)?;
    let lines = changes.iter().map(|change| {
        let mut prefix = format!("{}, {} пара", change.key.group, change.key.pair);
        if change.key.pg > 0 {
            prefix.push_str(&format!(", {} подгр.", change.key.pg));
        }
        match change.kind {
            ChangeKind::Added if change.extra => format!("{prefix}: добавлена доп. пара, {}", change.discipline),
            ChangeKind::Added => format!("{prefix}: добавлена, {}", change.discipline),
            ChangeKind::Removed if change.extra => format!("{prefix}: снята доп. пара, {}", change.discipline),
            ChangeKind::Removed => format!("{prefix}: снята, {}", change.discipline),
            ChangeKind::Modified => {
                let details = change.fields.iter().map(|field| {
                    format!("{}: {} → {}", field.field, field.before, field.after)
                }).collect::<Vec<_>>().join("; ");
                format!("{prefix}: {details}")
            }
        }
    }).collect::<Vec<_>>();
    Some(NotificationEvent {
        date: date.into(),
        title: format!("{} · Расписание изменено", human_date(parsed)),
        body: lines.join("\n"),
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct DateMarker {
    pub date: String,
    pub available: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_day() -> Day {
        serde_json::from_str::<OpecDaysResponse>(include_str!("../../../fixtures/opec-day.json"))
            .unwrap()
            .days
            .remove("02.11.2026")
            .unwrap()
    }

    #[test]
    fn parses_real_opec_response() {
        let day = sample_day();
        let view = project_day(&day, &["БП2-113".into()], &[]);
        assert_eq!(view.groups[0].lessons.len(), 2);
        assert_eq!(view.groups[0].lessons[1].subgroups[0].room, "547");
        assert_eq!(view.groups[0].status.as_deref(), Some("учеб.пр.+теорет.обуч."));
    }

    #[test]
    fn parses_real_omaep_response() {
        let day: Day = serde_json::from_str(include_str!("../../../fixtures/omaep-day.json")).unwrap();
        assert_eq!(day.groups.len(), 91);
        assert!(day.groups.iter().flat_map(|group| &group.lessons).flat_map(|lesson| &lesson.subgroups).any(|subgroup| subgroup.lesson_type.is_some()));
    }

    #[test]
    fn available_ignores_service_fields_and_detects_hashes() {
        let old = parse_available(r#"{"02.11.2026":"a","highlighted":["03.11.2026"]}"#).unwrap();
        let new = parse_available(r#"{"02.11.2026":"b","03.11.2026":"c","archived":[]}"#).unwrap();
        let changes = available_changes(&old, &new, parse_date("01.11.2026").unwrap());
        assert_eq!(changes.new_dates, ["03.11.2026"]);
        assert_eq!(changes.changed_dates, ["02.11.2026"]);
        assert!(!new.contains_key("highlighted"));
    }

    #[test]
    fn applies_overlay_extra_homework_and_hidden() {
        let mut day = sample_day();
        day.ov.overlay.insert("БП2-113".into(), BTreeMap::from([(
            "4".into(),
            BTreeMap::from([("0".into(), OverlayPatch {
                discipline: None,
                teacher: None,
                room: Some("б.а.3".into()),
                by_role: Some("headman".into()),
            })]),
        )]));
        day.ov.extra.insert("БП2-113".into(), BTreeMap::from([("5".into(), ExtraLesson {
            discipline: "Консультация".into(),
            teacher: "Рудакова О.И.".into(),
            room: "547".into(),
            ts: None,
        })]));
        day.ov.hw.insert("БП2-113".into(), BTreeMap::from([("4".into(), BTreeMap::from([("0".into(), Homework {
            text: "Повторить тему".into(),
            by_role: "headman".into(),
            ts: None,
        })]))]));
        let view = project_day(&day, &["БП2-113".into()], &[]);
        assert_eq!(view.groups[0].lessons[1].subgroups[0].room, "без ауд.");
        assert!(view.groups[0].lessons[1].subgroups[0].changed);
        assert!(view.groups[0].lessons[2].subgroups[0].extra);
        assert_eq!(view.groups[0].lessons[1].homework[0].text, "Повторить тему");

        day.ov.hidden.push("БП2-113".into());
        let hidden = project_day(&day, &["БП2-113".into()], &[]);
        assert!(hidden.groups[0].hidden);
        assert!(hidden.groups[0].lessons.is_empty());
    }

    #[test]
    fn teacher_filter_keeps_only_matching_subgroups() {
        let mut day = sample_day();
        day.groups[0].lessons[1].subgroups.push(Subgroup {
            pg: 1,
            discipline: "Другая дисциплина".into(),
            teacher: "Мальцева Т.А.".into(),
            room: "101".into(),
            changed: false,
            lesson_type: None,
            extra: false,
        });
        let view = project_day(&day, &[], &["Рудакова О.И.".into()]);
        assert_eq!(view.groups[0].lessons.len(), 1);
        assert_eq!(view.groups[0].lessons[0].subgroups.len(), 1);
        assert_eq!(view.groups[0].lessons[0].subgroups[0].teacher, "Рудакова О.И.");
    }

    #[test]
    fn diff_reports_fields_added_and_removed() {
        let day = sample_day();
        let before = project_day(&day, &["БП2-113".into()], &[]);
        let mut changed = day.clone();
        changed.groups[0].lessons[1].subgroups[0].room = "548".into();
        changed.groups[0].lessons.remove(0);
        let after = project_day(&changed, &["БП2-113".into()], &[]);
        let changes = diff_days(&before, &after);
        assert!(changes.iter().any(|change| change.kind == ChangeKind::Removed));
        assert!(changes.iter().any(|change| change.fields.iter().any(|field| field.field == "аудитория" && field.after == "548")));
    }

    #[test]
    fn extra_lesson_on_occupied_pair_is_a_separate_change() {
        let day = sample_day();
        let before = project_day(&day, &["БП2-113".into()], &[]);
        let mut changed = day.clone();
        changed.ov.extra.insert("БП2-113".into(), BTreeMap::from([("4".into(), ExtraLesson {
            discipline: "Консультация".into(),
            teacher: "Рудакова О.И.".into(),
            room: "547".into(),
            ts: None,
        })]));
        let after = project_day(&changed, &["БП2-113".into()], &[]);
        let changes = diff_days(&before, &after);
        let added = changes
            .iter()
            .find(|change| change.kind == ChangeKind::Added)
            .expect("доп. пара должна считаться добавленной");
        assert!(added.extra);
        assert_eq!(added.key.pair, 4);
        assert!(!changes.iter().any(|change| change.kind == ChangeKind::Removed));
    }

    #[test]
    fn normalizes_all_documented_room_variants() {
        for room in ["без ауд.", "без аудит", "без.а.", "БЕЗ АУД", "б.а.", "б.а.3", "Б.А.1"] {
            assert_eq!(normalize_room(room), "без ауд.");
        }
        assert_eq!(normalize_room("ДИС"), "ДИС");
    }
}
