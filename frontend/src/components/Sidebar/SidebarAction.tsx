'use client';

import type { ReactNode } from 'react';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';

/**
 * One definition for the sidebar's actions, rendered either as an icon in the
 * collapsed rail or as a labelled button in the expanded footer.
 *
 * They used to be two separate implementations of the same five buttons, and
 * they had already drifted: the collapsed set had tooltips, the expanded set
 * did not.
 */
export type SidebarActionTone = 'primary' | 'accent' | 'neutral';

const TONE_ICON: Record<SidebarActionTone, string> = {
  primary: 'bg-record text-record-foreground hover:brightness-110 disabled:opacity-60 brand-sheen',
  accent: 'bg-blue-50 text-blue-600 hover:bg-blue-100',
  neutral: 'text-muted-foreground hover:bg-muted',
};

const TONE_FULL: Record<SidebarActionTone, string> = {
  primary: 'bg-record text-record-foreground hover:brightness-110 disabled:opacity-60 disabled:cursor-not-allowed brand-sheen',
  accent: 'bg-blue-100 text-foreground/90 hover:bg-blue-200',
  neutral: 'bg-secondary text-foreground/90 hover:bg-accent',
};

interface SidebarActionProps {
  icon: ReactNode;
  label: string;
  onClick: () => void;
  collapsed: boolean;
  tone?: SidebarActionTone;
  disabled?: boolean;
  isActive?: boolean;
  /** Shown next to the label, e.g. "⌘R". */
  shortcut?: string;
}

export function SidebarAction({
  icon,
  label,
  onClick,
  collapsed,
  tone = 'neutral',
  disabled = false,
  isActive = false,
  shortcut,
}: SidebarActionProps) {
  const hint = shortcut ? `${label}  ${shortcut}` : label;

  if (collapsed) {
    return (
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            type="button"
            onClick={onClick}
            disabled={disabled}
            aria-label={label}
            aria-current={isActive ? 'page' : undefined}
            className={`p-2 transition-colors duration-150 ${
              tone === 'primary' ? 'rounded-full shadow-sm' : 'rounded-lg'
            } ${TONE_ICON[tone]} ${isActive && tone === 'neutral' ? 'bg-muted' : ''}`}
          >
            {icon}
          </button>
        </TooltipTrigger>
        <TooltipContent side="right">
          <p>{hint}</p>
        </TooltipContent>
      </Tooltip>
    );
  }

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          onClick={onClick}
          disabled={disabled}
          aria-current={isActive ? 'page' : undefined}
          className={`w-full flex items-center justify-center gap-2 px-3 py-2 text-sm font-medium rounded-lg transition-colors shadow-sm ${TONE_FULL[tone]} ${
            isActive && tone === 'neutral' ? 'bg-secondary' : ''
          }`}
        >
          {icon}
          <span>{label}</span>
        </button>
      </TooltipTrigger>
      {shortcut && (
        <TooltipContent side="right">
          <p>{hint}</p>
        </TooltipContent>
      )}
    </Tooltip>
  );
}
