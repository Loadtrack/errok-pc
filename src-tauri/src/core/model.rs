use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum College {
    #[default]
    Opec,
    Omaep,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
    #[default]
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub college: College,
    pub groups: Vec<String>,
    pub teachers: Vec<String>,
    pub theme: Theme,
    pub autostart: bool,
    pub onboarded: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            college: College::Opec,
            groups: vec![],
            teachers: vec![],
            theme: Theme::System,
            autostart: false,
            onboarded: false,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if self.groups.len() > 8 || self.teachers.len() > 8 {
            return Err("Можно выбрать до 8 групп и до 8 преподавателей".into());
        }
        if self.onboarded && self.groups.is_empty() && self.teachers.is_empty() {
            return Err("Выберите хотя бы одну группу или преподавателя".into());
        }
        if self.college == College::Omaep && !self.teachers.is_empty() {
            return Err("Для ОмАЭиП доступен выбор групп".into());
        }
        Ok(())
    }

    pub fn selection_key(&self) -> String {
        let mut groups = self.groups.clone();
        let mut teachers = self.teachers.clone();
        groups.sort();
        teachers.sort();
        format!("{}|{}", groups.join("\u{1}"), teachers.join("\u{1}"))
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Catalog {
    pub groups: Vec<String>,
    pub teachers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CollegeCache {
    pub available: BTreeMap<String, String>,
    pub available_etag: Option<String>,
    pub available_updated_at: Option<DateTime<Utc>>,
    pub days: BTreeMap<String, Day>,
    pub day_etags: BTreeMap<String, String>,
    pub day_fetched_at: BTreeMap<String, DateTime<Utc>>,
    pub last_updated: Option<DateTime<Utc>>,
    pub last_fav_at: Option<DateTime<Utc>>,
    pub initialized: bool,
    pub selection_key: String,
    pub catalog: Option<Catalog>,
    pub catalog_updated_at: Option<DateTime<Utc>>,
}

impl Default for CollegeCache {
    fn default() -> Self {
        Self {
            available: BTreeMap::new(),
            available_etag: None,
            available_updated_at: None,
            days: BTreeMap::new(),
            day_etags: BTreeMap::new(),
            day_fetched_at: BTreeMap::new(),
            last_updated: None,
            last_fav_at: None,
            initialized: false,
            selection_key: String::new(),
            catalog: None,
            catalog_updated_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PersistentData {
    pub version: u32,
    pub settings: Settings,
    pub opec: CollegeCache,
    pub omaep: CollegeCache,
    pub recent_requests: Vec<DateTime<Utc>>,
    pub dismissed_notices: Vec<String>,
}

impl Default for PersistentData {
    fn default() -> Self {
        Self {
            version: 1,
            settings: Settings::default(),
            opec: CollegeCache::default(),
            omaep: CollegeCache::default(),
            recent_requests: vec![],
            dismissed_notices: vec![],
        }
    }
}

impl PersistentData {
    pub fn cache(&self, college: College) -> &CollegeCache {
        match college {
            College::Opec => &self.opec,
            College::Omaep => &self.omaep,
        }
    }

    pub fn cache_mut(&mut self, college: College) -> &mut CollegeCache {
        match college {
            College::Opec => &mut self.opec,
            College::Omaep => &mut self.omaep,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct OpecDaysResponse {
    pub ok: bool,
    pub days: BTreeMap<String, Day>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Day {
    pub date: String,
    pub week: u8,
    pub day: String,
    pub groups: Vec<GroupDay>,
    pub gym: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    pub total: u32,
    pub ov: Overlay,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct GroupDay {
    pub name: String,
    pub status: Option<String>,
    pub lessons: Vec<Lesson>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Lesson {
    pub pair: u8,
    pub subgroups: Vec<Subgroup>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Subgroup {
    pub pg: u8,
    pub discipline: String,
    pub teacher: String,
    pub room: String,
    pub changed: bool,
    #[serde(rename = "type")]
    pub lesson_type: Option<String>,
    pub extra: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Overlay {
    pub overlay: BTreeMap<String, BTreeMap<String, BTreeMap<String, OverlayPatch>>>,
    pub extra: BTreeMap<String, BTreeMap<String, ExtraLesson>>,
    pub announcements: BTreeMap<String, Vec<Announcement>>,
    pub hw: BTreeMap<String, BTreeMap<String, BTreeMap<String, Homework>>>,
    pub hidden: Vec<String>,
    pub diffs: Vec<serde_json::Value>,
    pub statuses: BTreeMap<String, String>,
    pub bundle_ver: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct OverlayPatch {
    pub discipline: Option<String>,
    pub teacher: Option<String>,
    pub room: Option<String>,
    pub by_role: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ExtraLesson {
    pub discipline: String,
    pub teacher: String,
    pub room: String,
    pub ts: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Announcement {
    pub text: String,
    pub by_role: String,
    pub pair: u8,
    pub ts: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Homework {
    pub text: String,
    pub by_role: String,
    pub ts: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DayView {
    pub date: String,
    pub week: u8,
    pub day: String,
    pub groups: Vec<GroupView>,
    pub announcements: Vec<AnnouncementView>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct GroupView {
    pub name: String,
    pub status: Option<String>,
    pub hidden: bool,
    pub lessons: Vec<LessonView>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LessonView {
    pub pair: u8,
    pub subgroups: Vec<Subgroup>,
    pub homework: Vec<HomeworkEntry>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct HomeworkEntry {
    pub pg: u8,
    pub text: String,
    pub by_role: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnnouncementView {
    pub group: String,
    pub text: String,
    pub pair: u8,
    pub by_role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationEvent {
    pub date: String,
    pub title: String,
    pub body: String,
}
