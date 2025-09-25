use tonic::transport::Channel;

use super::types::{DeleteResponse, GetResponse, SetResponse};
pub struct KvStoreClient {
    inner: KvStoreClient<Channel>,
}

impl KvStoreClient {
    pub async fn connect(addr: &str) -> Result<Self, crate::ctl::types::KvCtlError> {
        let channel = Channel::from_shared(addr.to_string())
            .map_err(|e| crate::ctl::types::KvCtlError::InvalidArgument(e.to_string()))?
            .connect()
            .await?;

        Ok(Self {
            inner: KvStoreClient::new(channel),
        })
    }

    pub async fn get(&mut self, key: &str) -> Result<GetResponse, tonic::Status> {
        let request = tonic::Request::new(GetRequest {
            key: key.to_string(),
        });
        let response = self.inner.get(request).await?;
        Ok(response.into_inner())
    }

    pub async fn set(
        &mut self,
        key: &str,
        value: Vec<u8>,
        ttl_seconds: u64,
    ) -> Result<SetResponse, tonic::Status> {
        let request = tonic::Request::new(SetRequest {
            key: key.to_string(),
            value,
            ttl_seconds,
        });
        let response = self.inner.set(request).await?;
        Ok(response.into_inner())
    }

    pub async fn delete(&mut self, key: &str) -> Result<DeleteResponse, tonic::Status> {
        let request = tonic::Request::new(DeleteRequest {
            key: key.to_string(),
        });
        let response = self.inner.delete(request).await?;
        Ok(response.into_inner())
    }
}
