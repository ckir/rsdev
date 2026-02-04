import yfinance as yf
import logging
import websocket # Make sure to `pip install websocket-client`

# Enable verbose logging for the websocket-client library
# This will print the raw WebSocket frames being sent and received.
websocket.enableTrace(True)

# Set up a basic logger to see yfinance's output as well
logging.basicConfig(level=logging.DEBUG)

def on_message(ws, message):
    """Callback function to handle incoming messages."""
    print("--- Received Message from Yahoo ---")
    print(message)
    print("-----------------------------------")

def on_error(ws, error):
    """Callback function to handle errors."""
    print(f"--- WebSocket Error ---")
    print(error)
    print("-----------------------")

def on_close(ws, close_status_code, close_msg):
    """Callback function to handle connection closing."""
    print("--- WebSocket Closed ---")
    print(f"Status Code: {close_status_code}, Message: {close_msg}")
    print("------------------------")

def on_open(ws):
    """Callback function when the connection is opened."""
    print("--- WebSocket Opened ---")
    # Define the tickers to subscribe to
    tickers = ["AAPL", "GOOG", "TSLA"]
    print(f"Subscribing to: {tickers}")
    # The yfinance library handles the subscription format internally.
    # By tracing the websocket traffic, we can see what it sends.
    ws.send(str({"subscribe": tickers})) # This mimics how yfinance sends the message
    print("------------------------")

if __name__ == "__main__":
    # The WebSocket URL yfinance uses
    yahoo_ws_url = "wss://streamer.finance.yahoo.com/"

    print("Starting WebSocket inspector...")
    # We are creating our own WebSocketApp to have more control over logging.
    # We will manually mimic the subscription message that yfinance sends.
    ws = websocket.WebSocketApp(yahoo_ws_url,
                              on_open=on_open,
                              on_message=on_message,
                              on_error=on_error,
                              on_close=on_close)

    # Run the WebSocket client
    ws.run_forever()
