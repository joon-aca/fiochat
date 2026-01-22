import { readFileSync, existsSync } from "fs";
import { join } from "path";
import { homedir } from "os";

interface TelegramConfig {
  telegram_bot_token: string;
  allowed_user_ids: string;
  server_name?: string;
  ai_service_api_url?: string;
  ai_service_model?: string;
  ai_service_auth_token?: string;
  ai_service_session_namespace?: string;
}

interface FiochatConfig {
  telegram?: TelegramConfig;
  [key: string]: any;
}

/**
 * Load configuration from ~/.config/fiochat/config.yaml
 * Falls back to .env file if YAML doesn't exist or doesn't have telegram section
 * Environment variables always override config file values
 */
export function loadConfig() {
  const configPath = join(homedir(), ".config", "fiochat", "config.yaml");
  let yamlConfig: TelegramConfig | null = null;

  // Try to load from YAML
  if (existsSync(configPath)) {
    try {
      const yaml = readFileSync(configPath, "utf8");
      const parsed = parseYaml(yaml) as FiochatConfig;

      if (parsed.telegram) {
        yamlConfig = parsed.telegram;
        console.log(`[CONFIG] Loaded configuration from ${configPath}`);
      }
    } catch (err) {
      console.warn(`[CONFIG] Failed to parse ${configPath}:`, err);
    }
  }

  // Merge: YAML as base, env vars as overrides
  const config = {
    telegram_bot_token:
      process.env.TELEGRAM_BOT_TOKEN ||
      yamlConfig?.telegram_bot_token ||
      "",

    allowed_user_ids:
      process.env.ALLOWED_USER_IDS ||
      yamlConfig?.allowed_user_ids ||
      "",

    server_name:
      process.env.SERVER_NAME ||
      yamlConfig?.server_name ||
      "unknown-server",

    ai_service_api_url:
      process.env.AI_SERVICE_API_URL ||
      yamlConfig?.ai_service_api_url ||
      "http://127.0.0.1:8000/v1/chat/completions",

    ai_service_model:
      process.env.AI_SERVICE_MODEL ||
      yamlConfig?.ai_service_model ||
      "default",

    ai_service_auth_token:
      process.env.AI_SERVICE_AUTH_TOKEN ||
      yamlConfig?.ai_service_auth_token ||
      "Bearer dummy",

    ai_service_session_namespace:
      process.env.AI_SERVICE_SESSION_NAMESPACE ||
      yamlConfig?.ai_service_session_namespace ||
      undefined,
  };

  // Validate required fields
  if (!config.telegram_bot_token) {
    throw new Error(
      "TELEGRAM_BOT_TOKEN is not set. " +
      "Set it in ~/.config/fiochat/config.yaml or as an environment variable."
    );
  }

  if (!config.allowed_user_ids) {
    console.warn(
      "[WARN] ALLOWED_USER_IDS is not set. Bot will ignore all messages."
    );
  }

  return config;
}

/**
 * Simple YAML parser (supports basic key-value and nested objects)
 * Uses a minimal implementation to avoid external dependencies for now
 */
function parseYaml(content: string): any {
  const lines = content.split("\n");
  const result: any = {};
  let currentSection: any = result;
  let sectionStack: any[] = [result];
  let indentStack: number[] = [0];

  for (let line of lines) {
    // Skip comments and empty lines
    if (line.trim().startsWith("#") || line.trim() === "") continue;

    const indent = line.length - line.trimStart().length;
    const trimmed = line.trim();

    // Handle key-value pairs
    if (trimmed.includes(":")) {
      const colonIndex = trimmed.indexOf(":");
      const key = trimmed.substring(0, colonIndex).trim();
      let value = trimmed.substring(colonIndex + 1).trim();

      // Remove quotes
      if ((value.startsWith('"') && value.endsWith('"')) ||
          (value.startsWith("'") && value.endsWith("'"))) {
        value = value.slice(1, -1);
      }

      // Handle boolean and null
      if (value === "true") value = true as any;
      else if (value === "false") value = false as any;
      else if (value === "null") value = null as any;

      // Adjust section based on indent
      while (indentStack.length > 1 && indent <= indentStack[indentStack.length - 1]) {
        indentStack.pop();
        sectionStack.pop();
      }
      currentSection = sectionStack[sectionStack.length - 1];

      if (value === "" || value === null) {
        // This key starts a new section
        currentSection[key] = {};
        sectionStack.push(currentSection[key]);
        indentStack.push(indent);
        currentSection = currentSection[key];
      } else {
        currentSection[key] = value;
      }
    }
  }

  return result;
}
