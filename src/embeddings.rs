use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config as BertConfig, DTYPE};
use tokenizers::Tokenizer;
use hf_hub::api::sync::Api;
use anyhow::Result;

const MODEL_ID: &str = "BAAI/bge-small-en-v1.5";

pub struct EmbeddingEngine {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
}

impl EmbeddingEngine {
    pub fn new() -> Result<Self> {
        let device = Device::Cpu;
        let api = Api::new()?;
        let repo = api.model(MODEL_ID.to_string());

        let config_path = repo.get("config.json")?;
        let tokenizer_path = repo.get("tokenizer.json")?;
        let weights_path = repo.get("model.safetensors")?;

        let config: BertConfig = serde_json::from_str(&std::fs::read_to_string(config_path)?)?;
        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Tokenizer error: {e}"))?;

        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[weights_path], DTYPE, &device)?
        };
        let model = BertModel::load(vb, &config)?;

        Ok(Self { model, tokenizer, device })
    }

    pub fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        let mut results = Vec::with_capacity(texts.len());

        for text in &texts {
            let encoding = self.tokenizer
                .encode(text.as_str(), true)
                .map_err(|e| anyhow::anyhow!("Encode error: {e}"))?;

            let ids: Vec<u32> = encoding.get_ids().to_vec();
            let type_ids: Vec<u32> = encoding.get_type_ids().to_vec();
            let n_tokens = ids.len();

            let input_ids = Tensor::new(ids.as_slice(), &self.device)?.unsqueeze(0)?;
            let token_type_ids = Tensor::new(type_ids.as_slice(), &self.device)?.unsqueeze(0)?;

            // Forward pass — shape: [1, seq_len, hidden_size]
            let output = self.model.forward(&input_ids, &token_type_ids, None)?;

            // Mean pool over sequence dimension: [1, hidden_size]
            let mean_emb = (output.sum(1)? / (n_tokens as f64))?;

            // L2 normalize: [1, hidden_size]
            let norm = mean_emb.sqr()?.sum_keepdim(1)?.sqrt()?;
            let normalized = mean_emb.broadcast_div(&norm)?;

            let embedding: Vec<f32> = normalized.squeeze(0)?.to_vec1()?;
            results.push(embedding);
        }

        Ok(results)
    }
}
