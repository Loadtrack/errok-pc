import '@fontsource/mulish/cyrillic-400.css';
import '@fontsource/mulish/cyrillic-500.css';
import '@fontsource/mulish/cyrillic-600.css';
import '@fontsource/mulish/cyrillic-700.css';
import '@fontsource/mulish/cyrillic-800.css';
import '@fontsource/mulish/latin-400.css';
import '@fontsource/mulish/latin-500.css';
import '@fontsource/mulish/latin-600.css';
import '@fontsource/mulish/latin-700.css';
import '@fontsource/mulish/latin-800.css';
import '@fontsource/unbounded/cyrillic-600.css';
import '@fontsource/unbounded/cyrillic-700.css';
import '@fontsource/unbounded/cyrillic-800.css';
import '@fontsource/unbounded/latin-600.css';
import '@fontsource/unbounded/latin-700.css';
import '@fontsource/unbounded/latin-800.css';
import './styles.css';
import { listen } from '@tauri-apps/api/event';
import { isPermissionGranted, onAction, requestPermission, sendNotification } from '@tauri-apps/plugin-notification';
import { backend, isDesktop } from './bridge';
import { dayNumber, dayRelation, formatRetry, formatUpdated, fullDate, lessonTimes, today, weekdayShort } from './dates';
import type { AppSnapshot, Catalog, College, DayView, GroupView, LessonView, NotificationEvent, Settings, Theme } from './types';

const app = document.querySelector<HTMLDivElement>('#app')!;
const toastElement = document.querySelector<HTMLDivElement>('#toast')!;
let snapshot: AppSnapshot | null = null;
let selectedDate = today();
let currentDay: DayView | null = null;
let dayRequest = 0;
let refreshBusy = false;
let toastTimer: ReturnType<typeof setTimeout>;
let eventsInitialized = false;
let settingsSaved = false;
let lastUpdatedKey: string | null = null;
let pendingDate: string | null = null;
let notificationErrorShown = false;
const notificationDates = new Map<number, string>();
const NOTIFICATION_DATES_KEY = 'errok.notification-dates';
const NOTIFICATION_ASKED_KEY = 'errok.notification-asked';

function loadNotificationDates(): void {
  try {
    const stored = JSON.parse(localStorage.getItem(NOTIFICATION_DATES_KEY) ?? '{}') as Record<string, unknown>;
    for (const [id, date] of Object.entries(stored)) {
      const numeric = Number(id);
      if (Number.isInteger(numeric) && typeof date === 'string' && /^\d{2}\.\d{2}\.\d{4}$/.test(date)) {
        notificationDates.set(numeric, date);
      }
    }
  } catch {
    // Хранилище недоступно: уведомления всё равно работают в текущем сеансе.
  }
}

function rememberNotification(id: number, date: string): void {
  notificationDates.set(id, date);
  try {
    localStorage.setItem(NOTIFICATION_DATES_KEY, JSON.stringify(Object.fromEntries(notificationDates)));
  } catch {
    // Если записать не удалось, нажатие сработает до перезапуска программы.
  }
}

function notificationsWereAsked(): boolean {
  try {
    return localStorage.getItem(NOTIFICATION_ASKED_KEY) === '1';
  } catch {
    return false;
  }
}

function markNotificationsAsked(): void {
  try {
    localStorage.setItem(NOTIFICATION_ASKED_KEY, '1');
  } catch {
    // Ничего страшного: спросим разрешение при следующем запуске.
  }
}

const iconPaths = {
  settings: '<circle cx="12" cy="12" r="3"></circle><path d="M19.4 15a1.7 1.7 0 0 0 .34 1.88l.06.06-2.83 2.83-.06-.06a1.7 1.7 0 0 0-1.88-.34 1.7 1.7 0 0 0-1.03 1.56V21h-4v-.09A1.7 1.7 0 0 0 9 19.36a1.7 1.7 0 0 0-1.88.34l-.06.06-2.83-2.83.06-.06A1.7 1.7 0 0 0 4.63 15 1.7 1.7 0 0 0 3.09 14H3v-4h.09A1.7 1.7 0 0 0 4.64 9a1.7 1.7 0 0 0-.34-1.88l-.06-.06 2.83-2.83.06.06A1.7 1.7 0 0 0 9 4.63 1.7 1.7 0 0 0 10 3.09V3h4v.09A1.7 1.7 0 0 0 15 4.64a1.7 1.7 0 0 0 1.88-.34l.06-.06 2.83 2.83-.06.06A1.7 1.7 0 0 0 19.37 9c.15.61.72 1 1.33 1H21v4h-.3c-.61 0-1.16.39-1.31 1Z"></path>',
  refresh: '<path d="M20 6v5h-5"></path><path d="M4 18v-5h5"></path><path d="M18.5 9A7 7 0 0 0 6 6.5L4 11"></path><path d="M5.5 15A7 7 0 0 0 18 17.5l2-4.5"></path>',
  external: '<path d="M14 4h6v6"></path><path d="m20 4-9 9"></path><path d="M18 13v6a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h6"></path>',
  calendar: '<rect x="3" y="5" width="18" height="16" rx="2"></rect><path d="M16 3v4M8 3v4M3 10h18"></path><path d="M8 14h.01M12 14h.01M16 14h.01M8 18h.01M12 18h.01"></path>',
  search: '<circle cx="11" cy="11" r="7"></circle><path d="m20 20-4-4"></path>',
  close: '<path d="m6 6 12 12M18 6 6 18"></path>',
};

function icon(name: keyof typeof iconPaths): string {
  return `<svg class="icon" viewBox="0 0 24 24" aria-hidden="true">${iconPaths[name]}</svg>`;
}

function brand(): string {
  return '<div class="brand"><div class="brand-mark" aria-hidden="true"><span></span><span></span><span></span></div><div class="brand-copy"><strong>errok</strong><small>для ПК</small></div></div>';
}

function escapeHtml(value: string): string {
  return value.replace(/[&<>"']/g, (character) => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
  })[character]!);
}

function discipline(value: string): string {
  return escapeHtml(value).replace(/\./g, '.<wbr>');
}

function toast(message: string): void {
  clearTimeout(toastTimer);
  toastElement.textContent = message;
  toastElement.classList.add('visible');
  toastTimer = setTimeout(() => toastElement.classList.remove('visible'), 3600);
}

interface ThemeOption {
  id: Theme;
  label: string;
  swatch: string;
  accent: string;
  pageColor: string;
}

export const THEME_OPTIONS: ThemeOption[] = [
  { id: 'system', label: 'Системная', swatch: 'linear-gradient(135deg, #0d0d0f 50%, #f4f4f8 50%)', accent: '#7c6cfc', pageColor: '#0d0d0f' },
  { id: 'dark', label: 'Тёмная', swatch: '#0d0d0f', accent: '#7c6cfc', pageColor: '#0d0d0f' },
  { id: 'light', label: 'Светлая', swatch: '#f4f4f8', accent: '#6c5ce7', pageColor: '#f4f4f8' },
  { id: 'midnight', label: 'Полночь', swatch: '#0a0e1a', accent: '#5b8cff', pageColor: '#0a0e1a' },
  { id: 'ocean', label: 'Океан', swatch: '#07131a', accent: '#22d3ee', pageColor: '#07131a' },
  { id: 'rose', label: 'Роза', swatch: '#160d12', accent: '#fb7185', pageColor: '#160d12' },
  { id: 'forest', label: 'Лес', swatch: '#0a140e', accent: '#34d399', pageColor: '#0a140e' },
  { id: 'light-ocean', label: 'Светлый океан', swatch: '#eef7fa', accent: '#0891b2', pageColor: '#eef7fa' },
  { id: 'light-rose', label: 'Светлая роза', swatch: '#fdf2f5', accent: '#e11d6f', pageColor: '#fdf2f5' },
  { id: 'light-forest', label: 'Светлый лес', swatch: '#eef8f1', accent: '#059669', pageColor: '#eef8f1' },
];

function resolveTheme(theme: Theme): Exclude<Theme, 'system'> {
  if (theme !== 'system') return theme;
  return window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
}

function applyTheme(theme: Theme): void {
  const resolved = resolveTheme(theme);
  document.documentElement.dataset.theme = resolved;
  const option = THEME_OPTIONS.find((item) => item.id === resolved);
  document.querySelector('meta[name="theme-color"]')?.setAttribute('content', option?.pageColor ?? '#0d0d0f');
}

window.matchMedia('(prefers-color-scheme: light)').addEventListener('change', () => {
  if (snapshot?.settings.theme === 'system') applyTheme('system');
});

function collegeName(college: College): string {
  return college === 'opec' ? 'ОПЭК' : 'ОмАЭиП';
}

function defaultDate(value: AppSnapshot): string {
  const current = today();
  if (value.days.some((day) => day.date === current && day.available)) return current;
  return value.days.find((day) => day.available)?.date ?? current;
}

function renderShell(): void {
  app.innerHTML = `
    <div class="app-shell">
      <header class="site-header">
        <div class="hd-inner">
          ${brand()}
          <div class="hd-context">
            <span class="hd-college" id="college-label"></span>
            <span class="hd-favorites" id="favorites-text"></span>
          </div>
          <div class="hd-right">
            <button class="btn btn-ghost" id="refresh-button" title="Проверить сейчас">${icon('refresh')}<span>Проверить</span></button>
            <button class="btn btn-icon" id="site-button" title="Открыть на сайте" aria-label="Открыть на сайте">${icon('external')}</button>
            <button class="btn btn-icon" id="settings-button" title="Настройки" aria-label="Настройки">${icon('settings')}</button>
          </div>
        </div>
      </header>
      <main class="page">
        <div id="message-slot" aria-live="polite"></div>
        <section class="sch-header">
          <div class="sch-meta">
            <h1 class="sch-date-title" id="day-title"></h1>
            <div class="sch-tags">
              <span class="tag tag-week" id="day-subtitle"></span>
              <span class="tag tag-college" id="college-tag"></span>
            </div>
          </div>
          <div class="status-stack">
            <span class="connection" id="connection">Данные с errok.ru</span>
            <span class="updated" id="updated"></span>
          </div>
        </section>
        <div class="day-switcher" id="day-nav" role="tablist" aria-label="Дни расписания"></div>
        <div class="schedule-content" id="schedule-content"></div>
      </main>
    </div>
    <dialog id="settings-dialog" aria-labelledby="settings-title">
      <div class="dialog-head"><h2 id="settings-title">Настройки</h2><button class="btn btn-icon" id="close-settings" aria-label="Закрыть">${icon('close')}</button></div>
      <div class="dialog-body" id="settings-content"></div>
    </dialog>`;
  document.querySelector('#settings-button')!.addEventListener('click', openSettings);
  document.querySelector('#refresh-button')!.addEventListener('click', refreshSchedule);
  document.querySelector('#site-button')!.addEventListener('click', () => {
    void backend.openOnSite(selectedDate).catch((error) => toast(String(error)));
  });
  document.querySelector('#close-settings')!.addEventListener('click', () => closeSettings());
  const dialog = document.querySelector<HTMLDialogElement>('#settings-dialog')!;
  dialog.addEventListener('click', (event) => {
    if (event.target === dialog) closeSettings();
  });
  dialog.addEventListener('close', () => {
    if (!settingsSaved && snapshot) applyTheme(snapshot.settings.theme);
  });
  renderChrome();
}

function renderChrome(updateDays = true): void {
  if (!snapshot || !document.querySelector('#day-nav')) return;
  const favorites = [...snapshot.settings.groups, ...snapshot.settings.teachers];
  const visible = favorites.slice(0, 3).join(' · ');
  const remainder = favorites.length > 3 ? ` · ещё ${favorites.length - 3}` : '';
  document.querySelector('#college-label')!.textContent = collegeName(snapshot.settings.college);
  document.querySelector('#favorites-text')!.textContent = favorites.length
    ? `${visible}${remainder}`
    : 'Группы и преподаватели не выбраны';
  document.querySelector('#college-tag')!.textContent = collegeName(snapshot.settings.college);
  document.querySelector('#updated')!.textContent = formatUpdated(snapshot.last_updated);
  const connection = document.querySelector('#connection')!;
  connection.textContent = snapshot.message ? 'Нет соединения' : 'Данные с errok.ru';
  connection.classList.toggle('offline', Boolean(snapshot.message));
  const retry = formatRetry(snapshot.retry_at);
  document.querySelector('#message-slot')!.innerHTML = snapshot.message
    ? `<div class="notice-bar">${escapeHtml(snapshot.message)}. Показываем последнее сохранённое расписание.${retry ? ` ${escapeHtml(retry)}.` : ''}</div>`
    : '';
  if (updateDays) renderDays();
}

function renderDays(): void {
  if (!snapshot) return;
  const nav = document.querySelector('#day-nav');
  if (!nav) return;
  const current = today();
  nav.innerHTML = snapshot.days.map((day) => {
    const active = day.date === selectedDate;
    return `
    <button class="day-tab ${active ? 'active' : ''} ${day.date === current ? 'today' : ''} ${day.available ? 'fresh' : 'unavailable'}"
      role="tab" aria-selected="${active}" data-date="${day.date}"
      aria-label="${dayRelation(day.date)}, ${weekdayShort(day.date)} ${dayNumber(day.date)}" title="${dayRelation(day.date)}">
      <span class="day-tab-name">${weekdayShort(day.date)}</span>
      <span class="day-tab-date">${dayNumber(day.date)}</span>
      <span class="day-tab-dot" aria-hidden="true"></span>
    </button>`;
  }).join('');
  nav.querySelectorAll<HTMLButtonElement>('.day-tab').forEach((button) => {
    button.addEventListener('click', () => void selectDate(button.dataset.date!));
  });
}

function renderLoading(): void {
  const content = document.querySelector('#schedule-content');
  if (content) {
    content.innerHTML = `<div class="groups-grid">
      <div class="skeleton"></div><div class="skeleton"></div><div class="skeleton"></div>
    </div>`;
  }
}

function plural(count: number, one: string, few: string, many: string): string {
  const mod10 = count % 10;
  const mod100 = count % 100;
  if (mod10 === 1 && mod100 !== 11) return one;
  if (mod10 >= 2 && mod10 <= 4 && (mod100 < 10 || mod100 >= 20)) return few;
  return many;
}

const PAIR_TYPES: Array<[RegExp, string]> = [
  [/лек/i, 'ptb-lec'],
  [/лаб/i, 'ptb-lab'],
  [/сем/i, 'ptb-sem'],
  [/конс/i, 'ptb-cons'],
  [/экз|зач[её]т/i, 'ptb-exam'],
  [/курс/i, 'ptb-kur'],
  [/практ/i, 'ptb-pr'],
];

function pairTypeClass(value: string | null): string {
  if (!value) return '';
  return PAIR_TYPES.find(([pattern]) => pattern.test(value))?.[1] ?? '';
}

function minutesOf(value: string): number | null {
  const match = /^(\d{1,2}):(\d{2})$/.exec(value);
  if (!match) return null;
  return Number(match[1]) * 60 + Number(match[2]);
}

function isLessonNow(date: string, start: string, end: string): boolean {
  if (date !== today()) return false;
  const from = minutesOf(start);
  const to = minutesOf(end);
  if (from === null || to === null) return false;
  const now = new Date();
  const current = now.getHours() * 60 + now.getMinutes();
  return current >= from && current < to;
}

function renderHomework(lesson: LessonView): string {
  if (!lesson.homework.length) return '';
  return lesson.homework
    .map((item) => `<div class="homework"><strong>ДЗ${item.pg ? ` · ${item.pg} подгруппа` : ''}</strong>${escapeHtml(item.text)}</div>`)
    .join('');
}

function renderLesson(lesson: LessonView): string {
  if (!snapshot) return '';
  const date = currentDay?.date ?? selectedDate;
  const [start, end] = lessonTimes(snapshot.settings.college, date, lesson.pair);
  const multi = lesson.subgroups.length > 1;
  const now = isLessonNow(date, start, end);
  const subgroups = lesson.subgroups.map((subgroup) => `
    <div class="sub-item">
      <div class="lesson-top">
        <div class="lesson-top-left">
          ${subgroup.lesson_type ? `<span class="pair-type ${pairTypeClass(subgroup.lesson_type)}">${escapeHtml(subgroup.lesson_type)}</span>` : ''}
          <span class="lesson-teacher">${escapeHtml(subgroup.teacher)}</span>
        </div>
        <div class="lesson-top-right">
          ${subgroup.pg ? `<span class="subgroup-badge">ПГ <b>${subgroup.pg}</b></span>` : ''}
          ${subgroup.room ? `<span class="lesson-room">${escapeHtml(subgroup.room)}</span>` : ''}
        </div>
      </div>
      <div class="lesson-discipline">${discipline(subgroup.discipline)}</div>
      ${subgroup.changed || subgroup.extra ? `<div class="lesson-tags">
        ${subgroup.changed && !subgroup.extra ? '<span class="tag-changed">изменено</span>' : ''}
        ${subgroup.extra ? '<span class="tag-changed">доп. пара</span>' : ''}
      </div>` : ''}
    </div>`).join('');
  return `
    <article class="lesson-item ${now ? 'li-now' : ''} ${multi ? 'li-multi' : ''}">
      <div class="lp-col">
        <div class="lesson-pair">${lesson.pair}</div>
        ${start ? `<div class="lesson-time">${start}<span>${end}</span></div>` : ''}
      </div>
      <div class="lesson-content ${multi ? 'multi' : ''}">
        ${subgroups}
        ${renderHomework(lesson)}
      </div>
    </article>`;
}

function renderGroup(group: GroupView): string {
  const count = group.lessons.length;
  const countLabel = plural(count, 'пара', 'пары', 'пар');
  return `
    <section class="group-card ${group.hidden ? 'is-hidden' : ''}">
      <div class="group-header">
        <div class="group-title">
          <h2 class="group-name">${escapeHtml(group.name)}</h2>
          ${group.status && !group.hidden ? `<span class="group-status">${escapeHtml(group.status)}</span>` : ''}
        </div>
        ${group.hidden ? '' : `<div class="lesson-count"><b class="gc-n">${count}</b><span class="gc-u">${countLabel}</span></div>`}
      </div>
      ${group.hidden
        ? '<div class="group-empty"><strong>Занятий нет</strong></div>'
        : count
          ? `<div class="lessons-list">${group.lessons.map(renderLesson).join('')}</div>`
          : '<div class="group-empty"><strong>Пар нет</strong><span>На выбранный день занятий нет.</span></div>'}
    </section>`;
}

function renderDay(): void {
  if (!snapshot) return;
  document.title = `${fullDate(selectedDate)} · errok для ПК`;
  document.querySelector('#day-title')!.textContent = fullDate(selectedDate);
  const available = snapshot.days.find((day) => day.date === selectedDate)?.available ?? false;
  document.querySelector('#day-subtitle')!.textContent = currentDay
    ? `${currentDay.week} неделя`
    : (available ? 'Расписание выложено' : 'Ждём публикации');
  const content = document.querySelector('#schedule-content');
  if (!content) return;
  if (!currentDay) {
    content.innerHTML = `
      <div class="empty-state">
        <div class="empty-icon">${icon('calendar')}</div>
        <h2>${available ? 'Пар нет' : 'Расписание ещё не выложено'}</h2>
        <p>${available ? 'Для выбранных групп и преподавателей занятий нет.' : 'Программа проверит сайт и сообщит, когда расписание появится.'}</p>
      </div>`;
    return;
  }
  if (!currentDay.groups.length) {
    content.innerHTML = `<div class="empty-state"><div class="empty-icon">${icon('calendar')}</div><h2>Пар нет</h2><p>Для выбранных групп и преподавателей занятий нет.</p></div>`;
    return;
  }
  const announcements = currentDay.announcements.length ? `
    <div class="ann-box">${currentDay.announcements.map((item) => `
      <div class="ann-item">
        <div class="ann-head"><span class="ann-title">Объявление</span><span class="ann-by">${escapeHtml(item.group)}${item.pair ? ` · ${item.pair} пара` : ''}</span></div>
        <div class="ann-text">${escapeHtml(item.text)}</div>
      </div>`).join('')}</div>` : '';
  content.innerHTML = announcements + `<div class="groups-grid">${currentDay.groups.map(renderGroup).join('')}</div>`;
}

async function selectDate(date: string): Promise<void> {
  selectedDate = date;
  currentDay = null;
  const request = ++dayRequest;
  document.title = `${fullDate(date)} · errok для ПК`;
  renderDays();
  if (snapshot) document.querySelector('#day-title')!.textContent = fullDate(date);
  document.querySelector('#day-subtitle')!.textContent = '';
  renderLoading();
  const cached = await backend.getCachedDay(date).catch(() => null);
  if (request !== dayRequest) return;
  if (cached) {
    currentDay = cached;
    renderDay();
  }
  try {
    const loaded = await backend.loadDay(date);
    if (request !== dayRequest) return;
    currentDay = loaded;
    renderDay();
  } catch (error) {
    if (request !== dayRequest) return;
    if (!cached) renderDay();
    toast('Не удалось обновить день. Показываем сохранённые данные.');
  }
}

async function refreshSchedule(): Promise<void> {
  if (refreshBusy) return;
  refreshBusy = true;
  const button = document.querySelector<HTMLButtonElement>('#refresh-button');
  button?.classList.add('spinning');
  button!.disabled = true;
  try {
    snapshot = await backend.refreshNow();
    lastUpdatedKey = snapshot.last_updated;
    renderChrome();
    currentDay = await backend.getCachedDay(selectedDate);
    renderDay();
  } catch (error) {
    toast(String(error));
  } finally {
    refreshBusy = false;
    button?.classList.remove('spinning');
    if (button) button.disabled = false;
  }
}

function setupPicker(root: HTMLElement, draft: Settings, catalog: Catalog | null, onChange?: () => void): void {
  let kind: 'groups' | 'teachers' = 'groups';
  let query = '';
  root.innerHTML = `
    <div class="selected-chips"></div>
    <div class="picker-tools">
      <label class="search-box">${icon('search')}<input class="search-input" type="search" placeholder="Найти группу" aria-label="Поиск в справочнике"></label>
      ${draft.college === 'opec' ? '<div class="tabs"><button class="tab active" data-kind="groups">Группы</button><button class="tab" data-kind="teachers">Преподаватели</button></div>' : ''}
    </div>
    <div class="catalog-list"></div>`;
  const chips = root.querySelector<HTMLDivElement>('.selected-chips')!;
  const list = root.querySelector<HTMLDivElement>('.catalog-list')!;
  const input = root.querySelector<HTMLInputElement>('.search-input')!;

  const renderChips = () => {
    chips.innerHTML = [
      ...draft.groups.map((value) => ({ kind: 'groups', value })),
      ...draft.teachers.map((value) => ({ kind: 'teachers', value })),
    ].map((item) => `<button class="chip" data-kind="${item.kind}" data-value="${escapeHtml(item.value)}" title="Убрать">${escapeHtml(item.value)}</button>`).join('');
  };
  const renderList = () => {
    if (!catalog) {
      list.innerHTML = '<div class="catalog-empty">Загружаем справочник</div>';
      return;
    }
    const source = kind === 'groups' ? catalog.groups : catalog.teachers;
    const selected = kind === 'groups' ? draft.groups : draft.teachers;
    const normalized = query.trim().toLowerCase();
    const filtered = source.filter((value) => value.toLowerCase().includes(normalized));
    list.innerHTML = filtered.length ? filtered.map((value) => `
      <button class="catalog-item ${selected.includes(value) ? 'selected' : ''}" data-value="${escapeHtml(value)}">${escapeHtml(value)}</button>`).join('')
      : '<div class="catalog-empty">Ничего не найдено</div>';
  };
  input.addEventListener('input', () => { query = input.value; renderList(); });
  input.addEventListener('keydown', (event) => {
    if (event.key !== 'Enter') return;
    event.preventDefault();
    list.querySelector<HTMLButtonElement>('.catalog-item:not(.selected)')?.click();
  });
  root.onclick = (event) => {
    const button = (event.target as HTMLElement).closest<HTMLButtonElement>('button');
    if (!button) return;
    if (button.classList.contains('tab')) {
      kind = button.dataset.kind as 'groups' | 'teachers';
      root.querySelectorAll('.tab').forEach((tab) => tab.classList.toggle('active', tab === button));
      input.placeholder = kind === 'groups' ? 'Найти группу' : 'Найти преподавателя';
      renderList();
      return;
    }
    if (button.classList.contains('chip')) {
      const targetKind = button.dataset.kind as 'groups' | 'teachers';
      const target = targetKind === 'groups' ? draft.groups : draft.teachers;
      draft[targetKind] = target.filter((value) => value !== button.dataset.value);
      renderChips();
      renderList();
      onChange?.();
      return;
    }
    if (button.classList.contains('catalog-item')) {
      const value = button.dataset.value!;
      const target = kind === 'groups' ? draft.groups : draft.teachers;
      if (target.includes(value)) {
        draft[kind] = target.filter((item) => item !== value);
      } else {
        if (target.length >= 8) {
          toast(kind === 'groups' ? 'Можно выбрать до 8 групп' : 'Можно выбрать до 8 преподавателей');
          return;
        }
        target.push(value);
      }
      renderChips();
      renderList();
      onChange?.();
    }
  };
  renderChips();
  renderList();
}

function collegeCards(draft: Settings): string {
  return `<div class="college-options">
    <button class="college-card ${draft.college === 'opec' ? 'selected' : ''}" data-college="opec"><strong>ОПЭК</strong><span>Омский промышленно-экономический колледж</span></button>
    <button class="college-card ${draft.college === 'omaep' ? 'selected' : ''}" data-college="omaep"><strong>ОмАЭиП</strong><span>Омская академия экономики и права</span></button>
  </div>`;
}

function showOnboarding(): void {
  const draft: Settings = { college: 'opec', groups: [], teachers: [], theme: snapshot?.settings.theme ?? 'system', autostart: false, onboarded: false };
  let catalog: Catalog | null = null;
  app.innerHTML = `<div class="onboarding"><div class="onboarding-inner">
    <div class="onboarding-head">${brand()}<span class="onboarding-badge">Первый запуск</span></div>
    <h1>Ваше расписание<br>всегда под рукой</h1>
    <p class="onboarding-intro">Выберите колледж и нужные группы или преподавателей.<br>Программа сообщит о новом расписании и изменениях.</p>
    <section class="setup-section"><div class="setup-section-title"><h2>1. Колледж</h2></div><div id="onboarding-college">${collegeCards(draft)}</div></section>
    <section class="setup-section"><div class="setup-section-title"><h2>2. Кого показывать</h2><span>До 8 групп и до 8 преподавателей</span></div><div class="picker" id="onboarding-picker"></div></section>
    <div class="setup-footer"><p>Данные хранятся только на этом компьютере.<br>Настройки можно изменить позже.</p><button class="btn btn-primary btn-large" id="finish-setup" disabled>Показать расписание</button></div>
  </div></div>`;
  const picker = document.querySelector<HTMLElement>('#onboarding-picker')!;
  const finish = document.querySelector<HTMLButtonElement>('#finish-setup')!;
  const updateFinish = () => { finish.disabled = !draft.groups.length && !draft.teachers.length; };
  let catalogRequest = 0;
  const loadCatalog = async () => {
    const request = ++catalogRequest;
    catalog = null;
    setupPicker(picker, draft, catalog, updateFinish);
    updateFinish();
    try {
      const loaded = await backend.fetchCatalog(draft.college);
      if (request !== catalogRequest) return;
      catalog = loaded;
      setupPicker(picker, draft, catalog, updateFinish);
      updateFinish();
    } catch (error) {
      if (request !== catalogRequest) return;
      const list = picker.querySelector<HTMLElement>('.catalog-list');
      if (list) {
        list.innerHTML = '<div class="catalog-empty">Не удалось загрузить справочник. Проверьте интернет.<button class="btn" data-retry-catalog>Повторить</button></div>';
        list.querySelector('[data-retry-catalog]')?.addEventListener('click', () => void loadCatalog(), { once: true });
      }
      toast(String(error));
    }
  };
  document.querySelector('#onboarding-college')!.addEventListener('click', (event) => {
    const button = (event.target as HTMLElement).closest<HTMLButtonElement>('[data-college]');
    if (!button) return;
    const college = button.dataset.college as College;
    if (college === draft.college) return;
    draft.college = college;
    draft.groups = [];
    draft.teachers = [];
    document.querySelector('#onboarding-college')!.innerHTML = collegeCards(draft);
    updateFinish();
    void loadCatalog();
  });
  finish.addEventListener('click', async () => {
    finish.disabled = true;
    finish.textContent = 'Сохраняем';
    try {
      draft.onboarded = true;
      snapshot = await backend.saveSettings(structuredClone(draft));
      lastUpdatedKey = snapshot.last_updated;
      applyTheme(snapshot.settings.theme);
      await ensureNotificationPermission(true);
      showMain();
    } catch (error) {
      toast(String(error));
      finish.disabled = false;
      finish.textContent = 'Показать расписание';
    }
  });
  updateFinish();
  void loadCatalog();
}

function closeSettings(): void {
  document.querySelector<HTMLDialogElement>('#settings-dialog')!.close();
}

function openSettings(): void {
  if (!snapshot) return;
  settingsSaved = false;
  const draft = structuredClone(snapshot.settings);
  let catalog: Catalog | null = null;
  let catalogError = false;
  let catalogRequest = 0;
  const dialog = document.querySelector<HTMLDialogElement>('#settings-dialog')!;
  const content = document.querySelector<HTMLElement>('#settings-content')!;

  const render = () => {
    content.innerHTML = `
      <section class="setup-section"><div class="setup-section-title"><h2>Колледж</h2></div><div id="settings-college">${collegeCards(draft)}</div></section>
      <section class="setup-section"><div class="setup-section-title"><h2>Группы и преподаватели</h2><span>До 8 каждого вида</span></div><div class="picker" id="settings-picker"></div></section>
      <div class="settings-row settings-row-theme"><div><strong>Тема</strong><small>Девять тем сайта и системная</small></div>
        <div class="theme-grid" id="theme-options">
          ${THEME_OPTIONS.map((option) => `
            <button class="theme-option ${draft.theme === option.id ? 'active' : ''}" data-theme="${option.id}" title="${option.label}" aria-pressed="${draft.theme === option.id}">
              <span class="theme-swatch" aria-hidden="true" style="background:${option.swatch}"><i style="background:${option.accent}"></i></span>
              <span class="theme-name">${option.label}</span>
            </button>`).join('')}
        </div>
      </div>
      <div class="settings-row"><div><strong>Запускать вместе с Windows</strong><small>Сворачивание в трей продолжит работу в фоне</small></div><label class="switch"><input id="autostart-input" type="checkbox" ${draft.autostart ? 'checked' : ''}><span class="switch-slider"></span></label></div>
      <div class="dialog-footer"><button class="btn" id="cancel-settings">Отмена</button><button class="btn btn-primary" id="save-settings">Сохранить</button></div>`;
    setupPicker(content.querySelector<HTMLElement>('#settings-picker')!, draft, catalog);
    if (catalogError && !catalog) {
      const list = content.querySelector<HTMLElement>('.catalog-list');
      if (list) {
        list.innerHTML = '<div class="catalog-empty">Не удалось загрузить справочник. Проверьте интернет.<button class="btn" data-retry-catalog>Повторить</button></div>';
        list.querySelector('[data-retry-catalog]')?.addEventListener('click', () => {
          catalogError = false;
          render();
          void loadCatalog();
        });
      }
    }
    content.querySelector('#settings-college')!.addEventListener('click', (event) => {
      const button = (event.target as HTMLElement).closest<HTMLButtonElement>('[data-college]');
      if (!button) return;
      const college = button.dataset.college as College;
      if (college === draft.college) return;
      draft.college = college;
      draft.groups = [];
      draft.teachers = [];
      catalog = null;
      catalogError = false;
      render();
      void loadCatalog();
    });
    content.querySelector('#theme-options')!.addEventListener('click', (event) => {
      const button = (event.target as HTMLElement).closest<HTMLButtonElement>('[data-theme]');
      if (!button) return;
      draft.theme = button.dataset.theme as Theme;
      content.querySelectorAll('.theme-option').forEach((item) => {
        item.classList.toggle('active', item === button);
        item.setAttribute('aria-pressed', String(item === button));
      });
      applyTheme(draft.theme);
    });
    content.querySelector<HTMLInputElement>('#autostart-input')!.addEventListener('change', (event) => {
      draft.autostart = (event.target as HTMLInputElement).checked;
    });
    content.querySelector('#cancel-settings')!.addEventListener('click', closeSettings);
    content.querySelector('#save-settings')!.addEventListener('click', async () => {
      if (!draft.groups.length && !draft.teachers.length) {
        toast('Выберите хотя бы одну группу или преподавателя');
        return;
      }
      const save = content.querySelector<HTMLButtonElement>('#save-settings')!;
      save.disabled = true;
      save.textContent = 'Сохраняем';
      try {
        snapshot = await backend.saveSettings(structuredClone(draft));
        settingsSaved = true;
        applyTheme(snapshot.settings.theme);
        closeSettings();
        renderChrome();
        await refreshSchedule();
      } catch (error) {
        toast(String(error));
        save.disabled = false;
        save.textContent = 'Сохранить';
      }
    });
  };

  const loadCatalog = async () => {
    const request = ++catalogRequest;
    try {
      const loaded = await backend.fetchCatalog(draft.college);
      if (request !== catalogRequest) return;
      catalog = loaded;
      catalogError = false;
      if (dialog.open) render();
    } catch (error) {
      if (request !== catalogRequest) return;
      catalogError = true;
      toast(String(error));
      if (dialog.open) render();
    }
  };
  render();
  dialog.showModal();
  void loadCatalog();
}

function showMain(): void {
  if (!snapshot) return;
  renderShell();
  const initial = pendingDate ?? defaultDate(snapshot);
  pendingDate = null;
  void selectDate(initial);
}

async function reloadCachedDay(): Promise<void> {
  const request = dayRequest;
  const cached = await backend.getCachedDay(selectedDate).catch(() => null);
  if (request !== dayRequest || !cached) return;
  currentDay = cached;
  renderDay();
}

function notificationId(date: string): number {
  let hash = 17;
  for (const character of date) hash = (hash * 31 + character.codePointAt(0)!) & 0x7fffffff;
  return hash;
}

async function ensureNotificationPermission(force = false): Promise<void> {
  if (!isDesktop) return;
  try {
    if (await isPermissionGranted()) return;
    if (!force && notificationsWereAsked()) return;
    const permission = await requestPermission();
    markNotificationsAsked();
    if (permission !== 'granted') {
      toast('Windows не разрешила уведомления. Их можно включить в настройках системы.');
    }
  } catch {
    markNotificationsAsked();
    toast('Windows не разрешила уведомления. Их можно включить в настройках системы.');
  }
}

async function initializeEvents(): Promise<void> {
  if (!isDesktop || eventsInitialized) return;
  eventsInitialized = true;
  await listen<AppSnapshot>('state-changed', (event) => {
    const previous = snapshot;
    const next = event.payload;
    const settingsChanged = !previous
      || previous.settings.college !== next.settings.college
      || previous.settings.theme !== next.settings.theme
      || previous.settings.autostart !== next.settings.autostart
      || previous.settings.onboarded !== next.settings.onboarded
      || previous.settings.groups.join('\u0000') !== next.settings.groups.join('\u0000')
      || previous.settings.teachers.join('\u0000') !== next.settings.teachers.join('\u0000');
    const daysChanged = !previous
      || previous.days.length !== next.days.length
      || previous.days.some((day, index) => {
        const update = next.days[index];
        return day.date !== update.date || day.available !== update.available;
      });
    const statusChanged = !previous
      || previous.last_updated !== next.last_updated
      || previous.message !== next.message
      || previous.retry_at !== next.retry_at;
    snapshot = next;
    if (!settingsChanged && !daysChanged && !statusChanged) return;
    renderChrome(daysChanged || settingsChanged);
    if (next.settings.onboarded && next.last_updated !== lastUpdatedKey) {
      lastUpdatedKey = next.last_updated;
      void reloadCachedDay();
    }
  });
  await listen<NotificationEvent[]>('schedule-notifications', (event) => {
    for (const notification of event.payload) {
      const id = notificationId(notification.date);
      rememberNotification(id, notification.date);
      try {
        sendNotification({ id, title: notification.title, body: notification.body });
      } catch {
        if (!notificationErrorShown) {
          notificationErrorShown = true;
          toast('Windows не показала уведомление. Проверьте разрешения в настройках системы.');
        }
      }
    }
  });
  await onAction((event) => {
    const id = event.notification?.id;
    if (id === undefined) return;
    const date = notificationDates.get(id);
    if (!date) return;
    pendingDate = date;
    void backend.revealWindow();
    if (snapshot?.settings.onboarded && document.querySelector('#day-title')) {
      pendingDate = null;
      void selectDate(date);
    }
  });
}

document.addEventListener('keydown', (event) => {
  if (event.defaultPrevented || event.altKey || event.ctrlKey || event.metaKey) return;
  if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
  if (!snapshot || document.querySelector('dialog[open]')) return;
  const target = event.target;
  if (target instanceof Element && target.closest('input, textarea, select, [contenteditable="true"]')) return;
  const index = snapshot.days.findIndex((day) => day.date === selectedDate);
  if (index < 0) return;
  const next = snapshot.days[index + (event.key === 'ArrowLeft' ? -1 : 1)];
  if (!next) return;
  event.preventDefault();
  void selectDate(next.date);
});

async function boot(): Promise<void> {
  try {
    loadNotificationDates();
    snapshot = await backend.getSnapshot();
    lastUpdatedKey = snapshot.last_updated;
    applyTheme(snapshot.settings.theme);
    await initializeEvents();
    if (snapshot.settings.onboarded) {
      await ensureNotificationPermission();
      showMain();
    } else {
      showOnboarding();
    }
  } catch (error) {
    app.innerHTML = `<div class="boot-screen">${brand()}<span class="muted">Не удалось открыть данные приложения</span><button class="btn btn-primary" id="retry-boot">Попробовать снова</button></div>`;
    document.querySelector('#retry-boot')!.addEventListener('click', () => void boot());
    toast(String(error));
  }
}

void boot();
