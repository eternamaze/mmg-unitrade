use crate::common::account_model::SubAccountId;
use crate::common::actor_trait::{AccessChannel, Actor, ChannelPair};
use crate::dataform::kline::KlineSeries;
use crate::dataform::orderbook::OrderBook;
use crate::dataform::tradeflow::TradeFlow;
use crate::exchange::exchange_connector_template::ExchangeConnector;
use crate::exchange::exchange_feed_template::{
    ExchangeFeed, FeedCommand, KlineDomain, OrderBookDomain, TradeFlowDomain,
};
use crate::exchange::exchange_gateway_template::{
    BalanceCommand, BalanceDomain, BalanceUpdate, ExchangeGateway, OrderCommand, OrderDomain,
    OrderUpdate, PositionCommand, PositionDomain, PositionUpdate,
};
use async_trait::async_trait;
use std::marker::PhantomData;
use tokio::sync::{broadcast, mpsc};

/// 交易所三元组 (Exchange Triad)
///
/// 这是一个泛型结构体，用于组合 Connector, Feed, Gateway。
/// 它本身也是一个 Actor，负责管理这三个组件的生命周期。
/// 它是外部交互的唯一入口（Facade）。
///
/// 外部使用者通过 Exchange 获取所有业务域的信道，
/// Exchange 内部会将请求转发给 Feed 或 Gateway。
///
/// 泛型 `I` 代表 Instrument (标的) 的标识符类型。
pub struct Exchange<C, F, G, I> {
    /// 连接器是内部基础设施，不对外暴露。
    /// 外部只能通过 Feed 和 Gateway 与系统交互。
    connector: Option<C>,
    feed: Option<F>,
    gateway: Option<G>,
    _marker: PhantomData<I>,
}

impl<C, F, G, I> Exchange<C, F, G, I>
where
    C: ExchangeConnector,
    F: ExchangeFeed<Id = I>,
    G: ExchangeGateway,
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq,
{
    pub fn new(connector: C, feed: F, gateway: G) -> Self {
        Self {
            connector: Some(connector),
            feed: Some(feed),
            gateway: Some(gateway),
            _marker: PhantomData,
        }
    }
}

#[async_trait]
impl<C, F, G, I> Actor for Exchange<C, F, G, I>
where
    C: ExchangeConnector,
    F: ExchangeFeed<Id = I>,
    G: ExchangeGateway,
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq,
{
    async fn start(&self) -> anyhow::Result<()> {
        if let Some(c) = &self.connector {
            c.start().await?;
        }
        if let Some(f) = &self.feed {
            f.start().await?;
        }
        if let Some(g) = &self.gateway {
            g.start().await?;
        }
        Ok(())
    }

    async fn stop(&self) -> anyhow::Result<()> {
        // Stop in reverse order
        if let Some(g) = &self.gateway {
            g.stop().await?;
        }
        if let Some(f) = &self.feed {
            f.stop().await?;
        }
        if let Some(c) = &self.connector {
            c.stop().await?;
        }
        Ok(())
    }
}

// --- Feed Channel Forwarding ---

impl<C, F, G, I> AccessChannel<OrderBookDomain> for Exchange<C, F, G, I>
where
    C: ExchangeConnector,
    F: ExchangeFeed<Id = I>,
    F: AccessChannel<
            OrderBookDomain,
            Id = I,
            Sender = mpsc::Sender<FeedCommand<I>>,
            Receiver = broadcast::Receiver<OrderBook>,
        >,
    G: ExchangeGateway,
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq,
{
    type Id = I;
    type Sender = mpsc::Sender<FeedCommand<I>>;
    type Receiver = broadcast::Receiver<OrderBook>;

    fn access_channel(
        &self,
        domain: OrderBookDomain,
        id: Self::Id,
    ) -> anyhow::Result<ChannelPair<Self::Sender, Self::Receiver>> {
        match &self.feed {
            Some(feed) => feed.access_channel(domain, id),
            None => Ok(ChannelPair::default()),
        }
    }
}

impl<C, F, G, I> AccessChannel<TradeFlowDomain> for Exchange<C, F, G, I>
where
    C: ExchangeConnector,
    F: ExchangeFeed<Id = I>,
    F: AccessChannel<
            TradeFlowDomain,
            Id = I,
            Sender = mpsc::Sender<FeedCommand<I>>,
            Receiver = broadcast::Receiver<TradeFlow>,
        >,
    G: ExchangeGateway,
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq,
{
    type Id = I;
    type Sender = mpsc::Sender<FeedCommand<I>>;
    type Receiver = broadcast::Receiver<TradeFlow>;

    fn access_channel(
        &self,
        domain: TradeFlowDomain,
        id: Self::Id,
    ) -> anyhow::Result<ChannelPair<Self::Sender, Self::Receiver>> {
        match &self.feed {
            Some(feed) => feed.access_channel(domain, id),
            None => Ok(ChannelPair::default()),
        }
    }
}

impl<C, F, G, I> AccessChannel<KlineDomain> for Exchange<C, F, G, I>
where
    C: ExchangeConnector,
    F: ExchangeFeed<Id = I>,
    F: AccessChannel<
            KlineDomain,
            Id = I,
            Sender = mpsc::Sender<FeedCommand<I>>,
            Receiver = broadcast::Receiver<KlineSeries>,
        >,
    G: ExchangeGateway,
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq,
{
    type Id = I;
    type Sender = mpsc::Sender<FeedCommand<I>>;
    type Receiver = broadcast::Receiver<KlineSeries>;

    fn access_channel(
        &self,
        domain: KlineDomain,
        id: Self::Id,
    ) -> anyhow::Result<ChannelPair<Self::Sender, Self::Receiver>> {
        match &self.feed {
            Some(feed) => feed.access_channel(domain, id),
            None => Ok(ChannelPair::default()),
        }
    }
}

/// 交易所三元组 (Exchange Triad)
///
/// 这是一个泛型结构体，用于组合 Connector, Feed, Gateway。
/// 它本身也是一个 Actor，负责管理这三个组件的生命周期。
/// 它是外部交互的唯一入口（Facade）。
///
/// 外部使用者通过 Exchange 获取所有业务域的信道，
/// Exchange 内部会将请求转发给 Feed 或 Gateway。
impl<C, F, G, I> Exchange<C, F, G, I>
where
    C: ExchangeConnector,
    F: ExchangeFeed<Id = I, ConnectorHook = C::Hook>,
    G: ExchangeGateway<ConnectorHook = C::Hook>,
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq,
{
    /// 构造函数：强制使用 Connector 创建 Feed 和 Gateway
    ///
    /// 这是一个“工厂方法”，它封装了组件的创建过程。
    /// 外部用户只需要提供 Connector，Exchange 会自动创建并组装 Feed 和 Gateway。
    /// 这确保了 Feed 和 Gateway 必须复用同一个 Connector 的挂钩 (Hook)。
    pub fn build(connector: C) -> Self {
        let hook = connector.create_hook();
        let feed = Some(F::new(hook.clone()));
        let gateway = Some(G::new(hook));

        Self {
            connector: Some(connector),
            feed,
            gateway,
            _marker: PhantomData,
        }
    }
}

// --- Gateway Channel Forwarding ---

impl<C, F, G, I> AccessChannel<OrderDomain> for Exchange<C, F, G, I>
where
    C: ExchangeConnector,
    F: ExchangeFeed<Id = I>,
    G: ExchangeGateway<Id = I>,
    G: AccessChannel<
            OrderDomain,
            Id = SubAccountId,
            Sender = mpsc::Sender<OrderCommand<I>>,
            Receiver = broadcast::Receiver<OrderUpdate<I>>,
        >,
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq,
{
    type Id = SubAccountId;
    type Sender = mpsc::Sender<OrderCommand<I>>;
    type Receiver = broadcast::Receiver<OrderUpdate<I>>;

    fn access_channel(
        &self,
        domain: OrderDomain,
        id: Self::Id,
    ) -> anyhow::Result<ChannelPair<Self::Sender, Self::Receiver>> {
        match &self.gateway {
            Some(gateway) => gateway.access_channel(domain, id),
            None => Ok(ChannelPair::default()),
        }
    }
}

impl<C, F, G, I> AccessChannel<PositionDomain> for Exchange<C, F, G, I>
where
    C: ExchangeConnector,
    F: ExchangeFeed<Id = I>,
    G: ExchangeGateway<Id = I>,
    G: AccessChannel<
            PositionDomain,
            Id = SubAccountId,
            Sender = mpsc::Sender<PositionCommand<I>>,
            Receiver = broadcast::Receiver<PositionUpdate>,
        >,
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq,
{
    type Id = SubAccountId;
    type Sender = mpsc::Sender<PositionCommand<I>>;
    type Receiver = broadcast::Receiver<PositionUpdate>;

    fn access_channel(
        &self,
        domain: PositionDomain,
        id: Self::Id,
    ) -> anyhow::Result<ChannelPair<Self::Sender, Self::Receiver>> {
        match &self.gateway {
            Some(gateway) => gateway.access_channel(domain, id),
            None => Ok(ChannelPair::default()),
        }
    }
}

impl<C, F, G, I> AccessChannel<BalanceDomain> for Exchange<C, F, G, I>
where
    C: ExchangeConnector,
    F: ExchangeFeed<Id = I>,
    G: ExchangeGateway<Id = I>,
    G: AccessChannel<
            BalanceDomain,
            Id = SubAccountId,
            Sender = mpsc::Sender<BalanceCommand>,
            Receiver = broadcast::Receiver<BalanceUpdate>,
        >,
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq,
{
    type Id = SubAccountId;
    type Sender = mpsc::Sender<BalanceCommand>;
    type Receiver = broadcast::Receiver<BalanceUpdate>;

    fn access_channel(
        &self,
        domain: BalanceDomain,
        id: Self::Id,
    ) -> anyhow::Result<ChannelPair<Self::Sender, Self::Receiver>> {
        match &self.gateway {
            Some(gateway) => gateway.access_channel(domain, id),
            None => Ok(ChannelPair::default()),
        }
    }
}
