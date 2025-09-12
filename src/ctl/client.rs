use tonic::transport::Channel;

pub struct KvStoreClient {
    inner: kvstore::kv_store_client::KvStoreClient<Channel>,
}

impl KvStoreClient {
    pub async fn connect(addr: &str) -> Result<Self, crate::ctl::types::KvCtlError> {
        let channel = Channel::from_shared(addr.to_string())
            .map_err(|e| crate::ctl::types::KvCtlError::InvalidArgument(e.to_string()))?
            .connect()
            .await?;

        Ok(Self {
            inner: kvstore::kv_store_client::KvStoreClient::new(channel),
        })
    }

    pub async fn get(&mut self, key: &str) -> Result<kvstore::GetResponse, tonic::Status> {
        let request = tonic::Request::new(kvstore::GetRequest {
            key: key.to_string(),
        });
        let response = self.inner.get(request).await?;
        Ok(response.into_inner())
    }

    pub async fn set(&mut self, key: &str, value: Vec<u8>, ttl_seconds: u64) -> Result<kvstore::SetResponse, tonic::Status> {
        let request = tonic::Request::new(kvstore::SetRequest {
            key: key.to_string(),
            value,
            ttl_seconds,
        });
        let response = self.inner.set(request).await?;
        Ok(response.into_inner())
    }

    pub async fn delete(&mut self, key: &str) -> Result<kvstore::DeleteResponse, tonic::Status> {
        let request = tonic::Request::new(kvstore::DeleteRequest {
            key: key.to_string(),
        });
        let response = self.inner.delete(request).await?;
        Ok(response.into_inner())
    }
}