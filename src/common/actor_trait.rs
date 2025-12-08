use async_trait::async_trait;

/// 通用的通信管道对 (Channel Pair)
///
/// 包含一个发送端(Microphone)和一个接收端(Earpiece)。
/// 均使用 Option 包裹以支持灵活的初始化和所有权转移。
///
/// S: Sender Type (话筒 - 用于发送指令)
/// R: Receiver Type (听筒 - 用于接收反馈/事件)
#[derive(Debug)]
pub struct ChannelPair<S, R> {
    /// 话筒：用于向 Actor 发送指令或数据
    pub tx: Option<S>,
    /// 听筒：用于从 Actor 接收事件或数据
    pub rx: Option<R>,
}

impl<S, R> Default for ChannelPair<S, R> {
    fn default() -> Self {
        Self { tx: None, rx: None }
    }
}

impl<S, R> ChannelPair<S, R> {
    pub fn new(tx: Option<S>, rx: Option<R>) -> Self {
        Self { tx, rx }
    }
}

/// 标准 Actor 接口
///
/// 仅定义生命周期管理。
#[async_trait]
pub trait Actor: Send + Sync + 'static {
    /// 启动 Actor
    async fn start(&self) -> anyhow::Result<()>;

    /// 停止 Actor (可选)
    async fn stop(&self) -> anyhow::Result<()> {
        Ok(())
    }
}

/// 访问特定领域信道的接口
///
/// 这是一个“类型驱动”的接口。通过泛型 `D` (Domain) 来区分不同的信道。
/// 不同的 `D` 可以关联不同的 `Sender` 和 `Receiver` 类型。
///
/// 此外，引入了 `Id` 关联类型，允许在同一个领域下区分不同的信道实例（内容空间）。
/// 例如，在 `MarketData` 领域下，可以通过 `TradingPair` 作为 `Id` 来获取特定交易对的行情信道。
pub trait AccessChannel<D> {
    type Sender;
    type Receiver;
    type Id;

    /// 获取指定领域和ID的通信信道
    ///
    /// - `domain`: 领域标识符 (通常为 ZST)
    /// - `id`: 内容标识符 (用于区分同领域下的不同信道)
    fn access_channel(
        &self,
        domain: D,
        id: Self::Id,
    ) -> anyhow::Result<ChannelPair<Self::Sender, Self::Receiver>>;
}
