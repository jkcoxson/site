"""Drive a Light Beams server over websockets and print every message.

A script is a list of steps: ("conn", who, query) opens a socket for player
`who`; ("send", who, obj) sends a ClientMsg; ("close", who); ("wait", secs).
Received messages are printed as `<who> <- <json>`.
"""
import asyncio, json, sys, urllib.parse, urllib.request

BASE = sys.argv[1] if len(sys.argv) > 1 else "127.0.0.1:3999"
SUFFIX = __import__("os").environ.get("SFN_SUFFIX", "13940433110438446569")


def create_room(name, passcode, rules):
    data = urllib.parse.urlencode({"name": name, "passcode": passcode, "rules": json.dumps(rules)}).encode()
    with urllib.request.urlopen(f"http://{BASE}/api/create_room{SUFFIX}", data) as r:
        return json.loads(r.read())


def list_rooms():
    with urllib.request.urlopen(f"http://{BASE}/api/list_rooms{SUFFIX}", b"") as r:
        return json.loads(r.read())


CLASSIC = {"size": 4, "counts": {"red": 1, "yellow": 1, "blue": 1, "black": 1, "slash": 2, "arrow": 1, "four_arrow": 0},
           "source": "Versus", "resolution": "Live", "timed": False}


async def run(room, passcode, steps, log=print):
    import websockets
    socks, readers, tokens = {}, {}, {}

    async def reader(who, ws):
        try:
            async for m in ws:
                obj = json.loads(m)
                if obj.get("type") == "Welcome":
                    tokens[who] = obj["data"]["token"]
                log(f"{who} <- {m}")
        except Exception as e:
            log(f"{who} xx {type(e).__name__}")

    for step in steps:
        kind = step[0]
        if kind == "conn":
            who, query = step[1], dict(step[2]) if len(step) > 2 else {}
            if query.get("token") == "@":
                query["token"] = tokens[who]
            q = ("?" + urllib.parse.urlencode(query)) if query else ""
            ws = await websockets.connect(f"ws://{BASE}/games/ws/{room}/{passcode}{q}")
            socks[who] = ws
            readers[who] = asyncio.create_task(reader(who, ws))
            log(f"{who} == connected {q}")
        elif kind == "send":
            log(f"{step[1]} -> {json.dumps(step[2])}")
            await socks[step[1]].send(json.dumps(step[2]))
        elif kind == "close":
            await socks[step[1]].close()
            log(f"{step[1]} == closed")
        await asyncio.sleep(step[-1] if kind == "wait" else 0.25)
    for ws in socks.values():
        await ws.close()
    return tokens
