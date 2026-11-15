export const ACCENTS = [
  { id: "teal", label: "Teal" },
  { id: "blue", label: "Blue" },
  { id: "violet", label: "Violet" },
  { id: "rose", label: "Rose" },
  { id: "amber", label: "Amber" },
  { id: "slate", label: "Slate" },
] as const;

export type AccentId = (typeof ACCENTS)[number]["id"];

export const DEFAULT_ACCENT: AccentId = "teal";

const ACCENT_IDS = new Set<string>(ACCENTS.map((accent) => accent.id));

export function normalizeAccent(value: unknown): AccentId {
  return typeof value === "string" && ACCENT_IDS.has(value)
    ? (value as AccentId)
    : DEFAULT_ACCENT;
}
