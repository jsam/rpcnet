use std::sync::Arc;

use crate::errors::RpcError;


// Type alias for RPC handler functions
pub(crate) type HandlerFn = Arc<dyn Fn(Vec<u8>) -> Result<Vec<u8>, RpcError> + Send + Sync>;
