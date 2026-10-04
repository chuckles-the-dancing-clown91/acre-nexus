// Where a work order can go from each status. Mirrors `ticket_flow.rs`; the
// server has the final say, this just keeps the choices honest.

export const STATUS_WORDS: Record<string, string> = {
  open: "Open",
  triage: "Triage",
  scheduled: "Scheduled",
  in_progress: "In progress",
  on_hold: "On hold",
  resolved: "Resolved",
  closed: "Closed",
  cancelled: "Cancelled",
};

const ALL = Object.keys(STATUS_WORDS);

const isFinished = (s: string) =>
  ["resolved", "closed", "cancelled"].includes(s);

/** The statuses a work order can move to from `from`. */
export function nextStatuses(from: string): string[] {
  if (from === "resolved")
    return ["closed", "open", "in_progress", "scheduled"];
  if (from === "closed" || from === "cancelled") return ["open", "in_progress"];
  return ALL.filter((s) => s !== from && s !== "closed");
}

/** Whether the move needs the person to say why. */
export function needsReason(from: string, to: string): boolean {
  if (from === "resolved" && to === "closed") return false;
  return isFinished(from) || to === "cancelled";
}

/** What each status asks for, in a few words. */
export function ask(from: string, to: string): string {
  if (needsReason(from, to))
    return isFinished(from)
      ? "Say why it's being reopened."
      : "Say why it's cancelled.";
  switch (to) {
    case "scheduled":
      return "Pick the day the work is set for.";
    case "on_hold":
      return "Say what it's waiting on and when to chase it.";
    case "resolved":
      return "The tasks should be done, and the clocks stop.";
    default:
      return "Add a note if it helps.";
  }
}
