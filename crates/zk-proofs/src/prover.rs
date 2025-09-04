use anyhow::Result;

pub struct ProvingKey;

impl ProvingKey {
    pub async fn generate(_circuit: &crate::circuit::ZkCircuit) -> Result<Self> {
        Ok(Self)
    }
}
