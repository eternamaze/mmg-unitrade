use crate::common::account_model::{AssetIdentity, SubAccountIdentity};
use crate::common::actor_trait::{AccessChannel, Actor, ChannelPair};
use crate::dataform::kline::KlineSeries;
use crate::dataform::orderbook::OrderBook;
use crate::dataform::tradeflow::TradeFlow;
use crate::exchange::exchange_domain::{
    ConnectorRequest, FeedCommand, KlineDomain, OrderBookDomain, TradeFlowDomain,
};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast, mpsc};

use crate::exchange::exchange_connector_template::ConnectorChannels;

// --- Feed Control Language ---

// Moved to exchange_domain.rs

/// [API] 交易所行情源 (Feed)
///
/// **角色：框架标准接口 (Framework Standard Interface)**
/// **使用者：框架用户 (Framework User)**
///
/// 负责维护市场数据的实时状态（Canvas）。
/// 它通过 AccessChannel 提供对 OrderBook, TradeFlow, Kline 的访问。
/// 它是交易所三元组中的“感知层”。
///
/// 使用关联类型 Id 来支持不同的市场寻址方式（如 TradingPair 或 Instrument）。
pub trait ExchangeFeed<A: AssetIdentity, S: SubAccountIdentity>: Actor {
    type Id: Send + Sync + 'static + Clone + std::hash::Hash + Eq;

    /// 使用连接器信道创建 Feed 实例
    ///
    /// Feed 不再持有 Connector 的引用，而是直接持有与 Connector 通信的信道。
    /// 这遵循了 Actor 模型“只共享信道，不共享本体”的原则。
    fn new(
        connector_tx: mpsc::Sender<ConnectorRequest<Self::Id, S>>,
        connector_channels: ConnectorChannels<Self::Id, A, S>,
    ) -> Self;
}

// --- Standard Implementation ---

/// 标准行情源模板
///
/// 这是一个泛型结构体，实现了 `ExchangeFeed`。
/// 它自动处理订阅逻辑，并将数据通道暴露给外部。
/// 它不再依赖 `Connector` 本体，而是依赖 `connector_tx`。
pub struct StandardFeed<I, A, S>
where
    S: SubAccountIdentity,
    A: AssetIdentity,
{
    connector_tx: mpsc::Sender<ConnectorRequest<I, S>>,
    connector_channels: ConnectorChannels<I, A, S>,
    command_tx: mpsc::Sender<FeedCommand<I>>,
    command_rx: Arc<Mutex<Option<mpsc::Receiver<FeedCommand<I>>>>>,
    
    // User-facing channels (owned by Feed)
    orderbook_txs: Arc<Mutex<std::collections::HashMap<I, broadcast::Sender<OrderBook<A>>>>>,
    trade_txs: Arc<Mutex<std::collections::HashMap<I, broadcast::Sender<TradeFlow<A>>>>>,
    kline_txs: Arc<Mutex<std::collections::HashMap<I, broadcast::Sender<KlineSeries<A>>>>>,
}

impl<I, A, S> StandardFeed<I, A, S>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
    A: AssetIdentity,
    S: SubAccountIdentity,
{
    pub fn new(
        connector_tx: mpsc::Sender<ConnectorRequest<I, S>>,
        connector_channels: ConnectorChannels<I, A, S>,
    ) -> Self {
        let (command_tx, command_rx) = mpsc::channel(100);
        Self {
            connector_tx,
            connector_channels,
            command_tx,
            command_rx: Arc::new(Mutex::new(Some(command_rx))),
            orderbook_txs: Arc::new(Mutex::new(std::collections::HashMap::new())),
            trade_txs: Arc::new(Mutex::new(std::collections::HashMap::new())),
            kline_txs: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    fn get_user_orderbook_tx(&self, instrument: &I) -> broadcast::Sender<OrderBook<A>> {
        let mut map = self.orderbook_txs.blocking_lock();
        map.entry(instrument.clone())
            .or_insert_with(|| {
                let (tx, _) = broadcast::channel(100);
                tx
            })
            .clone()
    }

    fn get_user_trade_tx(&self, instrument: &I) -> broadcast::Sender<TradeFlow<A>> {
        let mut map = self.trade_txs.blocking_lock();
        map.entry(instrument.clone())
            .or_insert_with(|| {
                let (tx, _) = broadcast::channel(100);
                tx
            })
            .clone()
    }

    fn get_user_kline_tx(&self, instrument: &I) -> broadcast::Sender<KlineSeries<A>> {
        let mut map = self.kline_txs.blocking_lock();
        map.entry(instrument.clone())
            .or_insert_with(|| {
                let (tx, _) = broadcast::channel(100);
                tx
            })
            .clone()
    }
}

impl<I, A, S> ExchangeFeed<A, S> for StandardFeed<I, A, S>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
    A: AssetIdentity,
    S: SubAccountIdentity,
{
    type Id = I;

    fn new(
        connector_tx: mpsc::Sender<ConnectorRequest<Self::Id, S>>,
        connector_channels: ConnectorChannels<Self::Id, A, S>,
    ) -> Self {
        Self::new(connector_tx, connector_channels)
    }
}

#[async_trait]
impl<I, A, S> Actor for StandardFeed<I, A, S>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
    A: AssetIdentity,
    S: SubAccountIdentity,
{
    async fn start(&self) -> anyhow::Result<()> {
        let mut command_rx = self
            .command_rx
            .lock()
            .await
            .take()
            .ok_or(anyhow::anyhow!("Command RX already taken"))?;
        
        let request_tx = self.connector_tx.clone();
        let connector_channels = self.connector_channels.clone();
        
        // We need to access self.orderbook_txs etc. inside the loop, but self is borrowed.
        // So we clone the Arcs.
        let user_orderbook_txs = self.orderbook_txs.clone();
        let user_trade_txs = self.trade_txs.clone();
        let user_kline_txs = self.kline_txs.clone();

        tokio::spawn(async move {
            while let Some(cmd) = command_rx.recv().await {
                match cmd {
                    FeedCommand::Track {
                        instrument,
                        channel_type,
                    } => {
                        // 1. Send request to Connector
                        let req = ConnectorRequest::EstablishDataStream {
                            instrument: instrument.clone(),
                            channel_type,
                        };
                        if let Err(e) = request_tx.send(req).await {
                            eprintln!("Failed to send subscription request: {}", e);
                            continue;
                        }

                        // 2. Setup forwarding
                        match channel_type {
                            crate::exchange::exchange_domain::ChannelType::OrderBook => {
                                let connector_tx = connector_channels.get_orderbook_tx(&instrument);
                                let mut connector_rx = connector_tx.subscribe();
                                
                                let user_tx = {
                                    let mut map = user_orderbook_txs.lock().await;
                                    map.entry(instrument.clone())
                                        .or_insert_with(|| {
                                            let (tx, _) = broadcast::channel(100);
                                            tx
                                        })
                                        .clone()
                                };

                                tokio::spawn(async move {
                                    while let Ok(data) = connector_rx.recv().await {
                                        if let Err(_) = user_tx.send(data) {
                                            break; // No subscribers
                                        }
                                    }
                                });
                            }
                            crate::exchange::exchange_domain::ChannelType::TradeFlow => {
                                let connector_tx = connector_channels.get_trade_tx(&instrument);
                                let mut connector_rx = connector_tx.subscribe();
                                
                                let user_tx = {
                                    let mut map = user_trade_txs.lock().await;
                                    map.entry(instrument.clone())
                                        .or_insert_with(|| {
                                            let (tx, _) = broadcast::channel(100);
                                            tx
                                        })
                                        .clone()
                                };

                                tokio::spawn(async move {
                                    while let Ok(data) = connector_rx.recv().await {
                                        if let Err(_) = user_tx.send(data) {
                                            break;
                                        }
                                    }
                                });
                            }
                            crate::exchange::exchange_domain::ChannelType::Kline(_) => {
                                let connector_tx = connector_channels.get_kline_tx(&instrument);
                                let mut connector_rx = connector_tx.subscribe();
                                
                                let user_tx = {
                                    let mut map = user_kline_txs.lock().await;
                                    map.entry(instrument.clone())
                                        .or_insert_with(|| {
                                            let (tx, _) = broadcast::channel(100);
                                            tx
                                        })
                                        .clone()
                                };

                                tokio::spawn(async move {
                                    while let Ok(data) = connector_rx.recv().await {
                                        if let Err(_) = user_tx.send(data) {
                                            break;
                                        }
                                    }
                                });
                            }
                            _ => {}
                        }
                    }
                    FeedCommand::Untrack {
                        instrument,
                        channel_type,
                    } => {
                        let req = ConnectorRequest::StopDataStream {
                            instrument,
                            channel_type,
                        };
                        if let Err(e) = request_tx.send(req).await {
                            eprintln!("Failed to send unsubscribe request: {}", e);
                        }
                        // TODO: Stop the forwarding task?
                        // Currently we rely on Connector stopping the stream or just letting it run.
                        // To properly stop, we'd need a cancellation token map.
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

impl<I, A, S> AccessChannel<OrderBookDomain> for StandardFeed<I, A, S>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
    A: AssetIdentity,
    S: SubAccountIdentity,
{
    type Id = I;
    type Sender = mpsc::Sender<FeedCommand<I>>;
    type Receiver = broadcast::Receiver<OrderBook<A>>;

    fn access_channel(
        &self,
        _domain: OrderBookDomain,
        id: Self::Id,
    ) -> anyhow::Result<ChannelPair<Self::Sender, Self::Receiver>> {
        let tx = self.get_user_orderbook_tx(&id);
        Ok(ChannelPair {
            tx: Some(self.command_tx.clone()),
            rx: Some(tx.subscribe()),
        })
    }
}

impl<I, A, S> AccessChannel<TradeFlowDomain> for StandardFeed<I, A, S>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
    A: AssetIdentity,
    S: SubAccountIdentity,
{
    type Id = I;
    type Sender = mpsc::Sender<FeedCommand<I>>;
    type Receiver = broadcast::Receiver<TradeFlow<A>>;

    fn access_channel(
        &self,
        _domain: TradeFlowDomain,
        id: Self::Id,
    ) -> anyhow::Result<ChannelPair<Self::Sender, Self::Receiver>> {
        let tx = self.get_user_trade_tx(&id);
        Ok(ChannelPair {
            tx: Some(self.command_tx.clone()),
            rx: Some(tx.subscribe()),
        })
    }
}

impl<I, A, S> AccessChannel<KlineDomain> for StandardFeed<I, A, S>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
    A: AssetIdentity,
    S: SubAccountIdentity,
{
    type Id = I;
    type Sender = mpsc::Sender<FeedCommand<I>>;
    type Receiver = broadcast::Receiver<KlineSeries<A>>;

    fn access_channel(
        &self,
        _domain: KlineDomain,
        id: Self::Id,
    ) -> anyhow::Result<ChannelPair<Self::Sender, Self::Receiver>> {
        let tx = self.get_user_kline_tx(&id);
        Ok(ChannelPair {
            tx: Some(self.command_tx.clone()),
            rx: Some(tx.subscribe()),
        })
    }
}
