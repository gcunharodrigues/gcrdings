/**
 * Recency buckets for the Session list.
 *
 * A flat list of titles becomes unreadable past a couple of dozen Sessions:
 * every row looks identical and nothing says when anything happened.
 */
export type RecencyBucket = 'today' | 'yesterday' | 'week' | 'month' | 'older';

export const RECENCY_LABELS: Record<RecencyBucket, string> = {
  today: 'Today',
  yesterday: 'Yesterday',
  week: 'Earlier this week',
  month: 'Earlier this month',
  older: 'Older',
};

const BUCKET_ORDER: RecencyBucket[] = ['today', 'yesterday', 'week', 'month', 'older'];

function startOfDay(date: Date): number {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
}

export function recencyBucket(createdAt: string | undefined, now: Date = new Date()): RecencyBucket {
  if (!createdAt) return 'older';
  const created = new Date(createdAt);
  if (Number.isNaN(created.getTime())) return 'older';

  const days = Math.floor((startOfDay(now) - startOfDay(created)) / 86_400_000);
  if (days <= 0) return 'today';
  if (days === 1) return 'yesterday';
  if (days < 7) return 'week';
  if (days < 31) return 'month';
  return 'older';
}

/** Short label for a row: a time today, a weekday this week, a date otherwise. */
export function shortDateLabel(createdAt: string | undefined, now: Date = new Date()): string {
  if (!createdAt) return '';
  const created = new Date(createdAt);
  if (Number.isNaN(created.getTime())) return '';

  const bucket = recencyBucket(createdAt, now);
  if (bucket === 'today') return created.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
  if (bucket === 'yesterday') return 'Yesterday';
  if (bucket === 'week') return created.toLocaleDateString(undefined, { weekday: 'short' });
  if (created.getFullYear() === now.getFullYear()) {
    return created.toLocaleDateString(undefined, { day: '2-digit', month: 'short' });
  }
  return created.toLocaleDateString(undefined, { month: 'short', year: 'numeric' });
}

export interface RecencyGroup<T> {
  bucket: RecencyBucket;
  label: string;
  items: T[];
}

/**
 * Newest first inside each bucket, buckets in calendar order. Buckets with no
 * Sessions are omitted rather than rendered empty.
 */
export function groupByRecency<T extends { id: string; createdAt?: string }>(
  items: T[],
  now: Date = new Date(),
): Array<RecencyGroup<T>> {
  const buckets = new Map<RecencyBucket, T[]>();

  for (const item of items) {
    const bucket = recencyBucket(item.createdAt, now);
    const existing = buckets.get(bucket);
    if (existing) existing.push(item);
    else buckets.set(bucket, [item]);
  }

  return BUCKET_ORDER.flatMap((bucket) => {
    const group = buckets.get(bucket);
    if (!group || group.length === 0) return [];
    group.sort((a, b) => new Date(b.createdAt ?? 0).getTime() - new Date(a.createdAt ?? 0).getTime());
    return [{ bucket, label: RECENCY_LABELS[bucket], items: group }];
  });
}
