pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}

pub struct BlockHeader {
    pub height: u64,
    pub prev_hash: [u8;32],
    pub merkle_root: [u8;32],
    pub timestamp: u64,
}

impl BlockHeader {
    pub fn hash(&self) -> [u8;32] {
        // TODO: proper hashing via ffi-cryptonote
        [0u8;32]
    }
}
