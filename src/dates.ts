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

export function formatUpdated(value: string | null): string {
  if (!value) return 'Ещё не обновлялось';
  return `Обновлено ${new Date(value).toLocaleTimeString('ru-RU', { hour: '2-digit', minute: '2-digit' })}`;
}
