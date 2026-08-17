"use client";

import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Download, Share2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  agentHandoffRequest,
  type AgentHandoffAction,
  type AgentHandoffFormat,
  type AgentHandoffOutcome,
} from "@/types/verifiable-record";

interface AgentHandoffMenuProps {
  meetingId: string;
  disabled?: boolean;
}

const LABELS: Record<AgentHandoffOutcome, string> = {
  cancelled: "Agent Handoff cancelled. Nothing was written or shared.",
  saved: "Agent Handoff saved.",
  share_presented: "macOS sharing options opened.",
};

export interface PendingAgentHandoff {
  format: AgentHandoffFormat;
  action: AgentHandoffAction;
}

export function consumePendingAgentHandoff(
  open: boolean,
  pending: PendingAgentHandoff | null,
  startNativeAction: (pending: PendingAgentHandoff) => void,
): PendingAgentHandoff | null {
  if (open || pending === null) return pending;
  startNativeAction(pending);
  return null;
}

function failureMessage(error: unknown): string {
  if (typeof error === "object" && error !== null && "code" in error) {
    if (error.code === "snapshot_unavailable") return "The current Session record is unavailable.";
    if (error.code === "share_unavailable") return "macOS sharing is unavailable.";
  }
  return "Agent Handoff failed. No Session content changed.";
}

export function AgentHandoffMenu({ meetingId, disabled = false }: AgentHandoffMenuProps) {
  const [open, setOpen] = useState(false);
  const [pending, setPending] = useState<PendingAgentHandoff | null>(null);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState("");

  const exportRecord = useCallback(async (format: AgentHandoffFormat, action: AgentHandoffAction) => {
    setBusy(true);
    setStatus("");
    try {
      const outcome = await invoke<AgentHandoffOutcome>("api_export_agent_handoff", agentHandoffRequest(meetingId, format, action));
      setStatus(LABELS[outcome]);
    } catch (error) {
      setStatus(failureMessage(error));
    } finally {
      setBusy(false);
    }
  }, [meetingId]);

  useEffect(() => {
    const remaining = consumePendingAgentHandoff(open, pending, ({ format, action }) => {
      void exportRecord(format, action);
    });
    if (remaining !== pending) setPending(remaining);
  }, [exportRecord, open, pending]);

  function select(format: AgentHandoffFormat, action: AgentHandoffAction) {
    setPending({ format, action });
    setOpen(false);
  }

  return (
    <div className="flex items-center gap-3">
      <DropdownMenu open={open} onOpenChange={setOpen}>
        <DropdownMenuTrigger asChild>
          <Button type="button" variant="outline" size="sm" disabled={disabled || busy} aria-label="Agent Handoff options">
            <Share2 className="h-4 w-4" aria-hidden="true" />
            Agent Handoff
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          <DropdownMenuLabel>Save for an agent</DropdownMenuLabel>
          <DropdownMenuItem onSelect={() => select("markdown", "save")}><Download className="mr-2 h-4 w-4" />Save Markdown</DropdownMenuItem>
          <DropdownMenuItem onSelect={() => select("json", "save")}><Download className="mr-2 h-4 w-4" />Save JSON</DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuLabel>Share with macOS</DropdownMenuLabel>
          <DropdownMenuItem onSelect={() => select("markdown", "share")}><Share2 className="mr-2 h-4 w-4" />Share Markdown</DropdownMenuItem>
          <DropdownMenuItem onSelect={() => select("json", "share")}><Share2 className="mr-2 h-4 w-4" />Share JSON</DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
      <p aria-live="polite" className="text-xs text-gray-600">{disabled ? "Save transcript corrections before Agent Handoff." : status}</p>
    </div>
  );
}
