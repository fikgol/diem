use libra_crypto::HashValue;
use libra_logger::prelude::*;
use miner::types::{Algo, U256};

pub const BLOCK_WINDOW: u32 = 24;
pub const BLOCK_TIME_SEC: u32 = 5;

pub fn difficult_1_target() -> U256 {
    U256::max_value() / DIFF_1_HASH_TIMES.into()
}

pub const DIFF_1_HASH_TIMES: u32 = 5000;

pub fn current_hash_rate(target: &[u8]) -> u64 {
    // current_hash_rate = (difficult_1_target/target_current) * difficult_1_hash/block_per_esc
    let target_u256: U256 = target.into();
    ((difficult_1_target() / target_u256) * DIFF_1_HASH_TIMES).low_u64() / (BLOCK_TIME_SEC as u64)
}

pub fn get_next_work_required<B>(block_index: B, algo: Algo) -> U256
where
    B: TBlockIndex,
{
    let blocks = {
        let mut blocks: Vec<BlockInfo> = vec![];
        let mut count = 0;
        for b in block_index {
            if b.algo != algo {
                continue;
            }
            if b.timestamp == 0 {
                continue;
            }
            blocks.push(b);
            count += 1;
            if count == BLOCK_WINDOW {
                break;
            }
        }
        blocks
    };
    if blocks.len() <= 1 {
        info!(
            "Block length less than 1, set target to 1 difficult:{:?}",
            difficult_1_target()
        );
        return difficult_1_target();
    }
    let mut avg_time: u64 = 0;
    let mut avg_target = U256::zero();
    let mut latest_block_index = 0;
    let block_n = blocks.len() - 1;
    while latest_block_index < block_n {
        let solve_time =
            blocks[latest_block_index].timestamp - blocks[latest_block_index + 1].timestamp;
        avg_time += solve_time * (block_n - latest_block_index) as u64;
        debug!(
            "solve_time:{:?}, avg_time:{:?}, block_n:{:?}",
            solve_time, avg_time, block_n
        );
        avg_target = avg_target + blocks[latest_block_index].target / block_n.into();
        latest_block_index += 1
    }
    avg_time = avg_time / ((block_n as u64) * ((block_n + 1) as u64) / 2);
    if avg_time == 0 {
        avg_time = 1
    }
    let time_plan = BLOCK_TIME_SEC;
    // new_target = avg_target * avg_time_used/time_plan
    // avoid the target increase or reduce too fast.
    let new_target =
        if let Some(new_target) = (avg_target / time_plan.into()).checked_mul(avg_time.into()) {
            if new_target / 2.into() > avg_target {
                info!("target increase too fast, limit to 2 times");
                avg_target * 2
            } else if new_target < avg_target / 2.into() {
                info!("target reduce too fase, limit to 2 times");
                avg_target / 2.into()
            } else {
                new_target
            }
        } else {
            info!("target large than max value, set to 1_difficult");
            difficult_1_target()
        };
    info!(
        "avg_time:{:?}s, time_plan:{:?}s, target: {:?}",
        avg_time, time_plan, new_target
    );
    new_target
}

#[derive(Clone)]
pub struct BlockInfo {
    pub timestamp: u64,
    pub target: U256,
    pub algo: Algo,
}

pub trait TBlockIndex: Iterator<Item = BlockInfo> + Send + Sync + Clone {
    fn set_latest(&mut self, block: HashValue);
}
