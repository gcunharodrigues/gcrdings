/**
 * Diagnostic logging that stays out of a packaged build.
 *
 * The app shipped 300+ bare console.log calls, so the release console was
 * unreadable and leaked Session titles, file paths and model configuration.
 * Warnings and errors still reach the console in every build — those are the
 * ones a user is asked to copy into a bug report.
 */
const isDevelopment = process.env.NODE_ENV !== 'production';

export const log = {
  debug: (...args: unknown[]) => {
    if (isDevelopment) console.log(...args);
  },
  warn: (...args: unknown[]) => console.warn(...args),
  error: (...args: unknown[]) => console.error(...args),
};
