# Light Beams protocol scenarios

Websocket scenarios for the game server in `src/games/server.rs`. Each one
creates a fresh room, drives scripted clients, and writes a normalized
transcript (tokens, room codes, random nicknames and computer-generated boards
masked) so two servers can be compared with `diff`.

`golden/` holds transcripts recorded against the 2026-10-11 production build,
the reference the rebuilt server was matched against.

```
pip install websockets
# server-fn URLs carry a hash of the crate path; pass yours if it differs
SFN_SUFFIX=<suffix> python3 scenarios.py 127.0.0.1:3000 runs [scenario ...]
diff -r golden runs
```

`compact.py <transcript>` prints a transcript with boards drawn as one glyph
per cell, which is easier to read.
