use crate::common::account_model::TradingPair;
use rust_decimal::Decimal;
use std::collections::BTreeMap;

/// 订单簿方向
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

/// 订单簿层级
#[derive(Debug, Clone, Copy)]
pub struct Level {
    pub price: Decimal,
    pub quantity: Decimal,
}

/// 订单簿 (The Canvas)
///
/// 这是一个纯数据结构，用于“被画”。
/// 连接器负责调用它的方法来更新状态。
#[derive(Debug, Clone)]
pub struct OrderBook {
    pub pair: TradingPair,
    /// 卖单：价格从低到高排序
    pub asks: BTreeMap<Decimal, Decimal>,
    /// 买单：价格从高到低排序 (使用 Reverse 包装或自定义比较，这里简化为 BTreeMap，遍历时需注意)
    /// Rust 的 BTreeMap 默认升序。为了方便获取最优买单（最高价），通常需要处理。
    /// 这里为了简单，我们假设存储时 Key 为 Decimal，遍历时从后往前即为从高到低。
    pub bids: BTreeMap<Decimal, Decimal>,

    pub update_id: u64,
}

impl OrderBook {
    pub fn new(pair: TradingPair) -> Self {
        Self {
            pair,
            asks: BTreeMap::new(),
            bids: BTreeMap::new(),
            update_id: 0,
        }
    }

    /// 应用快照 (重绘整个画布)
    pub fn apply_snapshot(&mut self, asks: Vec<Level>, bids: Vec<Level>, update_id: u64) {
        self.asks.clear();
        self.bids.clear();

        for level in asks {
            self.asks.insert(level.price, level.quantity);
        }

        for level in bids {
            self.bids.insert(level.price, level.quantity);
        }

        self.update_id = update_id;
    }

    /// 应用增量更新 (修改画布的一部分)
    /// quantity 为 0 表示删除该层级
    pub fn apply_delta(&mut self, side: Side, levels: Vec<Level>, update_id: u64) {
        let book_side = match side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        };

        for level in levels {
            if level.quantity.is_zero() {
                book_side.remove(&level.price);
            } else {
                book_side.insert(level.price, level.quantity);
            }
        }

        self.update_id = update_id;
    }

    pub fn best_ask(&self) -> Option<Level> {
        self.asks.first_key_value().map(|(p, q)| Level {
            price: *p,
            quantity: *q,
        })
    }

    pub fn best_bid(&self) -> Option<Level> {
        self.bids.last_key_value().map(|(p, q)| Level {
            price: *p,
            quantity: *q,
        })
    }
}
