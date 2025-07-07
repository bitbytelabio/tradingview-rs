use crate::quote::models::QuoteValue;

pub fn merge_quotes(quote_old: &QuoteValue, quote_new: &QuoteValue) -> QuoteValue {
    QuoteValue {
        ask: quote_new.ask.or(quote_old.ask),
        ask_size: quote_new.ask_size.or(quote_old.ask_size),
        bid: quote_new.bid.or(quote_old.bid),
        bid_size: quote_new.bid_size.or(quote_old.bid_size),
        change: quote_new.change.or(quote_old.change),
        change_percent: quote_new.change_percent.or(quote_old.change_percent),
        open: quote_new.open.or(quote_old.open),
        high: quote_new.high.or(quote_old.high),
        low: quote_new.low.or(quote_old.low),
        prev_close: quote_new.prev_close.or(quote_old.prev_close),
        price: quote_new.price.or(quote_old.price),
        timestamp: quote_new.timestamp.or(quote_old.timestamp),
        volume: quote_new.volume.or(quote_old.volume),
        currency: quote_new.currency.or(quote_old.currency),
        symbol: quote_new.symbol.or(quote_old.symbol),
        exchange: quote_new.exchange.or(quote_old.exchange),
        market_type: quote_new.market_type.or(quote_old.market_type),
    }
}
