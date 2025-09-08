// Mining algorithms module
// Contains CN-UPX/2 and other mining algorithm implementations

pub mod cn_upx2;

pub use cn_upx2::{
    CnUpX2Miner, CnUpX2Hash, CnUpX2Stats, CnUpX2State, CnUpX2Mode, CnUpX2Params,
    calculate_cn_upx2_hash, verify_cn_upx2_hash,
    CN_UPX2_MEMORY_SIZE, CN_UPX2_ITERATIONS, CN_UPX2_AES_ROUNDS,
    CN_UPX2_MM_MEMORY_SIZE, CN_UPX2_MM_ITERATIONS, CN_UPX2_MM_AES_ROUNDS,
};
