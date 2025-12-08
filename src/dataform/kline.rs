use crate::common::account_model::{KlineInterval, TradingPair};
use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy)]
pub struct Kline {
    pub open_time: u64,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Decimal,
    pub close_time: u64,
}

/// K线图 (The Canvas)
#[derive(Debug, Clone)]
pub struct KlineSeries {
    pub pair: TradingPair,
    pub interval: KlineInterval,
    pub candles: Vec<Kline>,
}

impl KlineSeries {
    pub fn new(pair: TradingPair, interval: KlineInterval) -> Self {
        Self {
            pair,
            interval,
            candles: Vec::new(),
        }
    }

    /// 更新或追加 K 线
    /// 如果 timestamp 相同则更新，更新则追加
    pub fn update(&mut self, kline: Kline) {
        if let Some(last) = self.candles.last_mut() {
            if last.open_time == kline.open_time {
                *last = kline;
            } else if kline.open_time > last.open_time {
                self.candles.push(kline);
            }
        } else {
            self.candles.push(kline);
        }
    }
}
