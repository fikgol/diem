use super::event_processor::verify_block_for_pow;
use crate::pow::chain_manager::ChainManager;
use crate::pow::event_processor::EventProcessor;
use anyhow::Result;
use channel;
use consensus_types::block_retrieval::{
    BlockRetrievalResponse, BlockRetrievalStatus, PowBlockRetrievalRequest,
};
use consensus_types::{block::Block, payload_ext::BlockPayloadExt};
use futures::compat::Future01CompatExt;
use futures::SinkExt;
use futures::{channel::mpsc, StreamExt};
use futures_locks::Mutex;
use libra_crypto::HashValue;
use libra_logger::prelude::*;
use libra_types::account_address::AccountAddress;
use libra_types::PeerId;
use network::{
    proto::{
        ConsensusMsg,
        ConsensusMsg_oneof::{self},
    },
    validator_network::{ConsensusNetworkSender, Event},
};
use std::collections::HashMap;
use std::convert::TryInto;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::runtime::Handle;

pub struct SyncManager {
    inner: SyncInner,
}

#[derive(Clone)]
struct SyncInner {
    author: AccountAddress,
    self_sender: channel::Sender<Result<Event<ConsensusMsg>>>,
    network_sender: ConsensusNetworkSender,
    block_cache_sender: mpsc::Sender<Block<BlockPayloadExt>>,
    chain_manager: Arc<ChainManager>,
    sync_block_cache: Arc<Mutex<HashMap<PeerId, Vec<Block<BlockPayloadExt>>>>>,
    sync_block_height: Arc<AtomicU64>,
    dev_mode: bool,
    begin_mint_sender: mpsc::Sender<()>,
}

impl SyncManager {
    pub fn new(
        author: AccountAddress,
        self_sender: channel::Sender<Result<Event<ConsensusMsg>>>,
        network_sender: ConsensusNetworkSender,
        block_cache_sender: mpsc::Sender<Block<BlockPayloadExt>>,
        chain_manager: Arc<ChainManager>,
        dev_mode: bool,
        begin_mint_sender: mpsc::Sender<()>,
    ) -> Self {
        let inner = SyncInner {
            author,
            self_sender,
            network_sender,
            block_cache_sender,
            chain_manager,
            sync_block_cache: Arc::new(Mutex::new(HashMap::new())),
            sync_block_height: Arc::new(AtomicU64::new(0)),
            dev_mode,
            begin_mint_sender,
        };

        SyncManager { inner }
    }

    pub fn sync_block_msg(
        &self,
        executor: Handle,
        mut sync_block_receiver: mpsc::Receiver<(PeerId, BlockRetrievalResponse<BlockPayloadExt>)>,
        mut sync_signal_receiver: mpsc::Receiver<(PeerId, (u64, HashValue))>,
        mut sync_stop_receiver: mpsc::Receiver<()>,
    ) {
        let sync_inner = self.inner.clone();

        let sync_fut = async move {
            loop {
                ::futures::select! {
                    (peer_id, (height, root_hash)) = sync_signal_receiver.select_next_some() => {
                        //1. sync data from latest block
                        //TODO:timeout
                        println!("--------sync_signal_receiver-----000----");
                        if sync_inner.chain_manager.is_run().await {
                            println!("--------sync_signal_receiver-----222----");
                            if sync_inner.sync_block_height.load(Ordering::Relaxed) < height {
                                sync_inner.sync_block_height.store(height, Ordering::Relaxed);
                                let sync_block_req_msg = Self::sync_block_req(false, height, root_hash);
                                EventProcessor::send_consensus_msg(peer_id, &mut sync_inner.network_sender.clone(), sync_inner.author.clone(),
                                    &mut sync_inner.self_sender.clone(), sync_block_req_msg).await;
                            }
                        } else {
                            println!("--------sync_signal_receiver-----111----");
                            let sync_block_req_msg = Self::sync_block_req(true, height, root_hash);
                            EventProcessor::send_consensus_msg(peer_id, &mut sync_inner.network_sender.clone(), sync_inner.author.clone(),
                                &mut sync_inner.self_sender.clone(), sync_block_req_msg).await;
                        }
                    },
                    (peer_id, sync_block_resp) = sync_block_receiver.select_next_some() => {
                        println!("--------sync_block_receiver-----000----");
                        if sync_inner.chain_manager.is_run().await {
                            println!("--------sync_block_receiver-----111----");
                            Self::handle_desc_sync_resp(sync_inner.clone(), peer_id, sync_block_resp).await;
                        } else {
                            println!("--------sync_block_receiver-----222----");
                            Self::handle_asc_sync_resp(sync_inner.clone(), peer_id, sync_block_resp).await;
                        }
                    }
                    _ = sync_stop_receiver.select_next_some() => {
                        break;
                    }
                    complete => {
                        break;
                    }
                }
            }
        };
        executor.spawn(sync_fut);
    }

    async fn handle_asc_sync_resp(
        sync_inner: SyncInner,
        peer_id: PeerId,
        sync_block_resp: BlockRetrievalResponse<BlockPayloadExt>,
    ) {
        println!("-----------asc-------------");
        // 2. save data to cache
        let status = sync_block_resp.status();
        debug!("Sync block from {:?}, status : {:?}", peer_id, status);
        let blocks = sync_block_resp.blocks();

        let mut err_block_flag = false;
        let mut end_block_hash = None;
        let mut latest_height = 0;
        if blocks.len() > 0 {
            for block in blocks {
                if verify_block_for_pow(block, sync_inner.dev_mode) {
                    // add to sync_block_cache
                    end_block_hash = Some(block.id());

                    latest_height = block.round();
                    sync_inner
                        .block_cache_sender
                        .clone()
                        .send(block.clone())
                        .await
                        .expect("send block err.");
                } else {
                    err_block_flag = true;
                    warn!("sync block verify failed.");
                    break;
                }
            }
        }

        if !err_block_flag {
            match status {
                BlockRetrievalStatus::Succeeded => {
                    let asc = !sync_inner.chain_manager.is_run().await;
                    let sync_block_req_msg =
                        Self::sync_block_req(asc, latest_height, end_block_hash.unwrap());
                    EventProcessor::send_consensus_msg(
                        peer_id,
                        &mut sync_inner.network_sender.clone(),
                        sync_inner.author.clone(),
                        &mut sync_inner.self_sender.clone(),
                        sync_block_req_msg,
                    )
                    .await;
                }
                _ => {
                    //                                    BlockRetrievalStatus::IdNotFound
                    //                                    BlockRetrievalStatus::NotEnoughBlocks
                    let _ = sync_inner.begin_mint_sender.clone().send(()).await;
                }
            };
        }
    }

    async fn handle_desc_sync_resp(
        sync_inner: SyncInner,
        peer_id: PeerId,
        sync_block_resp: BlockRetrievalResponse<BlockPayloadExt>,
    ) {
        println!("-----------desc-------------");
        // 2. save data to cache
        let status = sync_block_resp.status();
        debug!("Sync block from {:?}, status : {:?}", peer_id, status);
        let blocks = sync_block_resp.blocks();

        let mut end_flag = false;
        let mut err_block_flag = false;
        let mut end_block_hash = None;
        let mut latest_height = 0;
        let mut sync_block_cache_lock = sync_inner
            .sync_block_cache
            .clone()
            .lock()
            .compat()
            .await
            .unwrap();
        if blocks.len() > 0 {
            if !sync_block_cache_lock.contains_key(&peer_id) {
                let block_vec = Vec::new();
                sync_block_cache_lock.insert(peer_id, block_vec);
            }
            for block in blocks {
                if verify_block_for_pow(block, sync_inner.dev_mode) {
                    let tmp_hash = block.id();
                    if sync_inner.chain_manager.block_exist(&tmp_hash).await {
                        end_flag = true;
                        break;
                    };

                    // add to sync_block_cache
                    end_block_hash = Some(block.parent_id());
                    sync_block_cache_lock
                        .get_mut(&peer_id)
                        .expect("peer block not exist.")
                        .push(block.clone());

                    latest_height = block.round();
                } else {
                    err_block_flag = true;
                    let _ = sync_block_cache_lock.remove(&peer_id);
                }
            }
        }

        if end_flag {
            let mut block_vec = sync_block_cache_lock
                .remove(&peer_id)
                .expect("peer block not exist.");
            block_vec.reverse();
            for b in block_vec {
                sync_inner
                    .block_cache_sender
                    .clone()
                    .send(b)
                    .await
                    .expect("send block err.");
            }
            info!("Sync block from {:?} end", peer_id);
        } else {
            if !err_block_flag {
                match status {
                    BlockRetrievalStatus::Succeeded => {
                        let asc = !sync_inner.chain_manager.is_run().await;
                        let sync_block_req_msg =
                            Self::sync_block_req(asc, latest_height, end_block_hash.unwrap());
                        EventProcessor::send_consensus_msg(
                            peer_id,
                            &mut sync_inner.network_sender.clone(),
                            sync_inner.author.clone(),
                            &mut sync_inner.self_sender.clone(),
                            sync_block_req_msg,
                        )
                        .await;
                    }
                    _ => {
                        //                                    BlockRetrievalStatus::IdNotFound
                        //                                    BlockRetrievalStatus::NotEnoughBlocks
                        let _ = sync_block_cache_lock.remove(&peer_id);
                    }
                };
            }
        }
    }

    fn sync_block_req(asc: bool, height: u64, hash: HashValue) -> ConsensusMsg {
        let num_blocks = 10;

        let req = PowBlockRetrievalRequest::new(height, hash, num_blocks, asc)
            .try_into()
            .expect("PowBlockRetrievalRequest pb err.");

        ConsensusMsg {
            message: Some(ConsensusMsg_oneof::PowRequestBlock(req)),
        }
    }
}
