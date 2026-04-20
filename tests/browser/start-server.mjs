import { spawnSync } from "node:child_process";
import { randomUUID } from "node:crypto";
import { createReadStream, existsSync } from "node:fs";
import { stat } from "node:fs/promises";
import http from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..", "..");
const distDir = path.join(repoRoot, "target", "playwright-dist");
const distArg = path.join("target", "playwright-dist");
const port = Number.parseInt(process.env.PORT ?? "4173", 10);

buildUiBundle();

const sessions = new Map();

const server = http.createServer(async (req, res) => {
  try {
    const requestUrl = new URL(req.url ?? "/", `http://127.0.0.1:${port}`);
    const pathname = requestUrl.pathname;

    if (pathname.startsWith("/api/v1/")) {
      await handleApiRequest(req, res, pathname);
      return;
    }

    await serveStaticAsset(res, pathname);
  } catch (error) {
    res.writeHead(500, { "content-type": "text/plain; charset=utf-8" });
    res.end(
      error instanceof Error ? error.stack ?? error.message : "unknown server error",
    );
  }
});

server.listen(port, "127.0.0.1", () => {
  console.log(`Elowen UI browser automation server listening on http://127.0.0.1:${port}`);
});

function buildUiBundle() {
  const result =
    process.platform === "win32"
      ? spawnSync("cmd.exe", ["/d", "/s", "/c", `trunk build --dist ${distArg}`], {
          cwd: repoRoot,
          stdio: "inherit",
        })
      : spawnSync("trunk", ["build", "--dist", distArg], {
          cwd: repoRoot,
          stdio: "inherit",
        });

  if (result.status !== 0) {
    throw new Error(`trunk build failed with status ${result.status ?? "unknown"}`);
  }
}

async function handleApiRequest(req, res, pathname) {
  const session = getSessionFromCookie(req);

  if (pathname === "/api/v1/auth/session" && req.method === "GET") {
    writeJson(res, 200, session?.authenticated ? authenticatedSession(session) : anonymousSession());
    return;
  }

  if (pathname === "/api/v1/auth/login" && req.method === "POST") {
    const body = await readJson(req);
    const username = body?.username == null ? null : String(body.username);
    const password = String(body?.password ?? "");
    const scenario = credentialsToScenario(username, password);

    if (!scenario) {
      writeJson(res, 401, { error: "invalid username or password" });
      return;
    }

    const nextSession = createSession(scenario);
    sessions.set(nextSession.id, nextSession);
    res.setHeader(
      "set-cookie",
      `elowen_session=${nextSession.id}; Path=/; HttpOnly; SameSite=Lax`,
    );
    writeJson(res, 200, authenticatedSession(nextSession));
    return;
  }

  if (pathname === "/api/v1/auth/logout" && req.method === "POST") {
    if (session) {
      closeSessionClients(session);
      sessions.delete(session.id);
    }

    res.setHeader(
      "set-cookie",
      "elowen_session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0",
    );
    writeJson(res, 200, anonymousSession());
    return;
  }

  if (!session?.authenticated) {
    writeJson(res, 401, { error: "sign in required" });
    return;
  }

  if (pathname === "/api/v1/threads" && req.method === "POST" && !canOperate(session)) {
    writeJson(res, 403, { error: "the signed-in account is not allowed to perform this action" });
    return;
  }

  if (pathname === `/api/v1/threads/${session.state.thread.id}/chat` && req.method === "POST" && !canOperate(session)) {
    writeJson(res, 403, { error: "the signed-in account is not allowed to perform this action" });
    return;
  }

  if (pathname === `/api/v1/threads/${session.state.thread.id}/message-dispatch` && req.method === "POST" && !canOperate(session)) {
    writeJson(res, 403, { error: "the signed-in account is not allowed to perform this action" });
    return;
  }

  if (pathname === `/api/v1/threads/${session.state.thread.id}/jobs` && req.method === "POST" && !canOperate(session)) {
    writeJson(res, 403, { error: "the signed-in account is not allowed to perform this action" });
    return;
  }

  if (pathname === `/api/v1/jobs/${session.state.job.id}/notes/promote` && req.method === "POST" && !canOperate(session)) {
    writeJson(res, 403, { error: "the signed-in account is not allowed to perform this action" });
    return;
  }

  if (pathname === `/api/v1/approvals/${session.state.approval.id}/resolve` && req.method === "POST" && !canAdmin(session)) {
    writeJson(res, 403, { error: "the signed-in account is not allowed to perform this action" });
    return;
  }

  if (pathname === "/api/v1/threads" && req.method === "GET") {
    writeJson(res, 200, session.state.threads);
    return;
  }

  if (pathname === "/api/v1/jobs" && req.method === "GET") {
    writeJson(res, 200, session.state.jobs);
    return;
  }

  if (pathname === `/api/v1/threads/${session.state.thread.id}` && req.method === "GET") {
    writeJson(res, 200, session.state.thread);
    return;
  }

  if (pathname === `/api/v1/threads/${session.state.thread.id}/chat` && req.method === "POST") {
    const body = await readJson(req);
    const content = String(body?.content ?? "").trim();

    if (!content) {
      writeJson(res, 400, { error: "message content is required" });
      return;
    }

    const reply = appendChatExchange(session, content);
    writeJson(res, 201, reply);
    return;
  }

  if (pathname === `/api/v1/jobs/${session.state.job.id}` && req.method === "GET") {
    writeJson(res, 200, session.state.job);
    return;
  }

  if (pathname === `/api/v1/jobs/${session.state.job.id}/notes/promote` && req.method === "POST") {
    writeJson(res, 201, {
      note_id: "note-promoted",
      title: "Promoted job summary",
      slug: "promoted-job-summary",
      summary: "Promoted from the current job summary.",
      tags: ["job", "promoted"],
      aliases: [],
      note_type: "job-summary",
      source_kind: "job",
      source_id: session.state.job.id,
      current_revision_id: "rev-promoted",
      updated_at: session.state.job.updated_at,
    });
    return;
  }

  if (pathname === `/api/v1/approvals/${session.state.approval.id}/resolve` && req.method === "POST") {
    const body = await readJson(req);
    const status = String(body?.status ?? "").trim().toLowerCase();
    if (status !== "approved" && status !== "rejected") {
      writeJson(res, 400, { error: "approval status must be `approved` or `rejected`" });
      return;
    }

    session.state.approval = {
      ...session.state.approval,
      status,
      resolved_by: session.actor.username,
      resolved_by_display_name: session.actor.display_name,
      resolution_reason: String(body?.reason ?? ""),
      resolved_at: "2026-04-15T15:10:00Z",
      updated_at: "2026-04-15T15:10:00Z",
    };
    session.state.job.approvals = [structuredClone(session.state.approval)];
    writeJson(res, 200, session.state.approval);
    return;
  }

  if (pathname === "/api/v1/events" && req.method === "GET") {
    handleEventStream(req, res, session);
    return;
  }

  writeJson(res, 404, { error: `unknown endpoint: ${pathname}` });
}

function handleEventStream(req, res, session) {
  res.writeHead(200, {
    "content-type": "text/event-stream",
    "cache-control": "no-cache, no-transform",
    connection: "keep-alive",
  });
  res.write(": connected\n\n");

  session.eventClients.add(res);

  if (session.scenario === "realtime" && !session.realtimeDelivered) {
    session.realtimeDelivered = true;
    setTimeout(() => {
      applyRealtimeCompletion(session);
      broadcastEvent(session, {
        event_type: "job.changed",
        thread_id: session.state.thread.id,
        job_id: session.state.job.id,
        device_id: session.state.job.device_id,
        created_at: session.state.job.updated_at,
      });
    }, 1_200);
  }

  req.on("close", () => {
    session.eventClients.delete(res);
    res.end();
  });
}

function applyRealtimeCompletion(session) {
  const nextUpdatedAt = "2026-04-15T15:05:00Z";
  const completedMessage = {
    ...session.state.thread.messages.at(-1),
    content: "Browser automation summary ready. The UI shell now has deterministic browser coverage.",
    status: "job_event:job-slice-30:completed",
    payload_json: {
      job_result: {
        job_id: "job-slice-30",
        details: "Playwright browser checks passed for auth, mobile details, sticky composer, and realtime behavior.",
      },
    },
    created_at: nextUpdatedAt,
  };

  session.state = {
    threads: [
      {
        ...session.state.threads[0],
        updated_at: nextUpdatedAt,
      },
    ],
    thread: {
      ...session.state.thread,
      updated_at: nextUpdatedAt,
      messages: [...session.state.thread.messages.slice(0, -1), completedMessage],
      jobs: [
        {
          ...session.state.thread.jobs[0],
          status: "completed",
          result: "success",
          updated_at: nextUpdatedAt,
        },
      ],
    },
    jobs: [
      {
        ...session.state.jobs[0],
        status: "completed",
        result: "success",
        updated_at: nextUpdatedAt,
      },
    ],
    job: {
      ...session.state.job,
      status: "completed",
      result: "success",
      updated_at: nextUpdatedAt,
      execution_report_json: {
        build: { status: "success" },
        test: { status: "success" },
        diff_stat: "5 files changed, 124 insertions(+), 11 deletions(-)",
        changed_files: [
          "src/app.rs",
          "README.md",
          "playwright.config.mjs",
          "tests/browser/start-server.mjs",
          "tests/browser/ui-browser-automation.spec.mjs",
        ],
        git_status: ["M src/app.rs", "A playwright.config.mjs", "A tests/browser/start-server.mjs"],
        last_message: "Browser automation checks completed successfully.",
      },
      summary: {
        ...session.state.job.summary,
        created_at: nextUpdatedAt,
        content:
          "Verified auth, mobile details/backdrop behavior, sticky composer placement, and realtime job presentation with Playwright.",
      },
      events: [
        ...session.state.job.events,
        {
          id: "evt-job-completed",
          correlation_id: session.state.job.correlation_id,
          event_type: "job.completed",
          payload_json: {
            result: "success",
          },
          created_at: nextUpdatedAt,
        },
      ],
    },
    approval: session.state.approval,
  };
}

function broadcastEvent(session, event) {
  const payload = `data: ${JSON.stringify(event)}\n\n`;

  for (const client of session.eventClients) {
    client.write(payload);
  }
}

function closeSessionClients(session) {
  for (const client of session.eventClients) {
    client.end();
  }
  session.eventClients.clear();
}

function getSessionFromCookie(req) {
  const cookieHeader = req.headers.cookie ?? "";
  const sessionId = cookieHeader
    .split(";")
    .map((part) => part.trim())
    .find((part) => part.startsWith("elowen_session="))
    ?.split("=")[1];

  if (!sessionId) {
    return null;
  }

  return sessions.get(sessionId) ?? null;
}

function credentialsToScenario(username, password) {
  const normalizedUsername = username?.trim().toLowerCase() ?? null;

  if (normalizedUsername === "admin" && password === "slice30") {
    return {
      scenario: "default",
      actor: actor("admin", "Playwright Admin", "admin"),
      authMode: "local_accounts",
    };
  }

  if (normalizedUsername === "admin" && password === "slice30-created") {
    return {
      scenario: "created-only",
      actor: actor("admin", "Playwright Admin", "admin"),
      authMode: "local_accounts",
    };
  }

  if (normalizedUsername === "admin" && password === "slice30-realtime") {
    return {
      scenario: "realtime",
      actor: actor("admin", "Realtime Admin", "admin"),
      authMode: "local_accounts",
    };
  }

  if (normalizedUsername === "admin" && password === "slice31-draft") {
    return {
      scenario: "draft",
      actor: actor("admin", "Draft Admin", "admin"),
      authMode: "local_accounts",
    };
  }

  if (normalizedUsername === "operator" && password === "slice32-operator") {
    return {
      scenario: "draft",
      actor: actor("operator", "Operator User", "operator"),
      authMode: "local_accounts",
    };
  }

  if (normalizedUsername === "viewer" && password === "slice32-viewer") {
    return {
      scenario: "default",
      actor: actor("viewer", "Viewer User", "viewer"),
      authMode: "local_accounts",
    };
  }

  if (!normalizedUsername && password === "slice32-legacy") {
    return {
      scenario: "default",
      actor: actor("legacy-admin", "Legacy Admin", "admin"),
      authMode: "legacy_shared_password",
    };
  }

  return null;
}

function actor(username, displayName, role) {
  return {
    username,
    display_name: displayName,
    role,
  };
}

function createSession({ scenario, actor, authMode }) {
  const now = "2026-04-15T14:40:00Z";
  const isCreatedOnly = scenario === "created-only";
  const isDraft = scenario === "draft";
  const jobRecord = {
    id: isDraft ? "job-slice-31" : "job-slice-30",
    short_id: isDraft ? "job-031" : "job-030",
    correlation_id: isDraft ? "corr-slice-31" : "corr-slice-30",
    thread_id: isDraft ? "thread-slice-31" : "thread-slice-30",
    title: isDraft ? "Chat surface polish" : "Browser automation rollout",
    status: isCreatedOnly ? "probing" : isDraft ? "awaiting_approval" : "running",
    result: isDraft ? "success" : null,
    failure_class: null,
    repo_name: "elowen-ui",
    device_id: "laptop-edge-01",
    branch_name: isDraft
      ? "slice/31-chat-surface-and-draft-ux-polish"
      : "slice/30-ui-browser-automation",
    base_branch: "main",
    created_at: "2026-04-15T14:10:00Z",
    updated_at: now,
  };

  const state = {
    threads: [
      {
        id: isDraft ? "thread-slice-31" : "thread-slice-30",
        title: isDraft ? "Slice 31 draft polish" : "Slice 30 browser automation",
        status: "active",
        message_count: isDraft ? 17 : 15,
        updated_at: now,
      },
    ],
    thread: {
      id: isDraft ? "thread-slice-31" : "thread-slice-30",
      title: isDraft ? "Slice 31 draft polish" : "Slice 30 browser automation",
      status: "active",
      updated_at: now,
      messages: createThreadMessages(now, {
        includeJobMessage: !isCreatedOnly,
        includeDraftMessage: isDraft,
        usePolishedResult: isDraft,
      }),
      jobs: [structuredClone(jobRecord)],
      related_notes: [
        {
          note_id: isDraft ? "note-slice-31" : "note-slice-30",
          title: isDraft ? "Slice 31 polish checklist" : "Slice 30 acceptance checklist",
          slug: isDraft ? "slice-31-polish-checklist" : "slice-30-acceptance-checklist",
          summary: isDraft
            ? "Focus on draft clarity, result disclosure, timestamps, and composer shortcuts."
            : "Focus on auth, mobile layout, sticky composer, and realtime presentation.",
          tags: isDraft ? ["slice", "chat-polish"] : ["slice", "browser-automation"],
          aliases: [],
          note_type: "roadmap-checklist",
          source_kind: "roadmap",
          source_id: isDraft ? "slice-31" : "slice-30",
          current_revision_id: "rev-1",
          updated_at: now,
        },
      ],
    },
    jobs: [structuredClone(jobRecord)],
    job: {
      ...structuredClone(jobRecord),
      execution_report_json: isCreatedOnly
        ? {}
        : isDraft
          ? {
              build: { status: "success" },
              test: { status: "success" },
              diff_stat: "4 files changed, 96 insertions(+), 14 deletions(-)",
              changed_files: ["src/app/mod.rs", "src/format.rs", "public/app.css", "tests/browser/start-server.mjs"],
              git_status: ["M src/app/mod.rs", "M src/format.rs", "M public/app.css"],
              last_message:
                "Chat surface polish is ready for review. The transcript now separates activity from final results more clearly.",
            }
        : {
            build: { status: "running" },
            test: { status: "pending" },
            diff_stat: "3 files changed, 88 insertions(+), 9 deletions(-)",
            changed_files: ["src/app.rs", "README.md", "roadmap.md"],
            git_status: ["M src/app.rs"],
            last_message: "Browser suite scaffold is still being wired into the UI repo.",
          },
      summary: isCreatedOnly
        ? null
        : isDraft
          ? {
              id: "summary-slice-31",
              scope: "job",
              source_id: "job-slice-31",
              version: 1,
              content:
                "Polished the chat transcript, localized timestamps, and moved operational detail behind disclosure by default.",
              created_at: now,
            }
        : {
            id: "summary-slice-30",
            scope: "job",
            source_id: "job-slice-30",
            version: 1,
            content: "Initial browser automation scaffold is in progress.",
            created_at: now,
          },
      approvals: [],
      related_notes: [],
      events: isCreatedOnly
        ? [
            {
              id: "evt-job-created",
              correlation_id: "corr-slice-30",
              event_type: "job.created",
              payload_json: {
                repo_name: "elowen-ui",
                device_id: "laptop-edge-01",
                branch_name: "slice/30-ui-browser-automation",
                base_branch: "main",
                request_text: "Check whether any edge agents are currently available.",
              },
              created_at: now,
            },
          ]
        : [
            {
              id: isDraft ? "evt-job-awaiting-approval" : "evt-job-running",
              correlation_id: isDraft ? "corr-slice-31" : "corr-slice-30",
              event_type: isDraft ? "job.awaiting_approval" : "job.running",
              payload_json: isDraft
                ? {
                    summary: "Push approval is pending.",
                  }
                : {
                    phase: "browser-automation",
                  },
              created_at: now,
            },
          ],
    },
    approval: {
      id: "approval-slice-31",
      thread_id: isDraft ? "thread-slice-31" : "thread-slice-30",
      job_id: jobRecord.id,
      action_type: "push",
      status: "pending",
      summary: "Push approval is pending.",
      resolved_by: null,
      resolved_by_display_name: null,
      resolution_reason: null,
      created_at: now,
      resolved_at: null,
      updated_at: now,
    },
  };

  if (isDraft) {
    state.job.approvals = [structuredClone(state.approval)];
  }

  return {
    id: randomUUID(),
    authenticated: true,
    scenario,
    actor,
    authMode,
    eventClients: new Set(),
    realtimeDelivered: false,
    state,
  };
}

function createThreadMessages(
  now,
  { includeJobMessage = true, includeDraftMessage = false, usePolishedResult = false } = {},
) {
  const messages = [];

  for (let index = 0; index < 6; index += 1) {
    messages.push({
      id: `msg-user-${index}`,
      role: "user",
      content: `Operator note ${index + 1}: keep the composer anchored while the thread pane scrolls.`,
      status: "conversation.user",
      payload_json: {},
      created_at: `2026-04-15T14:${10 + index}:00Z`,
    });
    messages.push({
      id: `msg-assistant-${index}`,
      role: "assistant",
      content: `Assistant response ${index + 1}: selector and layout checks are staged for browser coverage.`,
      status: "conversation.reply",
      payload_json: {},
      created_at: `2026-04-15T14:${10 + index}:30Z`,
    });
  }

  if (includeJobMessage) {
    messages.push({
      id: usePolishedResult ? "msg-job-result" : "msg-job-update",
      role: "assistant",
      content: usePolishedResult
        ? "Chat surface polish is ready for review. The transcript now separates activity from final results more clearly."
        : "Runner is still applying the requested UI automation changes.",
      status: usePolishedResult
        ? "job_event:job-slice-31:awaiting_approval"
        : "job_event:job-slice-30:running",
      payload_json: {
        job_result: {
          job_id: usePolishedResult ? "job-slice-31" : "job-slice-30",
          details: usePolishedResult
            ? "Build: success\nTest: success\nChanged entries: 4\n\nPush approval is pending while the final branch waits for review."
            : "Waiting for the browser automation harness to finish its realtime verification pass.",
        },
      },
      created_at: now,
    });
  }

  if (includeDraftMessage) {
    messages.push({
      id: "msg-draft-ready",
      role: "assistant",
      content:
        "I stayed in conversational mode and prepared a clean dispatch handoff below so you can review it before dispatching.",
      status: "conversation.reply",
      payload_json: {
        execution_draft: {
          title: "Polish chat transcript surfaces",
          repo_name: "elowen-ui",
          base_branch: "main",
          request_text:
            "Tighten the chat transcript, localize timestamps, keep operational result details behind disclosure by default, and preserve the pinned composer behavior.",
          execution_intent: "workspace_change",
          source_message_id: "msg-user-5",
          source_role: "user",
          rationale: "Prepared from the latest user request so it can be reviewed before dispatch.",
        },
      },
      created_at: "2026-04-15T14:42:00Z",
    });
  }

  return messages;
}

function appendChatExchange(session, content) {
  const nextUpdatedAt = "2026-04-15T14:55:00Z";
  const userMessage = {
    id: `msg-user-${session.state.thread.messages.length + 1}`,
    role: "user",
    content,
    status: "conversation.user",
    payload_json: {},
    created_at: nextUpdatedAt,
  };
  const assistantMessage = {
    id: `msg-assistant-${session.state.thread.messages.length + 2}`,
    role: "assistant",
    content:
      "I stayed in conversational mode and prepared a clean dispatch handoff below so you can refine it before dispatching.",
    status: "conversation.reply",
    payload_json: {
      execution_draft: {
        title: "Polish transcript timestamps",
        repo_name: "elowen-ui",
        base_branch: "main",
        request_text: content,
        execution_intent: "workspace_change",
        source_message_id: userMessage.id,
        source_role: "user",
        rationale: "Prepared from the latest user request so it can be reviewed before dispatch.",
      },
    },
    created_at: nextUpdatedAt,
  };

  session.state.thread = {
    ...session.state.thread,
    updated_at: nextUpdatedAt,
    messages: [...session.state.thread.messages, userMessage, assistantMessage],
  };
  session.state.threads = session.state.threads.map((thread) =>
    thread.id === session.state.thread.id
      ? {
          ...thread,
          updated_at: nextUpdatedAt,
          message_count: thread.message_count + 2,
        }
      : thread,
  );

  return {
    user_message: userMessage,
    assistant_message: assistantMessage,
  };
}

function authenticatedSession(session) {
  return {
    enabled: true,
    auth_mode: session.authMode,
    authenticated: true,
    actor: session.actor,
    permissions: permissionsForRole(session.actor.role),
  };
}

function anonymousSession() {
  return {
    enabled: true,
    auth_mode: "local_accounts",
    authenticated: false,
    actor: null,
    permissions: [],
  };
}

function permissionsForRole(role) {
  switch (role) {
    case "admin":
      return ["view", "operate", "admin"];
    case "operator":
      return ["view", "operate"];
    default:
      return ["view"];
  }
}

function canOperate(session) {
  return session.actor.role === "operator" || session.actor.role === "admin";
}

function canAdmin(session) {
  return session.actor.role === "admin";
}

function writeJson(res, statusCode, payload) {
  res.writeHead(statusCode, {
    "content-type": "application/json; charset=utf-8",
  });
  res.end(JSON.stringify(payload));
}

async function readJson(req) {
  const chunks = [];

  for await (const chunk of req) {
    chunks.push(chunk);
  }

  if (chunks.length === 0) {
    return {};
  }

  return JSON.parse(Buffer.concat(chunks).toString("utf-8"));
}

async function serveStaticAsset(res, pathname) {
  const relativePath = pathname === "/" ? "index.html" : pathname.replace(/^\/+/, "");
  let filePath = path.join(distDir, relativePath);

  if (!existsSync(filePath)) {
    filePath = path.join(distDir, "index.html");
  }

  const fileStat = await stat(filePath);
  const extension = path.extname(filePath);

  res.writeHead(200, {
    "content-type": contentTypeForExtension(extension),
    "content-length": fileStat.size,
  });

  createReadStream(filePath).pipe(res);
}

function contentTypeForExtension(extension) {
  switch (extension) {
    case ".css":
      return "text/css; charset=utf-8";
    case ".html":
      return "text/html; charset=utf-8";
    case ".js":
      return "application/javascript; charset=utf-8";
    case ".json":
      return "application/json; charset=utf-8";
    case ".wasm":
      return "application/wasm";
    default:
      return "application/octet-stream";
  }
}
