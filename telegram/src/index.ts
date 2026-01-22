import { Bot, Context } from "grammy";
import "dotenv/config";
import fetch, {
  RequestInit,
  Response,
} from "node-fetch";

// ---------- Config ----------

const token = process.env.TELEGRAM_BOT_TOKEN;
if (!token) {
  throw new Error("TELEGRAM_BOT_TOKEN is not set");
}

// AI Service configuration (abstracted from specific backend)
const aiServiceApiUrl =
  process.env.AI_SERVICE_API_URL || "http://127.0.0.1:8000/v1/chat/completions";
const aiServiceModel = process.env.AI_SERVICE_MODEL || "default";
const aiServiceAuthToken = process.env.AI_SERVICE_AUTH_TOKEN || "Bearer dummy";
const serverName = process.env.SERVER_NAME || "unknown-server";
const sessionNamespace =
  process.env.AI_SERVICE_SESSION_NAMESPACE?.trim() || serverName;

const allowedUserIdsEnv = process.env.ALLOWED_USER_IDS || "";
const ALLOWED_USER_IDS = new Set<number>(
  allowedUserIdsEnv
    .split(",")
    .map((s: string) => s.trim())
    .filter((s: string) => s.length > 0)
    .map((s: string) => Number(s))
    .filter((n: number) => Number.isFinite(n))
);

if (ALLOWED_USER_IDS.size === 0) {
  console.warn(
    "[WARN] ALLOWED_USER_IDS is empty. Bot will ignore all messages."
  );
}

// ---------- Types & State ----------

type Role = "system" | "user" | "assistant";
type Message = { role: Role; content: string };

// Track a rolling version number per chat so /reset can start a new
// persistent session without touching on-disk files directly.
const sessionVersionByChat = new Map<number, number>();

function getSessionId(chatId: number): string {
  const version = sessionVersionByChat.get(chatId) ?? 0;
  return `${sessionNamespace}-telegram-${chatId}-v${version}`;
}

// Simple fetch with timeout wrapper
async function fetchWithTimeout(
  url: string,
  init: RequestInit,
  timeoutMs: number
): Promise<Response> {
  const controller = new AbortController();
  const id = setTimeout(() => controller.abort(), timeoutMs);

  try {
    const res = await fetch(url, { ...init, signal: controller.signal });
    return res;
  } finally {
    clearTimeout(id);
  }
}

// Telegram limit is 4096 characters – we’ll stay safely under that.
const TELEGRAM_MAX_LEN = 4000;

function chunkText(text: string, maxLen: number = TELEGRAM_MAX_LEN): string[] {
  const chunks: string[] = [];
  let remaining = text;

  while (remaining.length > maxLen) {
    let idx = remaining.lastIndexOf("\n", maxLen);
    if (idx === -1) idx = maxLen;
    chunks.push(remaining.slice(0, idx));
    remaining = remaining.slice(idx);
  }
  if (remaining.length > 0) {
    chunks.push(remaining);
  }
  return chunks;
}

// ---------- AI Service bridge ----------

async function callAiService(chatId: number, userText: string): Promise<string> {
  const sessionId = getSessionId(chatId);

  const systemPrompt = `You are Fio, an assistant running on the server "${serverName}".
You are being accessed over Telegram by the server owner.
Be concise, but clear. If the user asks about this server, answer from the perspective of "${serverName}".`;

  const messages = [
    { role: "system" as const, content: systemPrompt },
    { role: "user" as const, content: userText },
  ];

  const body = {
    model: aiServiceModel,
    session_id: sessionId,
    messages,
    stream: false,
  };

  const res = await fetchWithTimeout(
    aiServiceApiUrl,
    {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: aiServiceAuthToken,
      },
      body: JSON.stringify(body),
    },
    60_000 // 60s timeout
  );

  if (!res.ok) {
    const errText = await res.text().catch(() => "");
    throw new Error(`AI service HTTP ${res.status}: ${errText}`);
  }

  const json: any = await res.json();
  const content: string =
    json.choices?.[0]?.message?.content ?? "(empty reply from AI service)";
  return content;
}

// ---------- Telegram bot setup ----------

const bot = new Bot<Context>(token);

// Access control middleware
bot.use(async (ctx: Context, next: () => Promise<void>) => {
  const fromId = ctx.from?.id;
  if (!fromId) return;

  if (!ALLOWED_USER_IDS.has(fromId)) {
    // You can either silently ignore, or send a one-time message.
    // For now, we silently ignore.
    console.log(
      `[INFO] Ignoring message from unauthorized user: ${fromId} (${ctx.from?.username})`
    );
    return;
  }

  return next();
});

// Commands
bot.command("start", async (ctx: Context) => {
  await ctx.reply(
    `Hi, I'm Fio, the Telegram gateway for "${serverName}". Send me a message and I'll relay it to the AI service running on this server.\n\nCommands:\n/reset – clear conversation history`
  );
});

bot.command("reset", async (ctx: Context) => {
  if (!ctx.chat) return;
  const chatId = ctx.chat.id;
  const nextVersion = (sessionVersionByChat.get(chatId) ?? 0) + 1;
  sessionVersionByChat.set(chatId, nextVersion);
  await ctx.reply("Persistent conversation context cleared for this chat.");
});

// Handle plain text messages
bot.on("message:text", async (ctx: Context) => {
  if (!ctx.chat || !ctx.message?.text) return;
  const chatId = ctx.chat.id;
  const fromId = ctx.from?.id;
  const text = ctx.message.text.trim();

  console.log(
    `[MSG] chat=${chatId}, from=${fromId}, text="${text.slice(0, 80)}${
      text.length > 80 ? "..." : ""
    }"`
  );

  if (!text) return;

  const thinkingMsg = await ctx.reply("🤔 Fio is thinking…");

  try {
    const replyText = await callAiService(chatId, text);
    const chunks = chunkText(replyText);

    // First chunk replaces the "thinking" message
    await ctx.api.editMessageText(chatId, thinkingMsg.message_id, chunks[0]);

    // Remaining chunks (if any) are sent as follow-up messages
    for (let i = 1; i < chunks.length; i++) {
      await ctx.reply(chunks[i]);
    }
  } catch (err: any) {
    console.error("Error calling AI service:", err);
    const msg = `⚠️ Error talking to Fio: ${
      err?.message ?? String(err)
    }`.slice(0, TELEGRAM_MAX_LEN);
    await ctx.api.editMessageText(chatId, thinkingMsg.message_id, msg);
  }
});

// Start long-polling
bot.start();
console.log(`Server bot for "${serverName}" started.`);
