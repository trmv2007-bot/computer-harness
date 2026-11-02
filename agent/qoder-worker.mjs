import {
  createSdkMcpServer,
  qodercliAuth,
  query,
  shutdownDaemon,
  tool,
} from "@qoder-ai/qoder-agent-sdk";
import { z } from "zod";
import net from "node:net";
import { readFile } from "node:fs/promises";
import { join } from "node:path";

const MAX_RESPONSE_BYTES = 8 * 1024 * 1024;
const REQUEST_TIMEOUT_MS = 5_000;
const MAX_PROMPT_BYTES = 32 * 1024;

function emit(type, fields = {}) {
  process.stdout.write(`${JSON.stringify({ type, ...fields })}\n`);
}

function requireEnv(name) {
  const value = process.env[name];
  if (!value) throw new Error(`Missing required environment variable: ${name}`);
  return value;
}

async function readInput() {
  let raw = "";
  for await (const chunk of process.stdin) {
    raw += chunk.toString("utf8");
    if (Buffer.byteLength(raw, "utf8") > MAX_PROMPT_BYTES + 1024) {
      throw new Error("Agent request exceeds the 32 KiB prompt limit");
    }
  }
  const input = JSON.parse(raw);
  if (typeof input.prompt !== "string" || input.prompt.trim().length === 0) {
    throw new Error("Agent request must contain a non-empty prompt");
  }
  if (Buffer.byteLength(input.prompt, "utf8") > MAX_PROMPT_BYTES) {
    throw new Error("Prompt exceeds the 32 KiB limit");
  }
  return input;
}

async function createIpcClient() {
  const dataDir = requireEnv("FAIRY_DATA_DIR");
  const endpoint = (await readFile(join(dataDir, "ipc-endpoint"), "utf8")).trim();
  const token = (await readFile(join(dataDir, "ipc-token"), "utf8")).trim();

  const match = /^127\.0\.0\.1:(\d+)$/.exec(endpoint);
  if (!match) throw new Error("Fairy IPC endpoint is not a loopback IPv4 address");
  const port = Number(match[1]);
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    throw new Error("Fairy IPC endpoint port is invalid");
  }
  if (token.length < 32) throw new Error("Fairy IPC token is invalid");

  return (action) => new Promise((resolve, reject) => {
    const socket = net.createConnection({ host: "127.0.0.1", port });
    let buffer = "";
    let settled = false;

    const finish = (error, result) => {
      if (settled) return;
      settled = true;
      socket.destroy();
      if (error) reject(error);
      else resolve(result);
    };

    socket.setTimeout(REQUEST_TIMEOUT_MS, () => {
      finish(new Error("Timed out waiting for Fairy control plane"));
    });
    socket.once("error", (error) => finish(error));
    socket.once("connect", () => {
      socket.write(`${JSON.stringify({ token, action })}\n`);
    });
    socket.on("data", (chunk) => {
      buffer += chunk.toString("utf8");
      if (Buffer.byteLength(buffer, "utf8") > MAX_RESPONSE_BYTES) {
        finish(new Error("Fairy IPC response exceeds the 8 MiB limit"));
        return;
      }
      const newline = buffer.indexOf("\n");
      if (newline < 0) return;
      try {
        const response = JSON.parse(buffer.slice(0, newline));
        if (!response.ok) {
          finish(new Error(response.message || "Fairy tool request was denied"));
        } else {
          finish(null, response.data);
        }
      } catch (error) {
        finish(error);
      }
    });
  });
}

function wrapToolFailure(error) {
  return {
    isError: true,
    content: [{ type: "text", text: error instanceof Error ? error.message : String(error) }],
  };
}

async function main() {
  const input = await readInput();
  const workspaceRoot = requireEnv("FAIRY_WORKSPACE_ROOT");
  const ipc = await createIpcClient();

  const listTool = tool(
    "filesystem_list",
    "List files and directories inside Fairy's configured workspace. Paths must be workspace-relative. This is read-only.",
    { path: z.string().optional().describe("Workspace-relative directory; use '.' for the workspace root") },
    async ({ path }) => {
      try {
        const data = await ipc({
          action: "execute_tool",
          name: "filesystem.list",
          input_json: JSON.stringify({ path: path ?? "." }),
          approval_id: null,
        });
        return { content: [{ type: "text", text: JSON.stringify(data) }] };
      } catch (error) {
        return wrapToolFailure(error);
      }
    },
    { annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false } },
  );

  const readTextTool = tool(
    "filesystem_read_text",
    "Read a UTF-8 text file inside Fairy's configured workspace (maximum 1 MiB). Paths must be workspace-relative. This is read-only.",
    { path: z.string().describe("Workspace-relative text file path") },
    async ({ path }) => {
      try {
        const data = await ipc({
          action: "execute_tool",
          name: "filesystem.read_text",
          input_json: JSON.stringify({ path }),
          approval_id: null,
        });
        return { content: [{ type: "text", text: JSON.stringify(data) }] };
      } catch (error) {
        return wrapToolFailure(error);
      }
    },
    { annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false } },
  );

  const server = createSdkMcpServer({
    name: "fairy",
    version: "0.1.0",
    tools: [listTool, readTextTool],
  });

  const q = query({
    prompt: input.prompt,
    options: {
      auth: qodercliAuth(),
      cwd: workspaceRoot,
      systemPrompt: [
        "You are Fairy, the computer companion in Computer Harness.",
        "Be honest, precise, proactive within the provided capabilities, and clear about uncertainty.",
        "Only the Fairy MCP tools are available. Do not claim to have used a capability unless its tool call succeeded.",
        "The available tools are read-only and restricted to the configured workspace. You cannot edit files, run shell commands, browse the web, or contact external services.",
        "Do not request secrets. Do not infer that Qoder inference is unlimited or free; report quota information only when the SDK supplies it.",
      ].join("\n"),
      tools: [
        "mcp__fairy__filesystem_list",
        "mcp__fairy__filesystem_read_text",
      ],
      allowedTools: [
        "mcp__fairy__filesystem_list",
        "mcp__fairy__filesystem_read_text",
      ],
      disallowedTools: ["Bash", "Edit", "Write", "Agent", "WebFetch", "WebSearch"],
      permissionMode: "dontAsk",
      maxTurns: 8,
      mcpServers: { fairy: server },
    },
  });

  let finalResult = null;
  try {
    for await (const message of q) {
      if (message.type === "assistant") {
        for (const block of message.message.content) {
          if (block.type === "text") emit("text", { text: block.text });
          else if (block.type === "tool_use") emit("tool", { name: block.name });
        }
      } else if (message.type === "result") {
        finalResult = message;
        emit("result", {
          subtype: message.subtype,
          result: typeof message.result === "string" ? message.result : undefined,
          isError: message.subtype === "success" ? Boolean(message.is_error) : true,
          errors: Array.isArray(message.errors) ? message.errors : undefined,
          errorCode: message.error_code,
          totalCredits: message.total_credits,
        });
      }
    }

    if (!finalResult) {
      throw new Error("Qoder SDK stream ended without a final result message");
    }
    if (finalResult.subtype !== "success" || finalResult.is_error) {
      const errors = Array.isArray(finalResult.errors) ? finalResult.errors : [];
      const detail = errors.length > 0
        ? errors.join("; ")
        : (typeof finalResult.result === "string" ? finalResult.result : "Qoder reported an unsuccessful result");
      emit("error", {
        message: detail,
        errorCode: finalResult.error_code,
        subtype: finalResult.subtype,
      });
      process.exitCode = 2;
    }

    const usage = await q.getUsageInfo().catch(() => null);
    if (usage) {
      emit("usage", {
        userType: usage.userType,
        totalUsagePercentage: usage.totalUsagePercentage,
        isQuotaExceeded: usage.isQuotaExceeded,
        userQuota: usage.userQuota
          ? {
              remaining: usage.userQuota.remaining,
              total: usage.userQuota.total,
              unit: usage.userQuota.unit,
            }
          : undefined,
        addOnQuota: usage.addOnQuota
          ? {
              remaining: usage.addOnQuota.remaining,
              total: usage.addOnQuota.total,
              unit: usage.addOnQuota.unit,
            }
          : undefined,
        session: usage.session
          ? { total_credits: usage.session.total_credits }
          : undefined,
      });
    } else {
      emit("usage", { available: false });
    }
  } finally {
    await q.close().catch(() => {});
    await shutdownDaemon().catch(() => {});
  }
}

main().catch((error) => {
  emit("error", { message: error instanceof Error ? error.message : String(error) });
  process.exitCode = 1;
});
