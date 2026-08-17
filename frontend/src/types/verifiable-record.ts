export type RecordType = "meeting" | "interview" | "content";
export type GenerationStatus = "pending" | "processing" | "completed" | "unavailable" | "failed" | "stale";

export interface EvidenceReference { block_id: string; timestamp_ms: number }
export interface Finding { label: string; detail: string; evidence: EvidenceReference[] }
export type GeneratedRecord =
  | { record_type: "meeting"; summary: string; decisions: Finding[]; action_items: Finding[]; key_points: Finding[] }
  | { record_type: "interview"; summary: string; answers: Finding[]; themes: Finding[]; follow_ups: Finding[] }
  | { record_type: "content"; summary: string; claims: Finding[]; outline: Finding[]; source_notes: Finding[] };
export interface VerifiableRecord {
  schema_version: number;
  meeting_id: string;
  title: string;
  record_type: RecordType;
  principal_transcript_revision: number;
  participants: Array<{ id: string; display_name: string; speaker_cluster_id?: string | null }>;
  transcript: Array<{ id: string; text: string; participant_id: string; start_ms: number; end_ms: number }>;
  generation_status: GenerationStatus;
  generated?: GeneratedRecord | null;
  error_code?: string | null;
}

export type AgentHandoffFormat = "markdown" | "json";
export type AgentHandoffAction = "save" | "share";
export type AgentHandoffOutcome = "cancelled" | "saved" | "share_presented";

export interface ProviderPreview {
  transferId: string;
  previewDigest: string;
  provider: string;
  providerDisplayName: string;
  sessionTitle: string;
  purpose: string;
  task: string;
  dataTypes: string[];
  principalTranscriptRevision: number;
}

export interface ProviderTransferOutcome {
  transferId: string;
  provider: string;
  task: string;
  status: "completed";
  result?: unknown;
  errorCode?: string | null;
}

export function providerTransferError(error: unknown): string {
  const code = verifiableRecordErrorCode(error);
  if (code === "task_disabled") return "Enable Agent Handoff transfer in Settings before sending.";
  if (code === "credential_unavailable") return "The provider credential is unavailable. Update it in Settings.";
  if (code === "stale_preview") return "The Session changed. Review a fresh preview before retrying.";
  if (code === "already_consumed") return "This confirmation was already used. Review a fresh preview to retry.";
  if (code === "request_failed") return "The provider request failed. Nothing was retried or sent to another provider.";
  if (code === "outcome_unknown") return "The provider may have accepted this transfer. Automatic and explicit retry are disabled to prevent a duplicate.";
  if (code === "invalid_response") return "The provider returned an invalid result. Review a fresh preview to retry.";
  return "The external transfer could not be completed.";
}

export function providerTransferCanRetry(code?: string): boolean {
  return code !== "outcome_unknown";
}

export function agentHandoffRequest(
  meetingId: string,
  format: AgentHandoffFormat,
  action: AgentHandoffAction,
): { meetingId: string; format: AgentHandoffFormat; action: AgentHandoffAction } {
  return { meetingId, format, action };
}

const UNAVAILABLE: Record<string, string> = {
  apple_intelligence_disabled: "Apple Intelligence is disabled. The Session remains available; local findings are pending.",
  helper_unavailable: "Local findings are unavailable in this app build. The Session remains available.",
  ineligible_device: "This Mac is not eligible for Apple Foundation Models. The Session remains available.",
  model_not_ready: "Apple Foundation Models are not ready. Try again later.",
  unsupported_locale: "The current locale is not supported for local findings.",
  unsupported_os: "This macOS version does not support local findings.",
};

export function generationMessage(status: GenerationStatus, code?: string): string {
  if (code === "cancelled") return "Local generation was cancelled. No Session content changed.";
  if (status === "unavailable") return UNAVAILABLE[code ?? ""] ?? "Local findings are unavailable. The Session remains available.";
  if (status === "stale") return "The principal transcript changed. Generate current findings again.";
  if (status === "failed" && code === "timeout") return "Local generation timed out. Try again when ready.";
  if (status === "failed" && code === "context_size") return "This transcript is too large for the local model context.";
  if (status === "failed") return "Local generation failed. The Session and prior accepted findings are unchanged.";
  if (status === "processing") return "Generating grounded local findings…";
  if (status === "completed") return "Findings match the current principal transcript.";
  return "Local findings are pending.";
}

export function generatedFindings(record: GeneratedRecord): Finding[] {
  if (record.record_type === "meeting") return [...record.decisions, ...record.action_items, ...record.key_points];
  if (record.record_type === "interview") return [...record.answers, ...record.themes, ...record.follow_ups];
  return [...record.claims, ...record.outline, ...record.source_notes];
}

export function verifiableRecordErrorCode(error: unknown): string | undefined {
  if (typeof error === "object" && error !== null && "code" in error && typeof error.code === "string") return error.code;
  return undefined;
}
