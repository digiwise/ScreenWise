# Screenpipe MCP Server

> **Parked in ScreenWise (owner policy, 2026-10-09).** This external-client MCP
> bridge is excluded from builds, testing, packaging and deployment candidates.
> Preserve the source; do not execute the retained setup/build/test instructions
> below or install its dependencies as part of ordinary repository work.
> Codex and Claude Code must explain a concrete reason and prompt the owner before
> revisiting this policy. See the authoritative [repository instructions](../../AGENTS.md#external-client-mcp-server--parked-2026-10-09).
> Pi's `screenwise_recordings` extension and direct recorder HTTP API remain active
> and do not require this MCP server. The remainder is retained reference material.

<a href="https://www.pulsemcp.com/servers/screenpipe-screenpipe"><img src="https://www.pulsemcp.com/badge/top-pick/screenpipe-screenpipe" width="400" alt="PulseMCP Badge"></a>

<br/>

https://github.com/user-attachments/assets/7466a689-7703-4f0b-b3e1-b1cb9ed70cff

MCP server for screenpipe - search your screen recordings, audio transcriptions, and control your computer with AI.

## Installation

### Option 1: NPX (Recommended)

The easiest way to use screenpipe-mcp is with npx. Edit your Claude Desktop config:

- **macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`
- **Windows**: `%AppData%\Claude\claude_desktop_config.json`

```json
{
  "mcpServers": {
    "screenpipe": {
      "command": "npx",
      "args": ["-y", "screenpipe-mcp"]
    }
  }
}
```

### Option 2: Authenticated Local HTTP Server

The MCP server can run over HTTP using the Streamable HTTP transport for local
clients which cannot launch a stdio child process. It always binds to
`127.0.0.1`, and every request requires the recorder's local API bearer token.

```bash
# loopback-only and authenticated
npx -y screenpipe-mcp --http --port 3031 --api-key <local-api-key>

# or from source — must build first so dist/ exists
bun install && bun run build
bun run start:http -- --port 3031 --api-key <local-api-key>
```

> Tip: `npx screenpipe-mcp-http` (without `--http`) does **not** work —
> npm resolves by package name, and there is no `screenpipe-mcp-http`
> package. The HTTP server ships as a transport inside the
> `screenpipe-mcp` package; use `--http` as shown above, or invoke the
> bin directly with `npx -p screenpipe-mcp screenpipe-mcp-http`.

The server exposes:
- **MCP endpoint**: `http://localhost:3031/mcp` — Streamable HTTP transport (POST for requests, GET for SSE stream)
- **Health check**: `http://localhost:3031/health` — authenticated like every other endpoint

**Options:**
| Flag | Description | Default |
|------|-------------|---------|
| `--port` | Port for the MCP HTTP server | `3031` |
| `--screenpipe-port` | Port where screenpipe API is running | `3030` |
| `--api-key <key>` | Local API bearer token required by both the wrapper and recorder | required |

**Connecting a local HTTP MCP client:**

Point any MCP client that supports HTTP transport at the `/mcp` endpoint:

```json
{
  "mcpServers": {
    "screenpipe": {
      "url": "http://127.0.0.1:3031/mcp",
      "headers": {
        "Authorization": "Bearer <local-api-key>"
      }
    }
  }
}
```

> **Note:** The HTTP server currently exposes `search_content` only. The stdio server has the full tool set (export-video, list-meetings, activity-summary, search-elements, frame-context). We're working on bringing HTTP to full parity.

### Option 3: From Source

Clone and build from source:

```bash
git clone https://github.com/screenpipe/screenpipe
cd screenpipe/packages/screenpipe-mcp
bun install
bun run build
```

Then configure Claude Desktop:

```json
{
  "mcpServers": {
    "screenpipe": {
      "command": "node",
      "args": ["/absolute/path/to/screenpipe-mcp/dist/index.js"]
    }
  }
}
```

**Note:** Restart Claude Desktop after making changes.

## Testing

Test with MCP Inspector:

```bash
npx @modelcontextprotocol/inspector npx screenpipe-mcp
```

## Transport Modes

| Mode | Command | Use Case |
|------|---------|----------|
| **stdio** (default) | `npx screenpipe-mcp` | Claude Desktop, local MCP clients |
| **HTTP** | `npx screenpipe-mcp --http --api-key <key>` | Authenticated clients on the same machine |

## Available Tools

### search-content
Search through recorded content with content type filtering:
- `all` — OCR + Audio + Accessibility (default)
- `ocr` — Screen text from screenshots
- `audio` — Audio transcriptions
- `input` — User actions (clicks, keystrokes, clipboard, app switches)
- `accessibility` — Accessibility tree text
- Time range, app/window, and speaker filtering
- Pagination support

### export-video
Export screen recordings as video files:
- Specify time range with start/end times
- Configurable FPS for output video

### activity-summary
Get a lightweight compressed activity overview for a time range:
- App usage with active minutes and frame counts
- Recent accessibility texts
- Audio speaker summary

### list-meetings
List detected meetings with duration, app, and attendees. Pass `q` to filter by substring (title, attendees, notes).

### search-elements
Search structured UI elements (accessibility tree nodes and OCR text blocks):
- Filter by source, role, app, time range
- Much lighter than search-content for targeted UI lookups

### frame-context
Get accessibility text, parsed tree nodes, and extracted URLs for a specific frame.

## Example Queries in Claude

- "Search for any mentions of 'rust' in my screen recordings"
- "Find audio transcriptions from the last hour"
- "Show me what was on my screen in VSCode yesterday"
- "Export a video of my screen from 2-3pm today"
- "Find what John said in our meeting about the database"
- "What did I type in Slack today?" (uses content_type=input)
- "What did I copy to clipboard recently?" (uses content_type=input)
- "Show me accessibility text from Chrome" (uses content_type=accessibility)

## Requirements

- screenpipe must be running on localhost:3030
- Node.js >= 18.0.0

## Notes

- All timestamps are handled in UTC
- Results are formatted for readability in Claude's interface
- macOS automation features require accessibility permissions

## Privacy Policy

The Screenpipe MCP server is a local-only bridge between Claude and your
local Screenpipe instance. It does not collect, transmit, or store any
data on its own.

### What this MCP server does
When Claude invokes a tool (`search-content`, `activity-summary`, etc.)
the MCP server forwards the request to `http://localhost:3030` — the
Screenpipe daemon running on your machine — and returns the response.
That's the entire data path.

### Data collection
**None by this MCP server.** No analytics, no telemetry, no usage tracking.

### Data usage
Tool calls are passed straight through to your local Screenpipe daemon
and the results stream back to Claude. The MCP server doesn't keep
anything.

### Data storage
Nothing is stored by the MCP server itself. Recordings, OCR text,
audio transcripts, and UI events are stored by the Screenpipe app in a
SQLite database under `~/.screenpipe/` on your device. Retention is
whatever you configure inside the Screenpipe app — typically you
control it via the storage settings panel.

### Third-party sharing
None. The MCP server only talks to `localhost:3030`. It does not
contact Anthropic, Screenpipe's servers, or any other external service.
If you choose to enable optional cloud features inside the Screenpipe
app itself (e.g. cloud sync, cloud AI), those are governed by the
Screenpipe app's privacy policy, not this MCP server's data flow.

### Retention
The MCP server has no persistent state. The data your Screenpipe app
captures is retained according to your Screenpipe storage configuration
and is deletable at any time (`rm -rf ~/.screenpipe` removes everything).

### Source code
The Screenpipe MCP server is MIT-licensed and the entire source is
public at <https://github.com/screenpipe/screenpipe/tree/main/packages/screenpipe-mcp>.
Every line is auditable.

### Contact
Questions or concerns: open an issue at
<https://github.com/screenpipe/screenpipe/issues> or reach out via
<https://screenpi.pe>.
