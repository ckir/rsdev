import asyncio
import websockets
import json
import time
from collections import deque, defaultdict

URL = "wss://ckir.ddns.net:9002/ws"
SYMBOLS = [
    "AAPL", "ABNB", "ADBE", "ADI", "ADP", "ADSK", "AEP", "ALNY", "AMAT", "AMD",
    "AMGN", "AMZN", "APP", "ARM", "ASML", "AVGO", "AXON", "BKNG", "BKR", "CCEP",
    "CDNS", "CEG", "CHTR", "CMCSA", "COST", "CPRT", "CRWD", "CSCO", "CSGP", "CSX",
    "CTAS", "CTSH", "DASH", "DDOG", "DXCM", "EA", "EXC", "FANG", "FAST", "FER",
    "FTNT", "GEHC", "GILD", "GOOG", "GOOGL", "HON", "IDXX", "INSM", "INTC", "INTU",
    "ISRG", "KDP", "KHC", "KLAC", "LIN", "LRCX", "MAR", "MCHP", "MDLZ", "MELI",
    "META", "MNST", "MPWR", "MRVL", "MSFT", "MSTR", "MU", "NFLX", "NVDA", "NXPI",
    "ODFL", "ORLY", "PANW", "PAYX", "PCAR", "PDD", "PEP", "PLTR", "PYPL", "QCOM",
    "REGN", "ROP", "ROST", "SBUX", "SHOP", "SNPS", "STX", "TEAM", "TMUS", "TRI",
    "TSLA", "TTWO", "TXN", "VRSK", "VRTX", "WBD", "WDAY", "WDC", "WMT", "XEL", "ZS",
]

async def rate_printer(global_timestamps, symbol_timestamps):
    """Calculates and prints rates once per minute."""
    try:
        while True:
            await asyncio.sleep(60)
            now = time.time()
            one_minute_ago = now - 60

            # Clean global timestamps
            while global_timestamps and global_timestamps[0] < one_minute_ago:
                global_timestamps.popleft()

            # Clean per-symbol timestamps and build current rate map
            per_symbol_rates = {}
            for symbol, dq in symbol_timestamps.items():
                while dq and dq[0] < one_minute_ago:
                    dq.popleft()
                if len(dq) > 0:
                    per_symbol_rates[symbol] = len(dq)

            # Sort by msg/min descending
            sorted_rates = sorted(per_symbol_rates.items(), key=lambda x: x[1], reverse=True)
            
            # Format as comma-separated string
            report = ", ".join([f"{symbol}: {rate} msg/min" for symbol, rate in sorted_rates])

            print("\n" + "="*30)
            print("----- 1‑Minute Summary -----")
            print(f"Global rate: {len(global_timestamps)} msg/min")
            print(f"Symbols: {report if report else 'No data yet'}")
            print("="*30 + "\n")

    except asyncio.CancelledError:
        # Expected on shutdown
        pass

async def main():
    global_timestamps = deque()
    symbol_timestamps = defaultdict(deque)
    reporter_task = None

    try:
        async with websockets.connect(URL) as ws:
            # Subscribe
            subscription_msg = {"subscribe": SYMBOLS}
            await ws.send(json.dumps(subscription_msg))
            print(f"Subscribed to {len(SYMBOLS)} symbols.")
            print("Running... Press Ctrl+C to stop.")

            # Start background reporter
            reporter_task = asyncio.create_task(rate_printer(global_timestamps, symbol_timestamps))

            # Process incoming messages
            async for raw in ws:
                now = time.time()
                data = json.loads(raw)

                if data.get("type") == "pricing":
                    message = data.get("message", {})
                    symbol = message.get("id", "UNKNOWN")
                    
                    global_timestamps.append(now)
                    symbol_timestamps[symbol].append(now)
                else:
                    # Print non-pricing messages (like system heartbeat)
                    print(f"System Message: {data}")

    except (KeyboardInterrupt, asyncio.CancelledError):
        print("\nShutdown signal received...")
    except Exception as e:
        print(f"\nUnexpected Error: {e}")
    finally:
        if reporter_task:
            reporter_task.cancel()
            try:
                await reporter_task
            except asyncio.CancelledError:
                pass
        print("Clean shutdown complete.")

if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        # Final safety catch for Windows terminal behavior
        pass
    