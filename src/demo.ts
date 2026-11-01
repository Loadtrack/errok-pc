import { formatDate, today } from './dates';
import type { AppSnapshot, Backend, Catalog, College, DayView, Settings } from './types';

const tomorrow = () => {
  const date = new Date();
  date.setDate(date.getDate() + 1);
  return formatDate(date);
};

const demoCatalog: Catalog = {
  groups: ['ББ-125', 'ББ-126', 'БД-144', 'БМ-223', 'БП1-113', 'БП2-113', 'БП2-114', 'БХ1-33', 'БХТ2-35', 'КИП1-195', 'ЛК1-205', 'О-26', 'ТО-84', 'ЭК1-204'],
  teachers: ['Андреев М.А.', 'Гуркова Ю.Р.', 'Жердева Т.И.', 'Мальцева Т.А.', 'Польченко А.О.', 'Рудакова О.И.', 'Смольякова К.Д.'],
};

function makeSnapshot(settings: Settings): AppSnapshot {
  const available = tomorrow();
  const days = Array.from({ length: 7 }, (_, index) => {
    const date = new Date();
    date.setDate(date.getDate() + index);
    const value = formatDate(date);
    return { date: value, available: value === available };
  });
  return { settings, days, last_updated: new Date().toISOString(), message: null, retry_at: null };
}

function demoDay(settings: Settings, date: string): DayView | null {
  if (date !== tomorrow()) return null;
  const name = settings.groups[0] ?? 'БП2-113';
  return {
    date,
    week: 2,
    day: 'Понедельник',
    announcements: [{ group: name, text: 'Взять сменную обувь', pair: 0, by_role: 'headman' }],
    groups: [{
      name,
      status: 'учеб.пр.+теорет.обуч.',
      hidden: false,
      lessons: [
        {
          pair: 3,
          homework: [],
          subgroups: [{ pg: 0, discipline: 'Физ-ра', teacher: 'Мальцева Т.А.', room: 'С.з 3к', changed: false, lesson_type: null, extra: false }],
        },
        {
          pair: 4,
          homework: [{ pg: 0, text: 'Повторить тему по материалам занятия', by_role: 'headman' }],
          subgroups: [{ pg: 0, discipline: 'Разраб.прог.модулей', teacher: 'Рудакова О.И.', room: '547', changed: true, lesson_type: null, extra: false }],
        },
      ],
    }],
  };
}

export function createDemoBackend(): Backend {
  const firstRun = new URLSearchParams(window.location.search).has('onboarding');
  let settings: Settings = { college: 'opec', groups: firstRun ? [] : ['БП2-113'], teachers: [], theme: 'system', autostart: false, onboarded: !firstRun };
  return {
    desktop: false,
    async getSnapshot() { return makeSnapshot(settings); },
    async fetchCatalog(college: College) {
      return college === 'opec' ? demoCatalog : { groups: ['Б-9.35', 'И-9.35', 'П-9.35', 'Т-9.35'], teachers: [] };
    },
    async saveSettings(next: Settings) { settings = structuredClone(next); return makeSnapshot(settings); },
    async getCachedDay(date: string) { return demoDay(settings, date); },
    async loadDay(date: string) {
      await new Promise((resolve) => setTimeout(resolve, 180));
      return demoDay(settings, date);
    },
    async refreshNow() { return makeSnapshot(settings); },
    async revealWindow() {},
    async openOnSite(date: string) {
      const group = settings.groups[0] ?? 'БП2-113';
      window.open(`https://errok.ru/#${date}/${encodeURIComponent(group)}`, '_blank', 'noopener');
    },
  };
}

export function emptyDemoDay(): DayView | null {
  return demoDay({ college: 'opec', groups: ['БП2-113'], teachers: [], theme: 'system', autostart: false, onboarded: true }, today());
}
