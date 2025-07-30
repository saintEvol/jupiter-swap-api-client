use quote::{InternalQuoteRequest, QuoteRequest, QuoteResponse};
use reqwest::{Client, Error, Response};
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::time::Duration;
use swap::{SwapInstructionsResponse, SwapInstructionsResponseInternal, SwapRequest, SwapResponse};
use thiserror::Error;

pub mod quote;
pub mod route_plan_with_metadata;
pub mod serde_helpers;
pub mod swap;
pub mod transaction_config;

#[derive(Clone)]
pub struct JupiterSwapApiClient {
    pub base_path: String,
    pub quote_path: String,
    pub swap_path: String,
    pub swap_instructions_path: String,
    pub http_client: Client,
}

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("Request failed with status {status}: {body}")]
    RequestFailed {
        status: reqwest::StatusCode,
        body: String,
    },
    #[error("Failed to deserialize response: {0}")]
    DeserializationError(#[from] reqwest::Error),
}

async fn check_is_success(response: Response) -> Result<Response, ClientError> {
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(ClientError::RequestFailed { status, body });
    }
    Ok(response)
}

async fn check_status_code_and_deserialize<T: DeserializeOwned>(
    response: Response,
) -> Result<T, ClientError> {
    let response = check_is_success(response).await?;
    response
        .json::<T>()
        .await
        .map_err(ClientError::DeserializationError)
}

impl JupiterSwapApiClient {
    pub fn new(base_path: String) -> Result<Self, Error> {
        let quote_path = format!("{}/quote", base_path);
        let swap_path = format!("{}/swap", base_path);
        let swap_instructions_path = format!("{}/swap-instructions", base_path);
        let http_client = Client::builder()
            .http2_keep_alive_while_idle(true)
            .pool_idle_timeout(None)
            .http2_keep_alive_interval(Some(Duration::from_secs(10)))
            .build()?;
        Ok(Self {
            base_path,
            quote_path,
            swap_path,
            swap_instructions_path,
            http_client,
        })
    }

    pub async fn quote(&self, mut quote_request: QuoteRequest) -> Result<QuoteResponse, ClientError> {
        let url = &self.quote_path;
        let extra_args = quote_request.quote_args.take();
        let internal_quote_request = InternalQuoteRequest::from(quote_request);
        let response = self.http_client
            .get(url)
            .query(&internal_quote_request)
            .query(&extra_args)
            .send()
            .await?;
        check_status_code_and_deserialize(response).await
    }

    pub async fn swap(
        &self,
        swap_request: &SwapRequest,
        extra_args: Option<HashMap<String, String>>,
    ) -> Result<SwapResponse, ClientError> {
        let response = self.http_client
            .post(&self.swap_path)
            .query(&extra_args)
            .json(swap_request)
            .send()
            .await?;
        check_status_code_and_deserialize(response).await
    }

    pub async fn swap_instructions(
        &self,
        swap_request: &SwapRequest,
    ) -> Result<SwapInstructionsResponse, ClientError> {
        let response = self.http_client
            .post(&self.swap_instructions_path)
            .json(swap_request)
            .send()
            .await?;
        check_status_code_and_deserialize::<SwapInstructionsResponseInternal>(response)
            .await
            .map(Into::into)
    }
}
