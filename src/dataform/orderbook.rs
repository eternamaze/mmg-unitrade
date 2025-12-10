use crate::common::account_model::{AssetIdentity, TradingPair};
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
pub struct OrderBook<A: AssetIdentity> {
    pub pair: TradingPair<A>,
    /// 卖单：价格从低到高排序
    pub asks: BTreeMap<Decimal, Decimal>,
    /// 买单：价格从高到低排序 (使用 Reverse 包装或自定义比较，这里简化为 BTreeMap，遍历时需注意)
    /// Rust 的 BTreeMap 默认升序。为了方便获取最优买单（最高价），通常需要处理。
    /// 这里为了简单，我们假设存储时 Key 为 Decimal，遍历时从后往前即为从高到低。
    pub bids: BTreeMap<Decimal, Decimal>,

    pub update_id: u64,
}

impl<A: AssetIdentity> OrderBook<A> {
    pub fn new(pair: TradingPair<A>) -> Self {
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
}
