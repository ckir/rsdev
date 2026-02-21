//! # Yahoo Finance Protobuf Handler
//!
//! This module contains the Rust representations of the Yahoo Finance `PricingData` 
//! protocol buffer messages. It uses the `prost` framework for efficient binary 
//! deserialization of real-time market data.

use prost::Message;
use serde::{Deserialize, Serialize};

/// The primary data structure for Yahoo Finance real-time quotes.
///
/// This struct corresponds to the `PricingData` message in the `.proto` definition.
/// It contains price, volume, and metadata for a specific financial instrument.
#[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
pub struct PricingData {
    /// The ticker symbol (e.g., "AAPL", "BTC-USD")
    #[prost(string, tag = "1")]
    pub id: String,

    /// The current price of the instrument
    #[prost(float, tag = "2")]
    pub price: f32,

    /// The timestamp of the quote in milliseconds since Unix Epoch
    #[prost(sint64, tag = "3")]
    pub time: i64,

    /// The currency in which the price is quoted
    #[prost(string, tag = "4")]
    pub currency: String,

    /// The exchange where the instrument is traded (e.g., "NMS", "NYQ")
    #[prost(string, tag = "5")]
    pub exchange: String,

    /// The type of quote (Equity, Index, Cryptocurrency, etc.)
    /// Maps to the [`QuoteType`] enum.
    #[prost(enumeration = "QuoteType", tag = "6")]
    pub quote_type: i32,

    /// The market session this quote belongs to (Pre, Regular, Post)
    /// Maps to the [`MarketHoursType`] enum.
    #[prost(enumeration = "MarketHoursType", tag = "7")]
    pub market_hours: i32,

    /// The percentage change since the last market close
    #[prost(float, tag = "8")]
    pub change_percent: f32,

    /// The total trading volume for the current day
    #[prost(sint64, tag = "9")]
    pub day_volume: i64,

    /// The highest price traded during the current day
    #[prost(float, tag = "10")]
    pub day_high: f32,

    /// The lowest price traded during the current day
    #[prost(float, tag = "11")]
    pub day_low: f32,

    /// The absolute price change since the last market close
    #[prost(float, tag = "12")]
    pub change: f32,

    /// The short descriptive name of the instrument
    #[prost(string, tag = "13")]
    pub short_name: String,

    /// The expiration date if the instrument is an option or future (Unix timestamp)
    #[prost(sint64, tag = "14")]
    pub expire_date: i64,

    /// The open interest for derivatives
    #[prost(sint64, tag = "15")]
    pub open_interest: i64,

    /// The underlying symbol for derivatives
    #[prost(string, tag = "16")]
    pub underlying_symbol: String,

    /// The strike price for options
    #[prost(float, tag = "17")]
    pub strike_price: f32,

    /// Whether the option is a Call or Put
    #[prost(enumeration = "OptionType", tag = "18")]
    pub option_type: i32,
}

/// Categorizes the type of financial instrument providing the data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration)]
#[repr(i32)]
pub enum QuoteType {
    /// No specific type defined
    None = 0,
    /// An alternative symbol mapping
    Altsymbol = 5,
    /// System heartbeat used to keep the connection alive
    Heartbeat = 7,
    /// Standard stock/equity
    Equity = 8,
    /// Market index (e.g., S&P 500)
    Index = 9,
    /// Mutual fund
    Mutualfund = 11,
    /// Money market fund
    Moneymarket = 12,
    /// Derivative option
    Option = 13,
    /// Forex/Currency pair
    Currency = 14,
    /// Financial warrant
    Warrant = 15,
    /// Debt instrument
    Bond = 17,
    /// Futures contract
    Future = 18,
    /// Exchange Traded Fund
    Etf = 20,
    /// Physical or hard commodity
    Commodity = 23,
    /// Electronic Communication Network quote
    Ecnquote = 28,
    /// Digital assets/Crypto
    Cryptocurrency = 41,
    /// Market indicator
    Indicator = 42,
    /// Industry-wide data
    Industry = 1000,
}

/// Represents the specific trading session of the quote.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration)]
#[repr(i32)]
pub enum MarketHoursType {
    /// Before the market officially opens
    PreMarket = 0,
    /// Standard exchange hours
    RegularMarket = 1,
    /// After the market officially closes
    PostMarket = 2,
    /// Combined extended hours trading
    ExtendedHoursMarket = 3,
}

/// Distinguishes between Call and Put options.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration)]
#[repr(i32)]
pub enum OptionType {
    /// Right to buy
    Call = 0,
    /// Right to sell
    Put = 1,
}