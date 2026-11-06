import { expect, test } from '@playwright/test';

test('главное окно показывает пары и настройки', async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'dark' });
  await page.goto('/');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await expect(page.getByRole('heading', { name: 'БП2-113' })).toBeVisible();
  await expect(page.getByText('Разраб.прог.модулей')).toBeVisible();
  await expect(page.getByText('547', { exact: true })).toBeVisible();
  await expect(page.getByText('изменено', { exact: true })).toBeVisible();
  await page.screenshot({ path: 'artifacts/main-dark.png', fullPage: true });

  await page.getByRole('button', { name: 'Настройки', exact: true }).click();
  await expect(page.getByRole('dialog')).toBeVisible();
  await page.getByRole('button', { name: 'Светлая', exact: true }).click();
  await page.getByRole('button', { name: 'Сохранить', exact: true }).click();
  await expect(page.getByRole('dialog')).not.toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await page.screenshot({ path: 'artifacts/main-light.png', fullPage: true });
});

test('день без расписания имеет понятное пустое состояние', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'БП2-113' })).toBeVisible();
  await page.getByRole('tab', { name: /Сегодня/ }).click();
  await expect(page.getByRole('heading', { name: 'Расписание ещё не выложено' })).toBeVisible();
});

test('дни переключаются стрелками на клавиатуре', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'БП2-113' })).toBeVisible();
  await page.keyboard.press('ArrowLeft');
  await expect(page.getByRole('heading', { name: 'Расписание ещё не выложено' })).toBeVisible();
  await page.keyboard.press('ArrowRight');
  await expect(page.getByText('Разраб.прог.модулей')).toBeVisible();
});

test('поиск в справочнике выбирает группу по Enter', async ({ page }) => {
  await page.goto('/?onboarding=1');
  await expect(page.getByRole('button', { name: 'БП2-113', exact: true })).toBeVisible();
  const search = page.getByRole('searchbox');
  await search.fill('БП2-113');
  await search.press('Enter');
  await expect(page.locator('.chip', { hasText: 'БП2-113' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Показать расписание' })).toBeEnabled();
});

test('первый запуск выбирает колледж и группу', async ({ page }) => {
  await page.goto('/?onboarding=1');
  await expect(page.getByRole('heading', { name: 'Ваше расписание всегда под рукой' })).toBeVisible();
  const group = page.getByRole('button', { name: 'БП2-113', exact: true });
  await expect(group).toBeVisible();
  await group.click();
  const finish = page.getByRole('button', { name: 'Показать расписание' });
  await expect(finish).toBeEnabled();
  await finish.click();
  await expect(page.getByRole('heading', { name: 'БП2-113' })).toBeVisible();
  await expect(page.getByText('Разраб.прог.модулей')).toBeVisible();
});
