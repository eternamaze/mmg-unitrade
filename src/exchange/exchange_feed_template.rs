use crate::common::actor_trait::{AccessChannel, Actor, ChannelPair};
use crate::dataform::kline::KlineSeries;
use crate::dataform::orderbook::OrderBook;
use crate::dataform::tradeflow::TradeFlow;
use crate::exchange::exchange_connector_template::{StandardHook};
use crate::exchange::exchange_domain::{
    ConnectorRequest, FeedCommand, KlineDomain, OrderBookDomain, TradeFlowDomain,
};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast, mpsc};

// --- Feed Control Language ---

// Moved to exchange_domain.rs

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

// --- Standard Implementation ---

/// 标准行情源模板
///
/// 这是一个泛型结构体，实现了 `ExchangeFeed`。
/// 它自动处理订阅逻辑，并将数据通道暴露给外部。
/// 它依赖于 `StandardHook` 来与连接器交互。
pub struct StandardFeed<I> {
    hook: StandardHook<I>,
    command_tx: mpsc::Sender<FeedCommand<I>>,
    command_rx: Arc<Mutex<Option<mpsc::Receiver<FeedCommand<I>>>>>,
}

impl<I> StandardFeed<I>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
{
    pub fn new(hook: StandardHook<I>) -> Self {
        let (command_tx, command_rx) = mpsc::channel(100);
        Self {
            hook,
            command_tx,
            command_rx: Arc::new(Mutex::new(Some(command_rx))),
        }
    }
}

impl<I> ExchangeFeed for StandardFeed<I>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
{
    type Id = I;
    type ConnectorHook = StandardHook<I>;

    fn new(hook: Self::ConnectorHook) -> Self {
        Self::new(hook)
    }
}

#[async_trait]
impl<I> Actor for StandardFeed<I>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
{
    async fn start(&self) -> anyhow::Result<()> {
        let mut command_rx = self
            .command_rx
            .lock()
            .await
            .take()
            .ok_or(anyhow::anyhow!("Command RX already taken"))?;
        let request_tx = self.hook.request_tx.clone();

        tokio::spawn(async move {
            while let Some(cmd) = command_rx.recv().await {
                match cmd {
                    FeedCommand::Subscribe {
                        instrument,
                        channel_type,
                    } => {
                        let req = ConnectorRequest::Subscribe {
                            instrument,
                            channel_type,
                        };
                        if let Err(e) = request_tx.send(req).await {
                            eprintln!("Failed to send subscription request: {}", e);
                        }
                    }
                    FeedCommand::Unsubscribe {
                        instrument,
                        channel_type,
                    } => {
                        let req = ConnectorRequest::Unsubscribe {
                            instrument,
                            channel_type,
                        };
                        if let Err(e) = request_tx.send(req).await {
                            eprintln!("Failed to send unsubscribe request: {}", e);
                        }
                    }
                    _ => {} // TODO: Implement other commands
                }
            }
        });
        Ok(())
    }

    async fn stop(&self) -> anyhow::Result<()> {
        Ok(())
    }
}

// --- Access Channels ---

impl<I> AccessChannel<OrderBookDomain> for StandardFeed<I>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
{
    type Id = I;
    type Sender = mpsc::Sender<FeedCommand<I>>;
    type Receiver = broadcast::Receiver<OrderBook>;

    fn access_channel(
        &self,
        _domain: OrderBookDomain,
        id: Self::Id,
    ) -> anyhow::Result<ChannelPair<Self::Sender, Self::Receiver>> {
        let tx = self.hook.channels.get_orderbook_tx(&id);
        Ok(ChannelPair {
            tx: Some(self.command_tx.clone()),
            rx: Some(tx.subscribe()),
        })
    }
}

impl<I> AccessChannel<TradeFlowDomain> for StandardFeed<I>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
{
    type Id = I;
    type Sender = mpsc::Sender<FeedCommand<I>>;
    type Receiver = broadcast::Receiver<TradeFlow>;

    fn access_channel(
        &self,
        _domain: TradeFlowDomain,
        id: Self::Id,
    ) -> anyhow::Result<ChannelPair<Self::Sender, Self::Receiver>> {
        let tx = self.hook.channels.get_trade_tx(&id);
        Ok(ChannelPair {
            tx: Some(self.command_tx.clone()),
            rx: Some(tx.subscribe()),
        })
    }
}

impl<I> AccessChannel<KlineDomain> for StandardFeed<I>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
{
    type Id = I;
    type Sender = mpsc::Sender<FeedCommand<I>>;
    type Receiver = broadcast::Receiver<KlineSeries>;

    fn access_channel(
        &self,
        _domain: KlineDomain,
        id: Self::Id,
    ) -> anyhow::Result<ChannelPair<Self::Sender, Self::Receiver>> {
        let tx = self.hook.channels.get_kline_tx(&id);
        Ok(ChannelPair {
            tx: Some(self.command_tx.clone()),
            rx: Some(tx.subscribe()),
        })
    }
}
