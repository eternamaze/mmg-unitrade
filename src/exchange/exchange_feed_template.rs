use crate::common::actor_trait::Actor;

// --- Domain Identifiers ---
pub struct OrderBookDomain;
pub struct TradeFlowDomain;
pub struct KlineDomain;

// --- Feed Control Language ---

/// Feed 控制指令
/// 策略层通过此指令控制 Feed 的行为。
/// 使用泛型 `I` (Instrument) 来指定操作对象。
#[derive(Debug, Clone)]
pub enum FeedCommand<I> {
    /// 订阅特定标的的特定数据流
    Subscribe {
        instrument: I,
        channel_type: ChannelType,
    },
    /// 取消订阅
    Unsubscribe {
        instrument: I,
        channel_type: ChannelType,
    },
    /// 请求立即推送一次全量快照
    RequestSnapshot { instrument: I },
    /// 暂停推送（节省带宽）
    Suspend { instrument: I },
    /// 恢复推送
    Resume { instrument: I },
    /// 设置聚合频率（毫秒），0 表示实时推送
    SetFrequency { instrument: I, interval_ms: u64 },
}

use crate::exchange::exchange_connector_template::ChannelType;

/// 交易所行情源 (Feed)
///
/// 负责维护市场数据的实时状态（Canvas）。
/// 它通过 AccessChannel 提供对 OrderBook, TradeFlow, Kline 的访问。
/// 它是交易所三元组中的“感知层”。
///
/// 使用关联类型 Id 来支持不同的市场寻址方式（如 TradingPair 或 Instrument）。
pub trait ExchangeFeed: Actor {
    type Id: Send + Sync + 'static + Clone + std::hash::Hash + Eq;
    /// 连接器提供的挂钩 (Hook)
    type ConnectorHook: Clone + Send + Sync + 'static;

    /// 使用连接器提供的挂钩创建 Feed 实例
    fn new(hook: Self::ConnectorHook) -> Self;
}

// 移除 ExchangeFeedAccess trait，直接在 Exchange 模板中使用 where 子句约束
// 这样可以避免 trait 定义中的递归依赖问题
