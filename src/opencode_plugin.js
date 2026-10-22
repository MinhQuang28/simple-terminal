// Repodeck's OpenCode plugin. Appends one JSON line per lifecycle event to the file named by
// REPODECK_AGENT_EVENTS, shaped like Claude Code's hook payloads so Repodeck parses both alike.
import { appendFileSync } from "node:fs";

export const RepodeckEvents = async () => {
  const path = process.env.REPODECK_AGENT_EVENTS;
  if (!path) return {};
  let root = null;
  const roles = new Map();
  let lastUserText = "";
  let lastAssistantText = "";
  const emit = (hook_event_name, session_id, extra = {}) => {
    try {
      appendFileSync(path, JSON.stringify({ hook_event_name, session_id, ...extra }) + "\n");
    } catch (_) {}
  };
  const sessionOf = (event) => {
    const p = event?.properties || {};
    return p.sessionID || p.sessionId || p.info?.sessionID || p.part?.sessionID || p.info?.id;
  };
  return {
    event: async ({ event }) => {
      const p = event?.properties || {};
      const session = sessionOf(event);
      if (event.type === "session.created") {
        const info = p.info || p;
        if (info.parentID) return; // sub-agent sessions belong to the root session's turn
        if (!root) {
          root = info.id || session;
          emit("SessionStart", root);
          // appendFileSync is synchronous, so the exit handler gets its line out.
          process.once("exit", () => emit("SessionEnd", root));
        }
        return;
      }
      if (root && session && session !== root) return;
      switch (event.type) {
        case "session.deleted":
          if ((p.info?.id || session) === root) emit("SessionEnd", root);
          return;
        case "message.updated":
          if (p.info?.id && p.info?.role) roles.set(p.info.id, p.info.role);
          return;
        case "message.part.updated": {
          const part = p.part || {};
          if (part.type !== "text" || !part.text) return;
          const role = roles.get(part.messageID);
          if (role === "user") lastUserText = part.text;
          if (role === "assistant") lastAssistantText = part.text;
          return;
        }
        case "session.status": {
          const status = p.status;
          const type = typeof status === "string" ? status : status?.type;
          if (type === "busy") emit("UserPromptSubmit", session, { prompt: lastUserText });
          else if (type === "idle") emit("Stop", session, { last_assistant_message: lastAssistantText });
          return;
        }
        case "session.idle":
          emit("Stop", session, { last_assistant_message: lastAssistantText });
          return;
        case "session.error": {
          const error = p.error || {};
          emit("StopFailure", session, { error: String(error.data?.message || error.name || "error") });
          return;
        }
        case "permission.asked":
          emit("PermissionRequest", p.sessionID || session, {
            tool_name: p.permission || p.type || "Permission",
            tool_input: { description: p.title || (p.patterns || []).join(" ") },
          });
          return;
        case "question.asked":
          emit("Notification", p.sessionID || session, {
            notification_type: "elicitation_dialog",
            message: p.questions?.[0]?.question || "Question",
          });
          return;
        default:
          return;
      }
    },
  };
};
