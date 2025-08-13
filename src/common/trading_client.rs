use super::trading_endpoint::TradingEndpoint;
use crate::errors::trading_endpoint_error::TradingEndpointError;
use crate::{
    dex::{dex_traits::DexTrait, types::DexType},
    swqos::SWQoSType,
};
use solana_client::nonblocking::rpc_client::RpcClient;
use std::{collections::HashMap, sync::Arc};

pub struct TradingConfig {
    pub rpc_url: String,
    pub swqos: Vec<SWQoSType>,
}

pub struct TradingClient {
    pub endpoint: Arc<TradingEndpoint>,
    pub dexs: HashMap<DexType, Arc<dyn DexTrait>>,
}

impl TradingClient {
    pub fn new(config: TradingConfig) -> anyhow::Result<Self> {
        let rpc = Arc::new(RpcClient::new(config.rpc_url));
        let swqos = config.swqos.into_iter().map(|swqos| swqos.instantiate(rpc.clone())).collect();
        let endpoint = Arc::new(TradingEndpoint::new(rpc, swqos));
        let dexs = DexType::all().into_iter().map(|dex| (dex, dex.instantiate(endpoint.clone()))).collect();

        Ok(Self { endpoint, dexs })
    }

    pub async fn initialize(&self) -> Result<(), TradingEndpointError> {
        for (_, dex) in &self.dexs {
            dex.initialize().await?;
        }
        Ok(())
    }
}
