import type { RecordType } from "@/types/verifiable-record";

/**
 * How a Session record reads and how it looks are separate choices.
 *
 * `RecordVoice` changes the words the model writes. `RecordShape` changes how
 * the finished findings are laid out on screen. Keeping them apart means a
 * reader who wants terse language but a table, or full prose but a timeline,
 * does not have to accept a bundle someone else chose.
 */
export type RecordVoice = "objective" | "executive" | "technical" | "narrative" | "legal";

export type RecordShape = "brief" | "prose" | "table" | "timeline" | "map" | "chart";

export interface RecordMode {
  recordType: RecordType;
  voice: RecordVoice;
  shape: RecordShape;
}

export const DEFAULT_RECORD_MODE: RecordMode = {
  recordType: "meeting",
  voice: "executive",
  shape: "brief",
};

interface VoiceDefinition {
  label: string;
  description: string;
  /** Appended to the generation prompt. The backend owns the rest of it. */
  instruction: string;
  /** One line written in this voice, so the choice can be judged before use. */
  sample: string;
}

export const RECORD_VOICES: Record<RecordVoice, VoiceDefinition> = {
  objective: {
    label: "Objective",
    description: "Shortest possible statements. No adjectives, no framing.",
    instruction:
      "Write in the fewest words that stay accurate. Drop adjectives, adverbs, hedging and framing. One clause per statement. Never restate the question.",
    sample: "Scope split in two. Catalogue first. Gateway is the risk.",
  },
  executive: {
    label: "Executive",
    description: "For someone who was not in the room. Decisions and consequences.",
    instruction:
      "Write for a reader who was not present. Lead with the decision or outcome, then its consequence. Name who owns what. Skip discussion that changed nothing.",
    sample:
      "The team split delivery in two to contain gateway risk: catalogue ships first, checkout follows.",
  },
  technical: {
    label: "Technical",
    description: "Keeps terms, numbers and trade-offs intact.",
    instruction:
      "Preserve domain terms, figures, versions and named systems exactly as spoken. State trade-offs with both sides. Do not simplify vocabulary for a general reader.",
    sample:
      "Delivery split into catalogue and checkout phases; the payment-gateway integration is the schedule-critical dependency.",
  },
  narrative: {
    label: "Narrative",
    description: "Chronological. Keeps how the conversation moved.",
    instruction:
      "Follow the order the conversation happened in. Show how a position changed and what prompted it. Keep the participants distinguishable.",
    sample:
      "The kickoff opened on scope. A concern about the gateway timeline shifted the plan, and the group settled on shipping the catalogue first.",
  },
  legal: {
    label: "Evidentiary",
    description: "Every claim carries its quoted passage and timestamp.",
    instruction:
      "Attribute every statement to a named participant. Quote the passage the claim rests on and cite its timestamp. Never infer beyond what was said aloud.",
    sample:
      'Marina, at 00:12: "the prazo de integração com o gateway" — recorded as the stated schedule risk.',
  },
};

interface ShapeDefinition {
  label: string;
  description: string;
  /** Shapes that need per-participant data are hidden without participants. */
  requiresParticipants?: boolean;
  /** Shapes that need evidence timestamps are hidden without them. */
  requiresTimestamps?: boolean;
}

export const RECORD_SHAPES: Record<RecordShape, ShapeDefinition> = {
  brief: {
    label: "Short list",
    description: "One line per finding. Fastest to scan.",
  },
  prose: {
    label: "Full text",
    description: "Continuous paragraphs. Reads like a written account.",
  },
  table: {
    label: "Table",
    description: "Owner, item and evidence in aligned columns.",
  },
  timeline: {
    label: "Timeline",
    description: "Findings in the order they were said.",
    requiresTimestamps: true,
  },
  map: {
    label: "Map",
    description: "A tree of themes and the findings under each.",
  },
  chart: {
    label: "Chart",
    description: "Where the findings and the speaking time actually fell.",
    requiresParticipants: true,
  },
};

export const RECORD_TYPE_LABELS: Record<RecordType, string> = {
  meeting: "Meeting",
  interview: "Interview",
  content: "Content",
};

export function isRecordVoice(value: unknown): value is RecordVoice {
  return typeof value === "string" && value in RECORD_VOICES;
}

export function isRecordShape(value: unknown): value is RecordShape {
  return typeof value === "string" && value in RECORD_SHAPES;
}

export function isRecordType(value: unknown): value is RecordType {
  return value === "meeting" || value === "interview" || value === "content";
}
