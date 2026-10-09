# Local development listener boundaries (2026-10-09)

All local APIs, development/preview servers, WebSocket/HMR endpoints and test
fixtures default to explicit numeric loopback. See [AGENTS.md](../AGENTS.md#loopback-listeners--development-and-tests-too).
A firewall prompt is not a reason to permit Node or alter firewall rules.

The Next development command now specifies `--hostname 127.0.0.1`. Both Vitest
configurations specify loopback and disable runner APIs, HMR and the standalone
WebSocket server. The MCP export fixture binds and connects to `127.0.0.1`.

Installed Vite 5.4.21 source shows that `hmr: false` alone can still construct a
standalone WebSocket listener with an unspecified host. `server.ws: false`
disables that path. The frontend regression loads the real installed Vite
configuration while intercepting `net.Server.listen` before setup, so a regression
fails without opening even a temporary wildcard listener. Two checks passed.
The MCP package dependencies were absent; its changed configuration/fixture were
source-reviewed, without acquiring dependencies or claiming its suite passed.

## Reported firewall prompt: evidence and limit

The owner cancelled a Node.js firewall prompt during this implementation.
Recorded execution shows a dashboard Vitest run starting at 12:19:37 AEDT and
lasting 35.01 seconds, overlapping the requested 12:17:28–12:20:28 investigation
window on 9 October 2026. It ran six dashboard/status/monitor test files with
`--maxWorkers 1 --minWorkers 1` using the pre-fix Vitest configuration; 61 tests
passed. The same tool invocation reported Node.js v24.16.0. A separate diagnostic
reason test began at 12:21:09, outside that window. No development or preview
server command was used for either run.

A later read-only inventory found inbound block rules for an fnm-managed Node
24.16.0 executable and no remaining task Node process/listener. A later escalated
Node check resolved an fnm runtime; earlier command discovery had returned an nvm
symlink. These are different observations, not proof of the first process's exact
executable. No matching recent firewall rule-creation event was available.

The overlapping Vitest run and installed Vite mechanism support a plausible
attribution. The original PID, canonical executable and listening address were not
recorded, and the popup has no matched process event. The exact trigger therefore
remains unconfirmed. No listener was recreated to investigate it, no firewall rule
was modified, and this evidence makes no claim about outbound traffic.