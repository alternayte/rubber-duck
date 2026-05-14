use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub context_window: u32,
}

pub const MODELS: &[ModelInfo] = &[
    ModelInfo { id: "deepseek/deepseek-chat-v4-0324:free", name: "DeepSeek v4 Flash", context_window: 131072 },
    ModelInfo { id: "deepseek/deepseek-chat", name: "DeepSeek v4", context_window: 131072 },
    ModelInfo { id: "anthropic/claude-sonnet-4", name: "Claude Sonnet 4", context_window: 200000 },
    ModelInfo { id: "anthropic/claude-haiku-4", name: "Claude Haiku 4", context_window: 200000 },
    ModelInfo { id: "openai/gpt-4o", name: "GPT-4o", context_window: 128000 },
    ModelInfo { id: "openai/gpt-4o-mini", name: "GPT-4o Mini", context_window: 128000 },
    ModelInfo { id: "meta-llama/llama-4-scout", name: "Llama 4 Scout", context_window: 512000 },
    ModelInfo { id: "qwen/qwen3-235b", name: "Qwen 3 235B", context_window: 131072 },
];

pub const DEFAULT_MODEL: &str = "deepseek/deepseek-chat-v4-0324:free";
