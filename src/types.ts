export type College = 'opec' | 'omaep';
export type Theme = 'light' | 'dark' | 'system';

export interface Settings {
  college: College;
  groups: string[];
  teachers: string[];
  theme: Theme;
  autostart: boolean;
  onboarded: boolean;
}

export interface DateMarker {
  date: string;
  available: boolean;
}

export interface AppSnapshot {
  settings: Settings;
  days: DateMarker[];
  last_updated: string | null;
  message: string | null;
  retry_at: string | null;
}

export interface Catalog {
  groups: string[];
  teachers: string[];
}

export interface Subgroup {
  pg: number;
  discipline: string;
  teacher: string;
  room: string;
  changed: boolean;
  lesson_type: string | null;
  extra: boolean;
}

export interface HomeworkEntry {
  pg: number;
  text: string;
  by_role: string;
}

export interface LessonView {
  pair: number;
  subgroups: Subgroup[];
  homework: HomeworkEntry[];
}

export interface GroupView {
  name: string;
  status: string | null;
  hidden: boolean;
  lessons: LessonView[];
}

export interface AnnouncementView {
  group: string;
  text: string;
  pair: number;
  by_role: string;
}

export interface DayView {
  date: string;
  week: number;
  day: string;
  groups: GroupView[];
  announcements: AnnouncementView[];
}

export interface NotificationEvent {
  date: string;
  title: string;
  body: string;
}

export interface Backend {
  desktop: boolean;
  getSnapshot(): Promise<AppSnapshot>;
  fetchCatalog(college: College): Promise<Catalog>;
  saveSettings(settings: Settings): Promise<AppSnapshot>;
  getCachedDay(date: string): Promise<DayView | null>;
  loadDay(date: string): Promise<DayView | null>;
  refreshNow(): Promise<AppSnapshot>;
  revealWindow(): Promise<void>;
  openOnSite(date: string): Promise<void>;
}
