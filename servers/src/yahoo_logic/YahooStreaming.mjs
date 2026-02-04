import EventEmitter from 'events';
import WebSocket from 'ws';
import path from 'path';
import { fileURLToPath } from 'node:url';
import { dirname } from 'path';
import { DateTime } from 'luxon';
import pkg from 'protobufjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const { loadSync } = pkg;
const protoRoot = loadSync(path.join(__dirname, 'PricingData.proto'));
const PricingData = protoRoot.lookupType('PricingData');

if (!globalThis.logger) {
    const { default: Logger } = await import('../../../../../../Common/Loggers/LoggerDummy.mjs');
    globalThis.logger = new Logger();
    globalThis.logger.debug('Logger initialized.');
}

export const QuoteType = {
    NONE: 0,
    ALTSYMBOL: 5,
    HEARTBEAT: 7,
    EQUITY: 8,
    INDEX: 9,
    MUTUALFUND: 11,
    MONEYMARKET: 12,
    OPTION: 13,
    CURRENCY: 14,
    WARRANT: 15,
    BOND: 17,
    FUTURE: 18,
    ETF: 20,
    COMMODITY: 23,
    ECNQUOTE: 28,
    CRYPTOCURRENCY: 41,
    INDICATOR: 42,
    INDUSTRY: 1000
};

export const MarketHoursType = {
    PRE_MARKET: 0,
    REGULAR_MARKET: 1,
    POST_MARKET: 2,
    EXTENDED_HOURS_MARKET: 3,
    CLOSED: 4
};

/**
 * @typedef {Object} Quote
 * @property {string} symbol
 * @property {number} price
 * @property {DateTime} ts
 * @property {string} type
 * @property {string} source
 */

/**
 * @typedef {Object} ThroughputInfo
 * @property {number} perInterval
 * @property {number} intervalMinutes
 * @property {DateTime} timestamp
 */

/**
 * @typedef {Object} SymbolThroughputInfo
 * @property {Record<string, number>} perInterval
 * @property {number} intervalMinutes
 * @property {DateTime} timestamp
 */

/**
 * @typedef {Object} SymbolAveragesInfo
 * @property {Record<string, number>} averages
 * @property {number} windowSize
 * @property {DateTime} timestamp
 */

/**
 * @typedef {Object} LatencyInfo
 * @property {string} symbol
 * @property {number} latencyMs
 * @property {DateTime} serverTime
 * @property {DateTime} clientTime
 */

/**
 * @typedef {Object} HeartbeatInfo
 * @property {boolean} alive
 * @property {number} thresholdSeconds
 * @property {DateTime} lastMessageTime
 * @property {DateTime} checkedAt
 */

/**
 * @typedef {Object} YahooStreamingOptions
 * @property {string} [url]  WebSocket URL
 * @property {number} [reportIntervalMinutes]  Interval for throughput reporting
 * @property {number} [refreshIntervalMinutes] Interval for auto‑refresh (unsubscribe/resubscribe)
 * @property {boolean} [enableGlobalThroughput]
 * @property {boolean} [enableSymbolThroughput]
 * @property {boolean} [enableSymbolAverages]
 * @property {boolean} [enableLatency]
 * @property {boolean} [enableHeartbeatDetection]
 * @property {boolean} [enableReconnect]
 * @property {boolean} [enableAutoRefresh]
 * @property {boolean} [enableDebugLogging]
 * @property {number} [heartbeatThresholdSeconds]
 * @property {number} [reconnectBaseDelayMs]
 * @property {number} [reconnectMaxDelayMs]
 * @property {number} [symbolAverageWindow]  Number of intervals for moving average
 */

/**
 * Monolithic Yahoo Finance WebSocket streaming client with:
 * - Global throughput
 * - Per‑symbol throughput
 * - Per‑symbol moving averages
 * - Latency measurement
 * - Heartbeat loss detection
 * - Auto‑reconnect with exponential backoff
 * - Auto‑refresh subscriptions
 * - Luxon timestamps
 *
 * All features are optional and controlled via constructor flags.
 *
 * @extends EventEmitter
 */
export default class YahooStreaming extends EventEmitter {
    /**
     * @type {YahooStreamingOptions}
     * @private
     */
    defaults = {
        url: 'wss://streamer.finance.yahoo.com/?version=2',
        reportIntervalMinutes: 1,
        refreshIntervalMinutes: 0,
        enableGlobalThroughput: true,
        enableSymbolThroughput: true,
        enableSymbolAverages: true,
        enableLatency: true,
        enableHeartbeatDetection: true,
        enableReconnect: true,
        enableAutoRefresh: true,
        enableDebugLogging: false,
        heartbeatThresholdSeconds: 30,
        reconnectBaseDelayMs: 1_000,
        reconnectMaxDelayMs: 60_000,
        symbolAverageWindow: 5,
    };

    /**
     * @param {string[]} [symbols=[]]  Initial symbols to subscribe
     * @param {YahooStreamingOptions} [options={}]  Configuration options
     */
    constructor(symbols = [], options = {}) {
        super();

        /** @type {YahooStreamingOptions} */
        this.options = { ...this.defaults, ...options };

        /** @type {string[]} */
        this.symbols = symbols;

        /** @type {WebSocket|null} */
        this.ws = null;

        /** @type {Console|any} */
        this.logger = globalThis.logger ?? console;

        // Throughput tracking
        /** @type {DateTime[]} */
        this.msgTimestamps = [];
        /** @type {Map<string, DateTime[]>} */
        this.symbolCounters = new Map();
        /** @type {DateTime} */
        this.lastGlobalReport = DateTime.now();
        /** @type {DateTime} */
        this.lastSymbolReport = DateTime.now();
        /** @type {number} */
        this.reportIntervalMs = this.options.reportIntervalMinutes * 60_000;

        // Per‑symbol moving averages
        /** @type {Map<string, number[]>} */
        this.symbolHistory = new Map();

        // Auto‑refresh
        /** @type {number} */
        this.refreshIntervalMs = this.options.refreshIntervalMinutes * 60_000;
        /** @type {DateTime} */
        this.lastRefresh = DateTime.now();

        // Latency
        /** @type {number|null} */
        this.lastLatencyMs = null;

        // Heartbeat
        /** @type {DateTime} */
        this.lastMessageTime = DateTime.now();
        /** @type {NodeJS.Timeout|null} */
        this.heartbeatTimer = null;

        // Reconnect
        /** @type {number} */
        this.reconnectAttempts = 0;
        /** @type {boolean} */
        this.manualClose = false;

        this._setupHeartbeatMonitor();

        this.logger.info(`${this.constructor.name}: Started`);
        this._registerSignalHandlers();		
    } // constructor

    /**
     * Gracefully shuts down the stream by terminating the WebSocket
     * and preventing reconnect attempts.
     */
    _gracefulShutdown() {
        this.logger.info(`${this.constructor.name}: Graceful shutdown requested`);

        // Prevent reconnect logic from triggering
        this.manualClose = true;

        // Stop heartbeat timer if running
        if (this.heartbeatTimer) {
            clearInterval(this.heartbeatTimer);
            this.heartbeatTimer = null;
        }

        // Terminate WebSocket immediately
        if (this.ws) {
            try {
                this.ws.terminate();
                this.logger.info(`${this.constructor.name}: WebSocket terminated`);
            } catch (err) {
                this.logger.error(`${this.constructor.name}: Error during termination`, err);
            }
        }

        this.emit('shutdown');
    } // _gracefulShutdown

    /**
 * Registers OS signal handlers for graceful shutdown.
 *
 * Ensures handlers are only registered once globally.
 *
 * @private
 */
    _registerSignalHandlers() {
        if (YahooStreaming._signalsRegistered) return;
        YahooStreaming._signalsRegistered = true;

        const shutdown = () => {
            this.logger.info(`${this.constructor.name}: Received shutdown signal`);
            this._gracefulShutdown();
        };

        process.on('SIGINT', shutdown);
        process.on('SIGTERM', shutdown);
        process.on('SIGQUIT', shutdown);

        this.logger.info(`${this.constructor.name}: Signal handlers registered`);
    } // _registerSignalHandlers

    /**
     * Connects to the WebSocket endpoint and sets up handlers.
     *
     * @returns {Promise<void>}
     */
    async connect() {
        this.manualClose = false;
        this._connectInternal();
    }

    /**
     * Internal connect with reconnect support.
     *
     * @private
     */
    _connectInternal() {
        this.ws = new WebSocket(this.options.url);

        this.ws.on('open', () => {
            this.reconnectAttempts = 0;
            this.logger.info(`${this.constructor.name}: Connected`);
            if (this.symbols.length > 0) {
                this.ws.send(JSON.stringify({ subscribe: this.symbols }));
                this.logger.info(`${this.constructor.name}: Initial subscribe ${JSON.stringify(this.symbols)}`);
            }
            this.emit('open');
        });

        this.ws.on('error', (err) => {
            this.logger.error(`${this.constructor.name}: WebSocket error`, err);
            this.emit('error', err);
        });

        this.ws.on('close', () => {
            this.logger.warn(`${this.constructor.name}: Connection closed`);
            this.emit('close');
            if (!this.manualClose && this.options.enableReconnect) {
                this._scheduleReconnect();
            }
        });

        this.ws.on('ping', () => {
            this.ws.pong();
            this.emit('ping');
        });

        this.ws.on('pong', () => {
            this.emit('pong');
        });

        this.ws.on('message', (data) => {
            const now = DateTime.now();
            this.lastMessageTime = now;

            if (this.options.enableAutoRefresh) {
                this._refreshSubscriptions(now);
            }

            if (this.options.enableGlobalThroughput) {
                this._recordGlobalThroughput(now);
            }

            this.emit('message', data);

            try {
                data = JSON.parse(data);

                if (data.type === 'pricing') {
                    const buffer = Buffer.from(data.message, 'base64');
                    const decoded = PricingData.decode(buffer);
                    const obj = PricingData.toObject(decoded, {
                        longs: String,
                        enums: String,
                        bytes: Buffer,
                    });

                    /** @type {Quote} */
                    const quote = {
                        symbol: obj.id,
                        price: obj.price,
                        ts: DateTime.fromMillis(parseInt(obj.time)),
                        type: 'pricing',
                        source: this.constructor.name,
                    };

                    if (this.options.enableLatency) {
                        this._recordLatency(quote);
                    }

                    if (this.options.enableSymbolThroughput || this.options.enableSymbolAverages) {
                        this._recordSymbolThroughput(quote.symbol, now);
                    }

                    this.emit('pricing', quote);
                }
            } catch (err) {
                this.logger.warn(`${this.constructor.name}: Non‑JSON message`, data);
            }
        });
    }

    /**
     * Schedules reconnect with exponential backoff.
     *
     * @private
     */
    _scheduleReconnect() {
        this.reconnectAttempts += 1;
        const base = this.options.reconnectBaseDelayMs;
        const max = this.options.reconnectMaxDelayMs;
        const delay = Math.min(max, base * 2 ** (this.reconnectAttempts - 1));

        this.logger.warn(`${this.constructor.name}: Reconnecting in ${delay} ms (attempt ${this.reconnectAttempts})`);

        setTimeout(() => {
            this._connectInternal();
        }, delay);
    }

    /**
     * Records global throughput and emits a throughput event every interval.
     *
     * @param {DateTime} now
     * @private
     */
    _recordGlobalThroughput(now) {
        this.msgTimestamps.push(now);

        const cutoff = now.minus({ milliseconds: this.reportIntervalMs });
        this.msgTimestamps = this.msgTimestamps.filter(t => t >= cutoff);

        if (now.diff(this.lastGlobalReport).milliseconds >= this.reportIntervalMs) {
            const perInterval = this.msgTimestamps.length;

            if (this.options.enableDebugLogging) {
                this.logger.info(
                    `${this.constructor.name}: Global throughput: ${perInterval} messages / ${this.options.reportIntervalMinutes} min`
                );
            }

            /** @type {ThroughputInfo} */
            const info = {
                perInterval,
                intervalMinutes: this.options.reportIntervalMinutes,
                timestamp: now,
            };

            this.emit('throughput', info);

            this.lastGlobalReport = now;
        }
    }

    /**
     * Records per‑symbol throughput and moving averages, and emits events.
     *
     * @param {string} symbol
     * @param {DateTime} now
     * @private
     */
    _recordSymbolThroughput(symbol, now) {
        if (!this.symbolCounters.has(symbol)) {
            this.symbolCounters.set(symbol, []);
        }

        const arr = this.symbolCounters.get(symbol);
        arr.push(now);

        const cutoff = now.minus({ milliseconds: this.reportIntervalMs });
        const filtered = arr.filter(t => t >= cutoff);
        this.symbolCounters.set(symbol, filtered);

        const shouldReport = now.diff(this.lastSymbolReport).milliseconds >= this.reportIntervalMs;

        if (shouldReport && this.options.enableSymbolThroughput) {
            const report = {};
            for (const [sym, timestamps] of this.symbolCounters.entries()) {
                report[sym] = timestamps.length;
            }

            if (this.options.enableDebugLogging) {
                this.logger.info(
                    `${this.constructor.name}: Per‑symbol throughput (${this.options.reportIntervalMinutes} min): ${JSON.stringify(report)}`
                );
            }

            /** @type {SymbolThroughputInfo} */
            const info = {
                perInterval: report,
                intervalMinutes: this.options.reportIntervalMinutes,
                timestamp: now,
            };

            this.emit('symbol-throughput', info);
        }

        if (shouldReport && this.options.enableSymbolAverages) {
            const averages = {};
            for (const [sym, timestamps] of this.symbolCounters.entries()) {
                const count = timestamps.length;
                if (!this.symbolHistory.has(sym)) {
                    this.symbolHistory.set(sym, []);
                }
                const hist = this.symbolHistory.get(sym);
                hist.push(count);
                while (hist.length > this.options.symbolAverageWindow) {
                    hist.shift();
                }
                const avg = hist.reduce((a, b) => a + b, 0) / hist.length;
                averages[sym] = avg;
            }

            if (this.options.enableDebugLogging) {
                this.logger.info(
                    `${this.constructor.name}: Per‑symbol moving averages (window=${this.options.symbolAverageWindow}): ${JSON.stringify(averages)}`
                );
            }

            /** @type {SymbolAveragesInfo} */
            const avgInfo = {
                averages,
                windowSize: this.options.symbolAverageWindow,
                timestamp: now,
            };

            this.emit('symbol-averages', avgInfo);
        }

        if (shouldReport) {
            this.lastSymbolReport = now;
        }
    }

    /**
     * Records latency between server timestamp and client receipt time.
     *
     * @param {Quote} quote
     * @private
     */
    _recordLatency(quote) {
        const clientTime = DateTime.now();
        const serverTime = quote.ts;
        const latencyMs = clientTime.diff(serverTime).milliseconds;
        this.lastLatencyMs = latencyMs;

        if (this.options.enableDebugLogging) {
            this.logger.info(
                `${this.constructor.name}: Latency for ${quote.symbol}: ${latencyMs.toFixed(1)} ms`
            );
        }

        /** @type {LatencyInfo} */
        const info = {
            symbol: quote.symbol,
            latencyMs,
            serverTime,
            clientTime,
        };

        this.emit('latency', info);
    }

    /**
     * Sets up heartbeat monitoring timer.
     *
     * @private
     */
    _setupHeartbeatMonitor() {
        if (!this.options.enableHeartbeatDetection) return;

        if (this.heartbeatTimer) {
            clearInterval(this.heartbeatTimer);
        }

        this.heartbeatTimer = setInterval(() => {
            const now = DateTime.now();
            const diffSec = now.diff(this.lastMessageTime).as('seconds');
            const threshold = this.options.heartbeatThresholdSeconds;
            const alive = diffSec <= threshold;

            if (!alive) {
                this.logger.warn(
                    `${this.constructor.name}: Heartbeat lost (last message ${diffSec.toFixed(1)}s ago)`
                );
            }

            /** @type {HeartbeatInfo} */
            const info = {
                alive,
                thresholdSeconds: threshold,
                lastMessageTime: this.lastMessageTime,
                checkedAt: now,
            };

            this.emit('heartbeat', info);
        }, 1_000);
    }

    /**
     * Automatically unsubscribes and re‑subscribes all symbols every N minutes.
     *
     * @param {DateTime} now
     * @private
     */
    _refreshSubscriptions(now) {
        if (!this.options.enableAutoRefresh) return;
        if (this.options.refreshIntervalMinutes <= 0) return;

        if (now.diff(this.lastRefresh).milliseconds < this.refreshIntervalMs) return;
        if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return;

        if (this.symbols.length > 0) {
            this.ws.send(JSON.stringify({ unsubscribe: this.symbols }));
            this.logger.info(`${this.constructor.name}: Auto‑refresh unsubscribed all symbols`);
            this.ws.send(JSON.stringify({ subscribe: this.symbols }));
            this.logger.info(`${this.constructor.name}: Auto‑refresh re‑subscribed all symbols`);
        }

        this.lastRefresh = now;
    }

    /**
     * Subscribes to additional symbols.
     *
     * @param {string[]} symbols
     */
    subscribe(symbols) {
        this.symbols = [...new Set([...this.symbols, ...symbols])].sort();
        if (this.ws && this.ws.readyState === WebSocket.OPEN) {
            this.ws.send(JSON.stringify({ subscribe: symbols }));
        }
        this.logger.info(`${this.constructor.name}: Subscribed ${JSON.stringify(symbols)}`);
    }

    /**
     * Unsubscribes from given symbols.
     *
     * @param {string[]} symbols
     */
    unsubscribe(symbols) {
        this.symbols = this.symbols.filter(s => !symbols.includes(s));
        if (this.ws && this.ws.readyState === WebSocket.OPEN) {
            this.ws.send(JSON.stringify({ unsubscribe: symbols }));
        }
        this.logger.info(`${this.constructor.name}: Unsubscribed ${JSON.stringify(symbols)}`);
    }

    /**
     * Returns the currently subscribed symbols.
     *
     * @returns {string[]}
     */
    getSymbols() {
        return this.symbols;
    }

    /**
     * Closes the WebSocket connection and stops reconnect attempts.
     */
    close() {
        this.manualClose = true;
        if (this.ws) {
            this.ws.close();
        }
        if (this.heartbeatTimer) {
            clearInterval(this.heartbeatTimer);
            this.heartbeatTimer = null;
        }
        this.logger.info(`${this.constructor.name}: Closed`);
    }
} // YahooStreaming


// // Instantiate with all features enabled
// const stream = new YahooStreaming(
//     ["AAPL", "MSFT", "TSLA"],
//     {
//         reportIntervalMinutes: 1,          // Throughput interval
//         refreshIntervalMinutes: 5,         // Auto-refresh subscriptions every 5 minutes

//         enableGlobalThroughput: true,
//         enableSymbolThroughput: true,
//         enableSymbolAverages: true,
//         enableLatency: true,
//         enableHeartbeatDetection: true,
//         enableReconnect: true,
//         enableAutoRefresh: true,
//         enableDebugLogging: true,

//         heartbeatThresholdSeconds: 30,
//         reconnectBaseDelayMs: 1000,
//         reconnectMaxDelayMs: 60000,
//         symbolAverageWindow: 5
//     }
// );

// // Fired when WebSocket connects
// stream.on("open", () => {
//     console.log("Connected to Yahoo Finance stream");
// });

// // Fired for every raw message
// stream.on("message", (msg) => {
//     // console.log("RAW:", msg.toString());
// });

// // Fired for decoded pricing messages
// stream.on("pricing", (quote) => {
//     console.log(
//         `QUOTE: ${quote.symbol} = ${quote.price} @ ${quote.ts.toISO()}`
//     );
// });

// // Fired every N minutes (global throughput)
// stream.on("throughput", (info) => {
//     console.log("GLOBAL THROUGHPUT:", info);
// });

// // Fired every N minutes (per-symbol throughput)
// stream.on("symbol-throughput", (info) => {
//     console.log("PER SYMBOL THROUGHPUT:", info);
// });

// // Fired every N minutes (per-symbol moving averages)
// stream.on("symbol-averages", (info) => {
//     console.log("MOVING AVERAGES:", info);
// });

// // Fired for each latency measurement
// stream.on("latency", (info) => {
//     console.log(
//         `LATENCY: ${info.symbol} = ${info.latencyMs.toFixed(1)} ms`
//     );
// });

// // Fired every second (heartbeat monitor)
// stream.on("heartbeat", (info) => {
//     if (!info.alive) {
//         console.warn("HEARTBEAT LOST:", info);
//     }
// });

// // Fired on reconnect attempts
// stream.on("close", () => {
//     console.log("Connection closed");
// });

// // Fired on WebSocket errors
// stream.on("error", (err) => {
//     console.error("ERROR:", err);
// });

// // Start the stream
// await stream.connect();