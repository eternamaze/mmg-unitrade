use crate::common::account_model::{TradeId, TradingPair};
use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone)]
pub struct Trade {
    pub id: TradeId,
    pub price: Decimal,
    pub quantity: Decimal,
    pub side: TradeSide,
    pub timestamp: u64,
}

/// 成交流 (The Canvas)
///
/// 这是一个环形缓冲区或流式容器，用于记录最近的成交。
#[derive(Debug, Clone)]
pub struct TradeFlow {
    pub pair: TradingPair,
    pub trades: Vec<Trade>,
    pub capacity: usize,
}

impl TradeFlow {
    pub fn new(pair: TradingPair, capacity: usize) -> Self {
        Self {
            pair,
            trades: Vec::with_capacity(capacity),
            capacity,
        }
    }

    /// 绘制一笔新成交
    pub fn push(&mut self, trade: Trade) {
        if self.trades.len() >= self.capacity {
            self.trades.remove(0); // 简单的 FIFO，生产环境可能用 RingBuffer
        }
        self.trades.push(trade);
    }
}
