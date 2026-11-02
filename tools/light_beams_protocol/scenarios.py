"""Protocol scenarios for the Light Beams server.

usage: scenarios.py <host:port> <outdir> [scenario ...]

Each scenario runs in a fresh room and writes a normalized transcript to
<outdir>/<name>.txt so two servers can be compared with `diff -r`.
"""
import asyncio, copy, json, os, re, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import wsprobe
from wsprobe import CLASSIC

HOST, OUT = sys.argv[1], sys.argv[2]
wsprobe.BASE = HOST
ONLY = set(sys.argv[3:])


def rules(**kw):
    r = copy.deepcopy(CLASSIC)
    for k, v in kw.items():
        if k in r["counts"]:
            r["counts"][k] = v
        else:
            r[k] = v
    return r


def board(size, cells):
    """cells: {(row, col): piece}"""
    b = [None] * (size * size)
    for (r, c), p in cells.items():
        b[r * size + c] = p
    return {"size": size, "cells": b}


F, B = {"Slash": "Forward"}, {"Slash": "Backward"}
# Two distinct valid classic boards.
BOARD_A = board(4, {(0, 0): "Red", (0, 3): "Yellow", (1, 1): "Blue", (2, 2): "Black",
                    (1, 2): F, (3, 0): B, (3, 3): {"Arrow": "UpLeft"}})
BOARD_B = board(4, {(0, 1): "Red", (1, 0): "Yellow", (2, 3): "Blue", (3, 2): "Black",
                    (0, 2): B, (2, 1): F, (1, 3): {"Arrow": "DownRight"}})
WRONG_FOR_A = board(4, {(0, 0): "Yellow", (0, 3): "Red", (1, 1): "Blue", (2, 2): "Black",
                        (1, 2): F, (3, 0): B, (3, 3): {"Arrow": "UpLeft"}})


def msg(t, **data):
    return {"type": t, "data": data} if data else {"type": t}


def S(who, t, **data):
    return ("send", who, msg(t, **data))


LOBBY2 = [("conn", "A"), ("conn", "B")]
START = [S("A", "StartGame")]
LOCK = [S("A", "LockBoard", board=BOARD_A), S("B", "LockBoard", board=BOARD_B)]

SCENARIOS = {
    # ── lobby / room plumbing ────────────────────────────────────────────────
    "lobby_basic": (CLASSIC, LOBBY2 + [
        S("A", "SetNickname", name="Alice"), S("B", "SetNickname", name="  "),
        S("B", "SetNickname", name="X" * 40), S("B", "StartGame"),
        ("close", "B"), ("wait", 0.3), ("conn", "C"), ("conn", "D")]),
    "solo_start": (CLASSIC, [("conn", "A"), S("A", "StartGame")]),
    "host_leaves": (CLASSIC, LOBBY2 + [("close", "A"), ("wait", 0.3), S("B", "StartGame"), ("conn", "C")]),
    "lobby_reconnect": (CLASSIC, LOBBY2 + [("close", "B"), ("conn", "B", {"token": "@"})]),
    "bad_token": (CLASSIC, LOBBY2 + [("close", "B"), ("conn", "B", {"token": "nope"})]),
    "computer_lobby3": (rules(source="Computer"), LOBBY2 + [("conn", "C"), S("C", "SetNickname", name="Cy")]),
    "wrong_passcode": (CLASSIC, [("badpass", "A")]),
    "no_room": (CLASSIC, [("noroom", "A")]),

    # ── classic versus live ──────────────────────────────────────────────────
    "versus_setup": (CLASSIC, LOBBY2 + START + [
        S("A", "LockBoard", board=board(4, {(0, 0): "Red"})),
        S("A", "ShootBeam", port=1),
        S("A", "LockBoard", board=BOARD_A),
        S("A", "LockBoard", board=BOARD_A),
        ("close", "A"), ("conn", "A", {"token": "@"}),
        S("B", "LockBoard", board=BOARD_B)]),
    "versus_play": (CLASSIC, LOBBY2 + START + LOCK + [
        S("B", "ShootBeam", port=3),
        S("A", "ShootBeam", port=0), S("A", "ShootBeam", port=17),
        S("A", "ShootBeam", port=1),
        S("A", "ShootBeam", port=2),
        S("B", "RequestRotate", row=9, col=0),
        S("B", "RequestRotate", row=0, col=2),
        S("B", "RequestRotate", row=1, col=2),
        S("B", "ShootBeam", port=5),
        S("A", "RequestRotate", row=0, col=0),
        S("B", "RequestRotate", row=1, col=2),
        S("B", "ShootBeam", port=7),
        S("A", "CallGame", guess=BOARD_A),
        S("A", "CallGame", guess=WRONG_FOR_A),
        ("close", "A"), ("conn", "A", {"token": "@"}),
        S("B", "CallGame", guess=WRONG_FOR_A),
    ]),
    "versus_win": (CLASSIC, LOBBY2 + START + LOCK + [
        S("A", "ShootBeam", port=4),
        S("B", "CallGame", guess=BOARD_A),
        ("close", "B"), ("conn", "B", {"token": "@"}),
        S("A", "PlayAgain"), S("B", "PlayAgain"),
        ("conn", "C"),
    ]),
    "versus_rotated_win": (CLASSIC, LOBBY2 + START + LOCK + [
        S("A", "RequestRotate", row=0, col=2),
        S("A", "CallGame", guess=board(4, {(0, 1): "Red", (1, 0): "Yellow", (2, 3): "Blue", (3, 2): "Black",
                                           (0, 2): F, (2, 1): F, (1, 3): {"Arrow": "DownRight"}})),
    ]),
    "versus_leave_midgame": (CLASSIC, LOBBY2 + START + LOCK + [
        S("A", "ShootBeam", port=6), ("close", "B"), ("wait", 0.3), ("conn", "C"),
        S("A", "ShootBeam", port=6)]),
    "join_in_progress": (CLASSIC, LOBBY2 + START + [("conn", "C")]),

    # ── independent engine ───────────────────────────────────────────────────
    "versus_async": (rules(resolution="Async"), LOBBY2 + START + LOCK + [
        S("A", "ShootBeam", port=1), S("A", "ShootBeam", port=2), S("B", "ShootBeam", port=3),
        S("A", "RequestRotate", row=0, col=2), S("A", "RequestRotate", row=0, col=2),
        S("A", "ShootBeam", port=8), S("A", "RequestRotate", row=3, col=3),
        S("B", "CallGame", guess=WRONG_FOR_A),
        ("close", "A"), ("conn", "A", {"token": "@"}),
        S("B", "CallGame", guess=BOARD_A),
        S("B", "ShootBeam", port=1),
        S("A", "CallGame", guess=board(4, {(0, 1): "Red", (1, 0): "Yellow", (2, 3): "Blue", (3, 2): "Black",
                                           (0, 2): F, (2, 1): F, (1, 3): {"Arrow": "DownRight"}})),
        ("close", "B"), ("conn", "B", {"token": "@"}),
        S("A", "PlayAgain"),
    ]),
    "versus_async_timed": (rules(resolution="Async", timed=True), LOBBY2 + START + LOCK + [
        S("B", "CallGame", guess=BOARD_A), S("A", "CallGame", guess=BOARD_B)]),
    "versus_async_quit": (rules(resolution="Async"), LOBBY2 + START + LOCK + [
        S("B", "CallGame", guess=BOARD_A), ("close", "A"), ("wait", 0.3)]),
    "computer_live3": (rules(source="Computer"), LOBBY2 + [("conn", "C")] + START + [
        S("A", "LockBoard", board=BOARD_A),
        S("B", "ShootBeam", port=1), S("B", "ShootBeam", port=1),
        S("C", "CallGame", guess=WRONG_FOR_A),
        S("C", "RequestRotate", row=0, col=0),
        ("close", "B"), ("conn", "B", {"token": "@"}),
        ("close", "C"), ("wait", 0.3), ("close", "A"), ("wait", 0.3)]),
    "computer_async2": (rules(source="Computer", resolution="Async", size=5, four_arrow=1),
                        LOBBY2 + START + [S("A", "ShootBeam", port=20), S("A", "ShootBeam", port=21),
                                          ("close", "A"), ("conn", "A", {"token": "@"})]),
    # One hidden black piece: a client can find it by guessing every cell.
    "computer_live_race": (rules(source="Computer", red=0, yellow=0, blue=0, black=1, slash=0, arrow=0),
                           LOBBY2 + [("conn", "C")] + START + [S("A", "ShootBeam", port=1), ("solve", "B"),
                           S("A", "ShootBeam", port=2), S("C", "PlayAgain"), S("A", "PlayAgain")]),
    "computer_async_finish": (rules(source="Computer", resolution="Async", red=0, yellow=0, blue=0, black=1, slash=0, arrow=0),
                              LOBBY2 + [("conn", "C")] + START + [S("C", "ShootBeam", port=3), ("solve", "A"),
                              ("close", "A"), ("conn", "A", {"token": "@"}), ("solve", "C"), ("close", "B"), ("wait", 0.3),
                              S("A", "PlayAgain")]),
    "computer_async_timed": (rules(source="Computer", resolution="Async", timed=True, red=0, yellow=0, blue=0, black=1, slash=0, arrow=0),
                             LOBBY2 + START + [("wait", 1.2), ("solve", "B"), ("wait", 1.1), ("solve", "A")]),
    "computer_live_timed": (rules(source="Computer", timed=True, red=0, yellow=0, blue=0, black=1, slash=0, arrow=0),
                            LOBBY2 + START + [("wait", 1.2), ("solve", "B")]),
    "rooms_over_join": (CLASSIC, LOBBY2 + [("rooms",)] + START + [("rooms",)] + LOCK + [
        S("B", "ShootBeam", port=2), S("A", "CallGame", guess=BOARD_B), ("rooms",), ("conn", "C"),
        ("close", "B"), ("rooms",), S("A", "PlayAgain"), ("rooms",)]),
    "independent_over_resync": (rules(resolution="Async"), LOBBY2 + START + LOCK + [
        S("A", "ShootBeam", port=5), S("B", "ShootBeam", port=6), S("A", "CallGame", guess=BOARD_B),
        S("B", "CallGame", guess=BOARD_A), ("close", "A"), ("conn", "A", {"token": "@"}), ("rooms",)]),
    "prune_after_playagain": (CLASSIC, LOBBY2 + START + LOCK + [
        S("A", "CallGame", guess=BOARD_B), ("close", "B"), S("A", "PlayAgain"), ("wait", 31), ("rooms",),
        ("conn", "C"), ("close", "C"), ("conn", "B", {"token": "@"})]),
    "prune_ingame_then_lobby": (CLASSIC, LOBBY2 + START + [("close", "B"), ("wait", 31), ("conn", "C"),
        ("conn", "B", {"token": "@"})]),
}


NICK_RE = re.compile(r'"nickname":"([^"]*)"')


class Normalizer:
    def __init__(self, room, random=False):
        self.room, self.maps, self.random = room, {}, random

    def tag(self, kind, val):
        m = self.maps.setdefault(kind, {})
        if val not in m:
            m[val] = f"<{kind}{len(m)}>"
        return m[val]

    def __call__(self, line):
        line = line.replace(self.room, "<room>")
        line = re.sub(r'"token":"[0-9a-f]{16}"', '"token":"<tok>"', line)
        line = re.sub(r"token=[0-9a-f]{16}", "token=<tok>", line)

        def nick(m):
            n = m.group(1)
            return m.group(0) if n in ("Alice", "Cy") or n.startswith("X") else f'"nickname":"{self.tag("nick", n)}"'
        line = NICK_RE.sub(nick, line)
        if self.random:
            # Computer-generated boards are random, and so is everything that
            # depends on them: exits, colours, answers, guess counts, scores.
            line = re.sub(r'"exit":\d+,"color":"\w+"', '"exit":<n>,"color":<c>', line)
            line = re.sub(r'"answer_board":\{"size":(\d+),"cells":\[[^\]]*\]\}', r'"answer_board":<board\1>', line)
            line = re.sub(r'"score":\d+', '"score":<n>', line)
            line = re.sub(r"CallGame x\d+", "CallGame x<n>", line)
        return line


async def run_one(name, rl, steps):
    room = wsprobe.create_room(f"sc-{name}", "pw", rl)
    out, norm = [], Normalizer(room, rl["source"] == "Computer")
    real = []
    for st in steps:
        if st[0] == "badpass":
            real += [("connraw", st[1], f"{room}/WRONG")]
        elif st[0] == "noroom":
            real += [("connraw", st[1], "ZZZZZZ/pw")]
        else:
            real.append(st)
    events = []
    await run_steps(room, real, events)
    # Messages to different sockets race each other; within one step keep
    # each player's stream in order but group by player.
    for step in sorted({e[0] for e in events}):
        grp = [e for e in events if e[0] == step]
        out += [norm(e[3]) for e in grp if e[1] == ""]
        out += [norm(e[3]) for e in sorted((e for e in grp if e[1]), key=lambda e: (e[1], e[2]))]
    out.append("rooms: " + json.dumps([r for r in wsprobe.list_rooms() if r["id"] == room]).replace(room, "<room>"))
    return out


async def run_steps(room, steps, events):
    import websockets
    socks, tokens, tasks = {}, {}, []
    cur = [0]

    def log(line, who=""):
        events.append((cur[0], who, len(events), line))

    async def reader(who, ws):
        try:
            async for m in ws:
                o = json.loads(m)
                if o.get("type") == "Welcome":
                    tokens[who] = o["data"]["token"]
                log(f"{who} <- {m}", who)
        except Exception as e:
            log(f"{who} xx {type(e).__name__}", who)
        else:
            log(f"{who} == server closed", who)

    async def conn(who, path):
        ws = await websockets.connect(f"ws://{wsprobe.BASE}/games/ws/{path}")
        socks[who] = ws
        tasks.append(asyncio.create_task(reader(who, ws)))

    for st in steps:
        cur[0] += 1
        k = st[0]
        if k == "conn":
            q = dict(st[2]) if len(st) > 2 else {}
            if q.get("token") == "@":
                q["token"] = tokens.get(st[1], "missing")
            qs = ("?" + "&".join(f"{a}={b}" for a, b in q.items())) if q else ""
            log(f"{st[1]} == connect {qs}")
            await conn(st[1], f"{room}/pw{qs}")
        elif k == "connraw":
            log(f"{st[1]} == connect raw")
            await conn(st[1], st[2])
        elif k == "send":
            log(f"{st[1]} -> {json.dumps(st[2], separators=(',', ':'))}")
            await socks[st[1]].send(json.dumps(st[2]))
        elif k == "solve":
            # guess a lone black piece cell by cell until the server stops
            # answering GuessIncorrect
            for i in range(16):
                cells = [None] * 16
                cells[i] = "Black"
                n = len(events)
                await socks[st[1]].send(json.dumps(msg("CallGame", guess={"size": 4, "cells": cells})))
                await asyncio.sleep(0.15)
                if not any(e[1] == st[1] and "GuessIncorrect" in e[3] for e in events[n:]):
                    log(f"{st[1]} -> CallGame x{i + 1} (solved)")
                    break
                for e in events[n:]:
                    if e[1] == st[1] and "GuessIncorrect" in e[3]:
                        events.remove(e)
        elif k == "rooms":
            log("rooms: " + json.dumps([r for r in wsprobe.list_rooms() if r["id"] == room]))
        elif k == "close":
            log(f"{st[1]} == close")
            await socks[st[1]].close()
        await asyncio.sleep(st[1] if k == "wait" else 0.15)
    await asyncio.sleep(0.3)
    cur[0] += 1
    log("== end")
    for ws in socks.values():
        await ws.close()
    await asyncio.sleep(0.1)
    for t in tasks:
        t.cancel()


def main():
    os.makedirs(OUT, exist_ok=True)
    for name, (rl, steps) in SCENARIOS.items():
        if ONLY and name not in ONLY:
            continue
        lines = asyncio.run(run_one(name, rl, steps))
        with open(f"{OUT}/{name}.txt", "w") as f:
            f.write("\n".join(lines) + "\n")
        print(f"{name}: {len(lines)} lines")


if __name__ == "__main__":
    main()
