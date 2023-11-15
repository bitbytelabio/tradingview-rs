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
        description: quote_new.description.clone().or(quote_old.description.clone()),
        country: quote_new.country.clone().or(quote_old.country.clone()),
        currency: quote_new.currency.clone().or(quote_old.currency.clone()),
        data_provider: quote_new.data_provider.clone().or(quote_old.data_provider.clone()),
        symbol: quote_new.symbol.clone().or(quote_old.symbol.clone()),
        symbol_id: quote_new.symbol_id.clone().or(quote_old.symbol_id.clone()),
        exchange: quote_new.exchange.clone().or(quote_old.exchange.clone()),
        market_type: quote_new.market_type.clone().or(quote_old.market_type.clone()),
    }
}
