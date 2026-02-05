import asyncio
import websockets
import json
import time
from collections import deque, defaultdict

URL = "wss://ckir.ddns.net:9002/ws"
SYMBOLS = [
    "AAPL",
    "ABNB",
    "ADBE",
    "ADI",
    "ADP",
    "ADSK",
    "AEP",
    "ALNY",
    "AMAT",
    "AMD",
    "AMGN",
    "AMZN",
    "APP",
    "ARM",
    "ASML",
    "AVGO",
    "AXON",
    "BKNG",
    "BKR",
    "CCEP",
    "CDNS",
    "CEG",
    "CHTR",
    "CMCSA",
    "COST",
    "CPRT",
    "CRWD",
    "CSCO",
    "CSGP",
    "CSX",
    "CTAS",
    "CTSH",
    "DASH",
    "DDOG",
    "DXCM",
    "EA",
    "EXC",
    "FANG",
    "FAST",
    "FER",
    "FTNT",
    "GEHC",
    "GILD",
    "GOOG",
    "GOOGL",
    "HON",
    "IDXX",
    "INSM",
    "INTC",
    "INTU",
    "ISRG",
    "KDP",
    "KHC",
    "KLAC",
    "LIN",
    "LRCX",
    "MAR",
    "MCHP",
    "MDLZ",
    "MELI",
    "META",
    "MNST",
    "MPWR",
    "MRVL",
    "MSFT",
    "MSTR",
    "MU",
    "NFLX",
    "NVDA",
    "NXPI",
    "ODFL",
    "ORLY",
    "PANW",
    "PAYX",
    "PCAR",
    "PDD",
    "PEP",
    "PLTR",
    "PYPL",
    "QCOM",
    "REGN",
    "ROP",
    "ROST",
    "SBUX",
    "SHOP",
    "SNPS",
    "STX",
    "TEAM",
    "TMUS",
    "TRI",
    "TSLA",
    "TTWO",
    "TXN",
    "VRSK",
    "VRTX",
    "WBD",
    "WDAY",
    "WDC",
    "WMT",
    "XEL",
    "ZS",
]


async def rate_printer(global_timestamps, symbol_timestamps):
    """Prints rates once per minute."""
    while True:
        await asyncio.sleep(60)
        now = time.time()
        one_minute_ago = now - 60

        # Clean global timestamps
        while global_timestamps and global_timestamps[0] < one_minute_ago:
            global_timestamps.popleft()

        global_rate = len(global_timestamps)

        # Clean per-symbol timestamps
        per_symbol_rates = {}
        for symbol, dq in symbol_timestamps.items():
            while dq and dq[0] < one_minute_ago:
                dq.popleft()
            per_symbol_rates[symbol] = len(dq)

        # Print summary
        print("----- 1‑Minute Summary -----")
        print(f"Global rate: {global_rate} msg/min")
        for symbol, rate in per_symbol_rates.items():
            print(f"{symbol}: {rate} msg/min")
        print("----------------------------\n")


async def main():
    global_timestamps = deque()
    symbol_timestamps = defaultdict(deque)

    async with websockets.connect(URL) as ws:
        # Send subscription message
        msg = {"subscribe": SYMBOLS}
        await ws.send(json.dumps(msg))
        print("Subscribed.")

        # Start the summary printer
        asyncio.create_task(rate_printer(global_timestamps, symbol_timestamps))

        async for raw in ws:
            now = time.time()
            data = json.loads(raw)

            if data.get("type") != "pricing":
                print(data)
                continue

            message = data.get("message", {})
            symbol = message.get("id", "UNKNOWN")

            # Track timestamps
            global_timestamps.append(now)
            symbol_timestamps[symbol].append(now)


asyncio.run(main())
