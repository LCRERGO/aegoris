use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::error::{ProviderError, ProviderResult};
use crate::provider::{Completion, CompletionRequest, LanguageModel};

type Responder = Box<dyn Fn(&CompletionRequest) -> ProviderResult<String> + Send + Sync>;

/// A `LanguageModel` that never touches the network.
///
/// This is first-class test infrastructure: the entire pipeline can run
/// end-to-end against canned model output.
pub struct FakeLanguageModel {
    responder: Responder,
    name: String,
    json_mode: bool,
}

impl FakeLanguageModel {
    pub fn new(
        responder: impl Fn(&CompletionRequest) -> ProviderResult<String> + Send + Sync + 'static,
    ) -> Self {
        Self {
            responder: Box::new(responder),
            name: "fake".to_string(),
            json_mode: true,
        }
    }

    /// Always return the same text.
    pub fn always(text: impl Into<String>) -> Self {
        let text = text.into();
        Self::new(move |_| Ok(text.clone()))
    }

    /// Return each response in turn, repeating the last one once exhausted.
    pub fn sequence(responses: Vec<String>) -> Self {
        let queue = Arc::new(Mutex::new(VecDeque::from(responses)));
        Self::new(move |_| {
            let mut queue = queue.lock().expect("fake queue poisoned");
            match queue.pop_front() {
                Some(next) => Ok(next),
                None => Err(ProviderError::Decode(
                    "no more canned responses".to_string(),
                )),
            }
        })
    }

    /// Always fail with the given error.
    pub fn failing(error: ProviderError) -> Self {
        let error = Arc::new(error);
        Self::new(move |_| Err(clone_error(&error)))
    }

    pub fn with_json_mode(mut self, enabled: bool) -> Self {
        self.json_mode = enabled;
        self
    }

    pub fn named(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
}

#[async_trait]
impl LanguageModel for FakeLanguageModel {
    async fn complete(&self, request: CompletionRequest) -> ProviderResult<Completion> {
        (self.responder)(&request).map(|text| Completion { text })
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn supports_json_mode(&self) -> bool {
        self.json_mode
    }
}

fn clone_error(error: &ProviderError) -> ProviderError {
    match error {
        ProviderError::Auth => ProviderError::Auth,
        ProviderError::RateLimit => ProviderError::RateLimit,
        ProviderError::Transport(message) => ProviderError::Transport(message.clone()),
        ProviderError::Decode(message) => ProviderError::Decode(message.clone()),
        ProviderError::Status { status, body } => ProviderError::Status {
            status: *status,
            body: body.clone(),
        },
        ProviderError::Refusal(message) => ProviderError::Refusal(message.clone()),
        ProviderError::Config(message) => ProviderError::Config(message.clone()),
    }
}
