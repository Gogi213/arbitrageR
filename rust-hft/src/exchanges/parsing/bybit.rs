//! Bybit V5 message parser
//!
//! Parses Bybit V5 WebSocket messages into TradeData/TickerData.
//! Zero-copy, zero-allocation hot path.

use super::{find_field, parse_timestamp_ms, ParseResult};
use crate::core::{FixedPoint8, Side, Symbol, TickerData, TradeData};

/// Bybit V5 message parser
pub struct BybitParser;

/// Partial ticker update from Bybit delta
#[derive(Debug, Clone, Copy)]
pub struct BybitTickerUpdate {
    pub symbol: Symbol,
    pub bid_price: Option<FixedPoint8>,
    pub bid_qty: Option<FixedPoint8>,
    pub ask_price: Option<FixedPoint8>,
    pub ask_qty: Option<FixedPoint8>,
    pub timestamp: u64,
}

impl BybitParser {
    /// Parse public trade message into TradeData
    #[inline]
    pub fn parse_public_trade(data: &[u8]) -> Option<ParseResult<TradeData>> {
        if !Self::is_public_trade(data) {
            return None;
        }
        Self::parse_first_trade_in_array(data)
    }

    /// Parse ticker message into TickerData (snapshot)
    #[inline]
    pub fn parse_ticker(data: &[u8]) -> Option<ParseResult<TickerData>> {
        if !Self::is_ticker(data) {
            return None;
        }

        let symbol_bytes =
            find_field(data, b"symbol").or_else(|| Self::extract_symbol_from_topic(data))?;
        let symbol = Symbol::from_bytes(symbol_bytes)?;

        let bid_price = FixedPoint8::parse_bytes(find_field(data, b"bid1Price")?)?;
        let bid_qty = FixedPoint8::parse_bytes(find_field(data, b"bid1Size")?)?;
        let ask_price = FixedPoint8::parse_bytes(find_field(data, b"ask1Price")?)?;
        let ask_qty = FixedPoint8::parse_bytes(find_field(data, b"ask1Size")?)?;

        let timestamp = find_field(data, b"ts")
            .and_then(parse_timestamp_ms)
            .unwrap_or(0);

        let ticker = TickerData::new(symbol, bid_price, bid_qty, ask_price, ask_qty, timestamp);

        Some(ParseResult {
            data: ticker,
            consumed: data.len(),
        })
    }

    /// Parse ticker message into BybitTickerUpdate (delta)
    #[inline]
    pub fn parse_ticker_update(data: &[u8]) -> Option<ParseResult<BybitTickerUpdate>> {
        if !Self::is_ticker(data) {
            return None;
        }

        let symbol_bytes =
            find_field(data, b"symbol").or_else(|| Self::extract_symbol_from_topic(data))?;
        let symbol = Symbol::from_bytes(symbol_bytes)?;

        let bid_price = find_field(data, b"bid1Price").and_then(FixedPoint8::parse_bytes);
        let bid_qty = find_field(data, b"bid1Size").and_then(FixedPoint8::parse_bytes);
        let ask_price = find_field(data, b"ask1Price").and_then(FixedPoint8::parse_bytes);
        let ask_qty = find_field(data, b"ask1Size").and_then(FixedPoint8::parse_bytes);

        let timestamp = find_field(data, b"ts")
            .and_then(parse_timestamp_ms)
            .unwrap_or(0);

        Some(ParseResult {
            data: BybitTickerUpdate {
                symbol,
                bid_price,
                bid_qty,
                ask_price,
                ask_qty,
                timestamp,
            },
            consumed: data.len(),
        })
    }

    /// Parse first trade from data array
    #[inline]
    fn parse_first_trade_in_array(data: &[u8]) -> Option<ParseResult<TradeData>> {
        let data_start = data.windows(7).position(|w| w == b"\"data\":").unwrap_or(0);
        if data_start == 0 {
            return None;
        }

        let data_section = &data[data_start + 7..];
        let obj_start = data_section.iter().position(|&b| b == b'{')?;
        let obj_section = &data_section[obj_start..];

        let symbol = Symbol::from_bytes(find_field(obj_section, b"s")?)?;
        let price = FixedPoint8::parse_bytes(find_field(obj_section, b"p")?)?;
        let qty = FixedPoint8::parse_bytes(find_field(obj_section, b"v")?)?;
        let timestamp = parse_timestamp_ms(find_field(obj_section, b"T")?)?;
        let side = Side::from_bytes(find_field(obj_section, b"S")?).unwrap_or(Side::Buy);
        let is_buyer_maker = matches!(side, Side::Sell);

        let trade = TradeData::new(symbol, price, qty, timestamp, side, is_buyer_maker);

        Some(ParseResult {
            data: trade,
            consumed: data.len(),
        })
    }

    /// Extract symbol from topic field
    #[inline]
    fn extract_symbol_from_topic(data: &[u8]) -> Option<&[u8]> {
        let topic = find_field(data, b"topic")?;
        if let Some(dot_pos) = topic.iter().position(|&b| b == b'.') {
            Some(&topic[dot_pos + 1..])
        } else {
            None
        }
    }

    /// Check if message is publicTrade
    #[inline(always)]
    fn is_public_trade(data: &[u8]) -> bool {
        data.windows(11).any(|w| w == b"publicTrade")
    }

    /// Check if message is tickers
    #[inline(always)]
    fn is_ticker(data: &[u8]) -> bool {
        data.windows(7).any(|w| w == b"tickers")
    }

    /// Detect message type
    #[inline]
    pub fn detect_message_type(data: &[u8]) -> BybitMessageType {
        if Self::is_public_trade(data) {
            BybitMessageType::PublicTrade
        } else if Self::is_ticker(data) {
            BybitMessageType::Ticker
        } else if data.windows(10).any(|w| w == b"\"op\":\"pong\"") {
            BybitMessageType::Pong
        } else if data.windows(21).any(|w| w == b"\"success\":true") {
            BybitMessageType::SubscriptionResponse
        } else {
            BybitMessageType::Unknown
        }
    }
}

/// Bybit message types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BybitMessageType {
    PublicTrade,
    Ticker,
    Pong,
    SubscriptionResponse,
    Unknown,
}

#[cfg(test)]
use crate::test_utils::init_test_registry;
mod tests {
    use super::*;
    use crate::core::registry::SymbolRegistry;


    #[test]
    fn test_detect_public_trade() {
        init_test_registry();
        let data = b"{\"topic\":\"publicTrade.BTCUSDT\",\"data\":[{\"s\":\"BTCUSDT\",\"p\":\"50000.00\"}]}";
        assert_eq!(
            BybitParser::detect_message_type(data),
            BybitMessageType::PublicTrade
        );
    }

    #[test]
    fn test_detect_ticker() {
        init_test_registry();
        let data = b"{\"topic\":\"tickers.BTCUSDT\",\"data\":{\"symbol\":\"BTCUSDT\",\"bid1Price\":\"50000.00\"}}";
        assert_eq!(
            BybitParser::detect_message_type(data),
            BybitMessageType::Ticker
        );
    }

    #[test]
    fn test_detect_unknown() {
        let data = b"{\"unknown\":\"message\"}";
        assert_eq!(
            BybitParser::detect_message_type(data),
            BybitMessageType::Unknown
        );
    }

    #[test]
    fn test_parse_ticker_snapshot() {
        init_test_registry();
        let data = br#"{"topic":"tickers.BTCUSDT","data":{"symbol":"BTCUSDT","bid1Price":"50000.50","bid1Size":"1.5","ask1Price":"50001.00","ask1Size":"0.8","ts":"1234567890123"}}"#;

        let result = BybitParser::parse_ticker(data);
        assert!(result.is_some());

        let parsed = result.unwrap();
        assert_eq!(parsed.data.symbol.as_str(), "BTCUSDT");
        assert!(parsed.data.bid_price.as_raw() > 0);
        assert!(parsed.data.ask_price.as_raw() > 0);
        assert!(parsed.data.timestamp > 0);
    }

    #[test]
    fn test_parse_ticker_update_delta() {
        init_test_registry();
        let data = br#"{"topic":"tickers.BTCUSDT","data":{"symbol":"BTCUSDT","bid1Price":"50000.50","ts":"1234567890123"}}"#;

        let result = BybitParser::parse_ticker_update(data);
        assert!(result.is_some());

        let parsed = result.unwrap();
        assert_eq!(parsed.data.symbol.as_str(), "BTCUSDT");
        assert!(parsed.data.bid_price.is_some());
        assert!(parsed.data.ask_price.is_none());
    }

    #[test]
    fn test_extract_symbol_from_topic() {
        let data = br#"{"topic":"tickers.BTCUSDT","data":{}}"#;
        let symbol = BybitParser::extract_symbol_from_topic(data);
        assert_eq!(symbol, Some(b"BTCUSDT".as_slice()));
    }

    #[test]
    fn test_is_public_trade() {
        let data = b"{\"topic\":\"publicTrade.BTCUSDT\"}";
        assert!(BybitParser::is_public_trade(data));

        let ticker = b"{\"topic\":\"tickers.BTCUSDT\"}";
        assert!(!BybitParser::is_public_trade(ticker));
    }

    #[test]
    fn test_is_ticker() {
        let data = b"{\"topic\":\"tickers.BTCUSDT\"}";
        assert!(BybitParser::is_ticker(data));

        let trade = b"{\"topic\":\"publicTrade.BTCUSDT\"}";
        assert!(!BybitParser::is_ticker(trade));
    }
}
