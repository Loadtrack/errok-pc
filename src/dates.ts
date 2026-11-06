import bells from '../shared/bells.json';
import type { College } from './types';

const MONTHS = ['января', 'февраля', 'марта', 'апреля', 'мая', 'июня', 'июля', 'августа', 'сентября', 'октября', 'ноября', 'декабря'];
const WEEKDAYS_SHORT = ['Пн', 'Вт', 'Ср', 'Чт', 'Пт', 'Сб', 'Вс'];
const WEEKDAYS_LONG = ['Понедельник', 'Вторник', 'Среда', 'Четверг', 'Пятница', 'Суббота', 'Воскресенье'];

export function parseDate(value: string): Date {
  const [day, month, year] = value.split('.').map(Number);
  return new Date(year, month - 1, day, 12);
}

export function formatDate(date: Date): string {
  const day = String(date.getDate()).padStart(2, '0');
  const month = String(date.getMonth() + 1).padStart(2, '0');
  return `${day}.${month}.${date.getFullYear()}`;
}

export function today(): string {
  return formatDate(new Date());
}

export function dayNumber(value: string): number {
  return parseDate(value).getDate();
}

export function weekdayShort(value: string): string {
  return WEEKDAYS_SHORT[(parseDate(value).getDay() + 6) % 7];
}

export function fullDate(value: string): string {
  const date = parseDate(value);
  const weekday = WEEKDAYS_LONG[(date.getDay() + 6) % 7];
  return `${weekday}, ${date.getDate()} ${MONTHS[date.getMonth()]}`;
}

export function dayRelation(value: string): string {
  const current = today();
  if (value === current) return 'Сегодня';
  const tomorrow = new Date();
  tomorrow.setDate(tomorrow.getDate() + 1);
  if (value === formatDate(tomorrow)) return 'Завтра';
  return `${dayNumber(value)} ${MONTHS[parseDate(value).getMonth()]}`;
}

export function lessonTimes(college: College, date: string, pair: number): [string, string] {
  const special = (bells.special[college] as Record<string, [string, string][]>)[date];
  const table = special ?? (bells[college] as [string, string][]);
  return table[pair - 1] ?? ['', ''];
}

function isSameDay(left: Date, right: Date): boolean {
  return left.getFullYear() === right.getFullYear()
    && left.getMonth() === right.getMonth()
    && left.getDate() === right.getDate();
}

function shortTime(value: Date): string {
  return value.toLocaleTimeString('ru-RU', { hour: '2-digit', minute: '2-digit' });
}

export function formatUpdated(value: string | null): string {
  if (!value) return 'Ещё не обновлялось';
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return 'Ещё не обновлялось';
  const current = new Date();
  const time = shortTime(date);
  if (isSameDay(date, current)) return `Обновлено в ${time}`;
  const yesterday = new Date();
  yesterday.setDate(yesterday.getDate() - 1);
  if (isSameDay(date, yesterday)) return `Обновлено вчера в ${time}`;
  const day = String(date.getDate()).padStart(2, '0');
  const month = String(date.getMonth() + 1).padStart(2, '0');
  if (date.getFullYear() !== current.getFullYear()) {
    return `Обновлено ${day}.${month}.${date.getFullYear()} в ${time}`;
  }
  return `Обновлено ${day}.${month} в ${time}`;
}

export function formatRetry(value: string | null): string | null {
  if (!value) return null;
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return null;
  const current = new Date();
  const time = shortTime(date);
  if (isSameDay(date, current)) return `Повторим в ${time}`;
  const tomorrow = new Date();
  tomorrow.setDate(tomorrow.getDate() + 1);
  if (isSameDay(date, tomorrow)) return `Повторим завтра в ${time}`;
  return `Повторим ${formatDate(date)} в ${time}`;
}
