use crate::api::{ApiClient, ApiError, Fetch};
use crate::core::*;
use crate::storage::Storage;
use chrono::{DateTime, Duration, Local, NaiveDate, Utc};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt;

static EXITING: AtomicBool = AtomicBool::new(false);

#[derive(Debug)]
struct PollState {
    next_available: DateTime<Utc>,
    next_fav: DateTime<Utc>,
    retry_at: Option<DateTime<Utc>>,
    failures: u32,
    consecutive_timeouts: u32,
    message: Option<String>,
}

impl Default for PollState {
    fn default() -> Self {
        Self {
            next_available: Utc::now(),
            next_fav: Utc::now(),
            retry_at: None,
            failures: 0,
            consecutive_timeouts: 0,
            message: None,
        }
    }
}

pub struct AppState {
    data: Mutex<PersistentData>,
    poll: Mutex<PollState>,
    storage: Storage,
    client: ApiClient,
    network: tokio::sync::Mutex<()>,
}

impl AppState {
    fn new(storage: Storage, client: ApiClient) -> Self {
        let data = storage.load();
        Self {
            data: Mutex::new(data),
            poll: Mutex::new(PollState::default()),
            storage,
            client,
            network: tokio::sync::Mutex::new(()),
        }
    }

    fn save(&self, data: &PersistentData) -> Result<(), String> {
        self.storage.save(data)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AppSnapshot {
    pub settings: Settings,
    pub days: Vec<DateMarker>,
    pub last_updated: Option<DateTime<Utc>>,
    pub message: Option<String>,
    pub retry_at: Option<DateTime<Utc>>,
}

fn snapshot(state: &AppState) -> AppSnapshot {
    let data = state.data.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let poll = state.poll.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let settings = data.settings.clone();
    let cache = data.cache(settings.college);
    let today = Local::now().naive_local().date();
    let mut dates = BTreeSet::new();
    for offset in 0..7 {
        dates.insert(today + Duration::days(offset));
    }
    for date in cache.available.keys().filter_map(|date| parse_date(date)) {
        if date >= today {
            dates.insert(date);
        }
    }
    let days = dates
        .into_iter()
        .map(|date| DateMarker {
            date: format_date(date),
            available: cache.available.contains_key(&format_date(date)),
        })
        .collect();
    AppSnapshot {
        settings,
        days,
        last_updated: cache.last_updated,
        message: poll.message.clone(),
        retry_at: poll.retry_at,
    }
}

fn save_data(state: &AppState) -> Result<(), String> {
    let data = state.data.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    state.save(&data)
}

fn reserve_request(state: &AppState, now: DateTime<Utc>, background: bool) -> bool {
    let mut data = state.data.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut log: VecDeque<_> = data.recent_requests.iter().copied().collect();
    let allowed = if background {
        reserve_background_request(&mut log, now)
    } else {
        let cutoff = now - Duration::seconds(BACKGROUND_WINDOW_SECONDS);
        while log.front().is_some_and(|time| *time < cutoff) {
            log.pop_front();
        }
        log.push_back(now);
        true
    };
    data.recent_requests = log.into_iter().collect();
    allowed
}

fn record_success(state: &AppState) {
    let mut poll = state.poll.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    poll.failures = 0;
    poll.consecutive_timeouts = 0;
    poll.retry_at = None;
    poll.message = None;
}

fn record_failure(state: &AppState, now: DateTime<Utc>, error: &ApiError) {
    let mut poll = state.poll.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    poll.failures = poll.failures.saturating_add(1);
    if error.is_timeout() {
        poll.consecutive_timeouts = poll.consecutive_timeouts.saturating_add(1);
    } else {
        poll.consecutive_timeouts = 0;
    }
    let mut delay = backoff_delay(poll.failures, poll.consecutive_timeouts);
    if let Some(seconds) = error.retry_after() {
        delay = delay.max(Duration::seconds(seconds as i64));
    }
    poll.retry_at = Some(now + delay);
    poll.next_available = now + delay;
    poll.message = Some(error.to_string());
}

fn emit_snapshot(app: &AppHandle) {
    let state = app.state::<AppState>();
    let _ = app.emit("state-changed", snapshot(&state));
}

fn emit_notifications(app: &AppHandle, notifications: Vec<NotificationEvent>) {
    if !notifications.is_empty() {
        let _ = app.emit("schedule-notifications", notifications);
    }
}

struct FreshAvailable {
    dates: BTreeMap<String, String>,
    etag: Option<String>,
    changes: AvailableChanges,
}

#[allow(clippy::too_many_arguments)]
fn apply_opec_data(
    state: &AppState,
    fresh: Option<FreshAvailable>,
    responses: Vec<OpecDaysResponse>,
    now: DateTime<Utc>,
    today: NaiveDate,
    fav_refreshed: bool,
) -> Result<Vec<NotificationEvent>, String> {
    let mut data = state.data.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let settings = data.settings.clone();
    let cache = &mut data.opec;
    let initialized = cache.initialized;
    let new_dates: BTreeSet<_> = fresh
        .as_ref()
        .map(|fresh| fresh.changes.new_dates.iter().cloned().collect())
        .unwrap_or_default();
    let mut notifications = vec![];

    for response in responses {
        for (date, day) in response.days {
            let Some(parsed) = parse_date(&date) else { continue };
            if parsed < today {
                cache.days.insert(date, day);
                continue;
            }
            let old = cache.days.get(&date).cloned();
            let view = project_day(&day, &settings.groups, &settings.teachers);
            if initialized && new_dates.contains(&date) {
                if let Some(event) = new_day_notification(&date, &view, College::Opec) {
                    notifications.push(event);
                }
            } else if initialized {
                if let Some(old) = old {
                    let previous = project_day(&old, &settings.groups, &settings.teachers);
                    let changes = diff_days(&previous, &view);
                    if let Some(event) = changes_notification(&date, &changes) {
                        notifications.push(event);
                    }
                }
            }
            cache.days.insert(date, day);
        }
    }

    if let Some(fresh) = fresh {
        cache.available = fresh.dates;
        if fresh.etag.is_some() {
            cache.available_etag = fresh.etag;
        }
    }
    cache.available_updated_at = Some(now);
    cache.last_updated = Some(now);
    cache.initialized = true;
    if fav_refreshed {
        cache.last_fav_at = Some(now);
        state.poll.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).next_fav = now + Duration::seconds(FAV_REFRESH_SECONDS);
    }
    state.save(&data)?;
    Ok(notifications)
}

fn apply_omaep_data(
    state: &AppState,
    fresh: Option<FreshAvailable>,
    responses: Vec<(String, Fetch<Day>)>,
    now: DateTime<Utc>,
    today: NaiveDate,
) -> Result<Vec<NotificationEvent>, String> {
    let mut data = state.data.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let settings = data.settings.clone();
    let cache = &mut data.omaep;
    let initialized = cache.initialized;
    let new_dates: BTreeSet<_> = fresh
        .as_ref()
        .map(|fresh| fresh.changes.new_dates.iter().cloned().collect())
        .unwrap_or_default();
    let mut notifications = vec![];

    for (date, response) in responses {
        cache.day_fetched_at.insert(date.clone(), now);
        let Fetch::Fresh { data: day, etag } = response else { continue };
        if let Some(etag) = etag {
            cache.day_etags.insert(date.clone(), etag);
        }
        let old = cache.days.get(&date).cloned();
        let view = project_day(&day, &settings.groups, &settings.teachers);
        if initialized && parse_date(&date).is_some_and(|date| date >= today) {
            if new_dates.contains(&date) {
                if let Some(event) = new_day_notification(&date, &view, College::Omaep) {
                    notifications.push(event);
                }
            } else if let Some(old) = old {
                let previous = project_day(&old, &settings.groups, &settings.teachers);
                let changes = diff_days(&previous, &view);
                if let Some(event) = changes_notification(&date, &changes) {
                    notifications.push(event);
                }
            }
        }
        cache.days.insert(date, day);
    }

    if let Some(fresh) = fresh {
        cache.available = fresh.dates;
        if fresh.etag.is_some() {
            cache.available_etag = fresh.etag;
        }
    }
    cache.available_updated_at = Some(now);
    cache.last_updated = Some(now);
    cache.initialized = true;
    state.save(&data)?;
    Ok(notifications)
}

fn available_is_due(state: &AppState, now: DateTime<Utc>) -> bool {
    let data = state.data.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let cache = data.cache(data.settings.college);
    let minimum_passed = cache
        .available_updated_at
        .map_or(true, |last| now - last >= Duration::seconds(AVAILABLE_MIN_INTERVAL_SECONDS));
    let poll = state.poll.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    minimum_passed && now >= poll.next_available
}

fn retry_is_pending(state: &AppState, now: DateTime<Utc>) -> bool {
    state
        .poll
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .retry_at
        .is_some_and(|retry| now < retry)
}

async fn refresh(app: &AppHandle, background: bool) -> Result<AppSnapshot, String> {
    let state = app.state::<AppState>();
    let _guard = state.network.lock().await;
    let now = Utc::now();
    let today = Local::now().naive_local().date();
    let settings = state.data.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).settings.clone();
    if !settings.onboarded || retry_is_pending(&state, now) {
        return Ok(snapshot(&state));
    }

    let mut fresh: Option<FreshAvailable> = None;
    let mut available_attempted = false;
    if available_is_due(&state, now) {
        if !reserve_request(&state, now, background) {
            return Ok(snapshot(&state));
        }
        available_attempted = true;
        let etag = state
            .data
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .cache(settings.college)
            .available_etag
            .clone();
        match state.client.available(settings.college, etag.as_deref()).await {
            Ok(Fetch::NotModified) => {
                let mut data = state.data.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                let cache = data.cache_mut(settings.college);
                cache.available_updated_at = Some(now);
                cache.last_updated = Some(now);
                state.save(&data)?;
                record_success(&state);
            }
            Ok(Fetch::Fresh { data: dates, etag }) => {
                let old = state
                    .data
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .cache(settings.college)
                    .available
                    .clone();
                fresh = Some(FreshAvailable {
                    changes: available_changes(&old, &dates, today),
                    dates,
                    etag,
                });
                record_success(&state);
            }
            Err(error) => {
                record_failure(&state, now, &error);
                save_data(&state)?;
                return Ok(snapshot(&state));
            }
        }
        state.poll.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).next_available = now + next_available_delay();
    }

    let initialized = state.data.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).cache(settings.college).initialized;
    let mut notifications = vec![];

    match settings.college {
        College::Opec => {
            let mut target_dates = if initialized {
                fresh.as_ref().map(|fresh| {
                    fresh.changes.new_dates.iter()
                        .chain(fresh.changes.changed_dates.iter())
                        .filter_map(|date| parse_date(date))
                        .collect::<Vec<_>>()
                }).unwrap_or_default()
            } else {
                vec![today, today + Duration::days(1)]
            };
            target_dates.sort();
            target_dates.dedup();
            let fav_due = state.poll.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).next_fav <= now;
            let should_refresh_fav = target_dates.is_empty() && (!background || fav_due || !initialized);
            if should_refresh_fav {
                target_dates = vec![today, today + Duration::days(1)];
            }
            let fav_refreshed = target_dates.contains(&today);
            let ranges = ranges_for_dates(target_dates);
            let mut responses = vec![];
            for range in ranges {
                if !reserve_request(&state, Utc::now(), background) {
                    if available_attempted {
                        return Ok(snapshot(&state));
                    }
                    break;
                }
                match state.client.opec_days(range.from, range.to, &settings.groups, &settings.teachers).await {
                    Ok(response) => responses.push(response),
                    Err(error) => {
                        record_failure(&state, Utc::now(), &error);
                        save_data(&state)?;
                        return Ok(snapshot(&state));
                    }
                }
            }
            if !responses.is_empty() || fresh.is_some() {
                notifications = apply_opec_data(&state, fresh, responses, now, today, fav_refreshed)?;
            }
        }
        College::Omaep => {
            let mut targets = BTreeSet::new();
            if initialized {
                if let Some(fresh) = &fresh {
                    for date in &fresh.changes.new_dates {
                        if let Some(date) = parse_date(date) {
                            targets.insert(date);
                        }
                    }
                }
            }
            let data = state.data.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            for date in [today, today + Duration::days(1)] {
                let key = format_date(date);
                if !data.omaep.available.contains_key(&key)
                    && !fresh.as_ref().is_some_and(|fresh| fresh.dates.contains_key(&key))
                {
                    continue;
                }
                let due = !initialized
                    || data.omaep.day_fetched_at.get(&key).map_or(true, |last| {
                        now - *last >= Duration::seconds(OMAEP_DAY_MIN_INTERVAL_SECONDS)
                    });
                if due {
                    targets.insert(date);
                }
            }
            drop(data);
            let mut responses = vec![];
            for date in targets {
                let key = format_date(date);
                if !reserve_request(&state, Utc::now(), background) {
                    return Ok(snapshot(&state));
                }
                let etag = state.data.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).omaep.day_etags.get(&key).cloned();
                match state.client.omaep_day(date, etag.as_deref()).await {
                    Ok(response) => responses.push((key, response)),
                    Err(error) => {
                        record_failure(&state, Utc::now(), &error);
                        save_data(&state)?;
                        return Ok(snapshot(&state));
                    }
                }
            }
            if !responses.is_empty() || fresh.is_some() || !initialized {
                notifications = apply_omaep_data(&state, fresh, responses, now, today)?;
            }
        }
    }

    record_success(&state);
    save_data(&state)?;
    emit_notifications(app, notifications);
    Ok(snapshot(&state))
}

#[tauri::command]
fn get_snapshot(state: State<'_, AppState>) -> AppSnapshot {
    snapshot(&state)
}

#[tauri::command]
async fn fetch_catalog(app: AppHandle, college: College) -> Result<Catalog, String> {
    let state = app.state::<AppState>();
    let _guard = state.network.lock().await;
    let cached = {
        let data = state.data.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let cache = data.cache(college);
        match (&cache.catalog, cache.catalog_updated_at) {
            (Some(catalog), Some(updated)) if Utc::now() - updated < Duration::hours(24) => Some(catalog.clone()),
            _ => None,
        }
    };
    if let Some(catalog) = cached.clone() {
        return Ok(catalog);
    }
    if retry_is_pending(&state, Utc::now()) {
        return cached.ok_or_else(|| "Сервер просит повторить запрос позже".into());
    }
    reserve_request(&state, Utc::now(), false);
    match state.client.catalog(college).await {
        Ok(catalog) => {
            let mut data = state.data.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            let cache = data.cache_mut(college);
            cache.catalog = Some(catalog.clone());
            cache.catalog_updated_at = Some(Utc::now());
            state.save(&data)?;
            record_success(&state);
            Ok(catalog)
        }
        Err(error) => {
            record_failure(&state, Utc::now(), &error);
            let cached = state.data.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).cache(college).catalog.clone();
            cached.ok_or_else(|| error.to_string())
        }
    }
}

#[tauri::command]
async fn save_settings(app: AppHandle, settings: Settings) -> Result<AppSnapshot, String> {
    settings.validate()?;
    let state = app.state::<AppState>();
    let _guard = state.network.lock().await;
    if settings.autostart {
        app.autolaunch().enable().map_err(|error| error.to_string())?;
    } else {
        app.autolaunch().disable().map_err(|error| error.to_string())?;
    }
    let mut data = state.data.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let selection_key = settings.selection_key();
    let cache = data.cache_mut(settings.college);
    if cache.selection_key != selection_key {
        cache.days.clear();
        cache.day_etags.clear();
        cache.day_fetched_at.clear();
        cache.initialized = false;
        cache.selection_key = selection_key;
        cache.last_fav_at = None;
        state.poll.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).next_fav = Utc::now();
    }
    data.settings = settings;
    state.save(&data)?;
    drop(data);
    Ok(snapshot(&state))
}

#[tauri::command]
fn get_cached_day(state: State<'_, AppState>, date: String) -> Result<Option<DayView>, String> {
    let data = state.data.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let settings = &data.settings;
    let day = data.cache(settings.college).days.get(&date);
    Ok(day.map(|day| project_day(day, &settings.groups, &settings.teachers)))
}

#[tauri::command]
async fn load_day(app: AppHandle, date: String) -> Result<Option<DayView>, String> {
    let parsed = parse_date(&date).ok_or("Некорректная дата")?;
    let state = app.state::<AppState>();
    let _guard = state.network.lock().await;
    let now = Utc::now();
    if retry_is_pending(&state, now) {
        return get_cached_day(state, date);
    }
    let settings = state.data.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).settings.clone();
    if !settings.onboarded {
        return Ok(None);
    }
    reserve_request(&state, now, false);
    match settings.college {
        College::Opec => {
            let response = state.client.opec_days(parsed, parsed, &settings.groups, &settings.teachers).await;
            match response {
                Ok(response) => {
                    let Some(day) = response.days.get(&date).cloned() else { return Ok(None) };
                    let mut data = state.data.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                    let old = data.opec.days.get(&date).cloned();
                    let view = project_day(&day, &settings.groups, &settings.teachers);
                    let notification = if data.opec.initialized && parsed >= Local::now().naive_local().date() {
                        old.and_then(|old| {
                            let previous = project_day(&old, &settings.groups, &settings.teachers);
                            changes_notification(&date, &diff_days(&previous, &view))
                        })
                    } else {
                        None
                    };
                    data.opec.days.insert(date, day);
                    data.opec.last_updated = Some(now);
                    state.save(&data)?;
                    drop(data);
                    record_success(&state);
                    if let Some(notification) = notification {
                        emit_notifications(&app, vec![notification]);
                    }
                    Ok(Some(view))
                }
                Err(error) => {
                    record_failure(&state, now, &error);
                    Err(error.to_string())
                }
            }
        }
        College::Omaep => {
            {
                let data = state.data.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                if let Some(day) = data.omaep.days.get(&date) {
                    let recent = data.omaep.day_fetched_at.get(&date).is_some_and(|last| {
                        now - *last < Duration::seconds(OMAEP_DAY_MIN_INTERVAL_SECONDS)
                    });
                    if recent || !data.omaep.available.contains_key(&date) {
                        return Ok(Some(project_day(day, &settings.groups, &settings.teachers)));
                    }
                } else if !data.omaep.available.contains_key(&date) {
                    return Ok(None);
                }
            }
            let etag = state.data.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).omaep.day_etags.get(&date).cloned();
            match state.client.omaep_day(parsed, etag.as_deref()).await {
                Ok(Fetch::NotModified) => {
                    record_success(&state);
                    get_cached_day(state, date)
                }
                Ok(Fetch::Fresh { data: day, etag }) => {
                    let mut data = state.data.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                    let old = data.omaep.days.get(&date).cloned();
                    let view = project_day(&day, &settings.groups, &settings.teachers);
                    let notification = if data.omaep.initialized && parsed >= Local::now().naive_local().date() {
                        old.and_then(|old| {
                            let previous = project_day(&old, &settings.groups, &settings.teachers);
                            changes_notification(&date, &diff_days(&previous, &view))
                        })
                    } else {
                        None
                    };
                    if let Some(etag) = etag {
                        data.omaep.day_etags.insert(date.clone(), etag);
                    }
                    data.omaep.day_fetched_at.insert(date.clone(), now);
                    data.omaep.days.insert(date, day);
                    data.omaep.last_updated = Some(now);
                    state.save(&data)?;
                    drop(data);
                    record_success(&state);
                    if let Some(notification) = notification {
                        emit_notifications(&app, vec![notification]);
                    }
                    Ok(Some(view))
                }
                Err(error) => {
                    record_failure(&state, now, &error);
                    Err(error.to_string())
                }
            }
        }
    }
}

#[tauri::command]
async fn refresh_now(app: AppHandle) -> Result<AppSnapshot, String> {
    let result = refresh(&app, false).await;
    emit_snapshot(&app);
    result
}

#[tauri::command]
fn reveal_window(app: AppHandle) -> Result<(), String> {
    show_window(&app)
}

fn show_window(app: &AppHandle) -> Result<(), String> {
    let window = app.get_webview_window("main").ok_or("Окно не найдено")?;
    window.show().map_err(|error| error.to_string())?;
    window.unminimize().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
fn open_on_site(app: AppHandle, date: String) -> Result<(), String> {
    parse_date(&date).ok_or("Некорректная дата")?;
    let state = app.state::<AppState>();
    let settings = state.data.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).settings.clone();
    let target = if let Some(group) = settings.groups.first() {
        urlencoding::encode(group).into_owned()
    } else if let Some(teacher) = settings.teachers.first() {
        format!("t:{}", urlencoding::encode(teacher))
    } else {
        return Err("Сначала выберите группу или преподавателя".into());
    };
    let url = format!("https://errok.ru/#{date}/{target}");
    tauri_plugin_opener::open_url(url, None::<&str>).map_err(|error| error.to_string())
}

pub fn run() {
    let client = ApiClient::new().unwrap_or_else(|error| panic!("Не удалось создать HTTP-клиент: {error}"));
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .setup(move |app| {
            let directory = app.path().app_data_dir()?;
            app.manage(AppState::new(Storage::new(directory), client.clone()));

            let open = tauri::menu::MenuItem::with_id(app, "open", "Открыть", true, None::<&str>)?;
            let check = tauri::menu::MenuItem::with_id(app, "check", "Проверить сейчас", true, None::<&str>)?;
            let quit = tauri::menu::MenuItem::with_id(app, "quit", "Выход", true, None::<&str>)?;
            let menu = tauri::menu::Menu::with_items(app, &[&open, &check, &quit])?;
            let mut tray = tauri::tray::TrayIconBuilder::with_id("main")
                .menu(&menu)
                .tooltip("errok для ПК")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => {
                        let _ = show_window(app);
                    }
                    "check" => {
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let _ = refresh(&app, false).await;
                            emit_snapshot(&app);
                        });
                    }
                    "quit" => {
                        EXITING.store(true, Ordering::SeqCst);
                        app.exit(0);
                    }
                    _ => {}
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    if handle.state::<AppState>().data.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).settings.onboarded {
                        let _ = refresh(&handle, true).await;
                        emit_snapshot(&handle);
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if !EXITING.load(Ordering::SeqCst) {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            fetch_catalog,
            save_settings,
            get_cached_day,
            load_day,
            refresh_now,
            reveal_window,
            open_on_site,
        ])
        .run(tauri::generate_context!())
        .expect("Ошибка запуска errok для ПК");
}
