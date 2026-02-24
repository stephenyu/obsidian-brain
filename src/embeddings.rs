use crate::config::MODEL_ID;
use anyhow::Result;
use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config as BertConfig, DTYPE};
use hf_hub::api::sync::Api;
use tokenizers::Tokenizer;

pub struct EmbeddingEngine {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
}

impl EmbeddingEngine {
    pub fn new() -> Result<Self> {
        let device = Device::new_metal(0).unwrap_or(Device::Cpu);
        let api = Api::new()?;
        let repo = api.model(MODEL_ID.to_string());

        let config_path = repo.get("config.json")?;
        let tokenizer_path = repo.get("tokenizer.json")?;
        let weights_path = repo.get("model.safetensors")?;

        let config: BertConfig = serde_json::from_str(&std::fs::read_to_string(config_path)?)?;
        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Tokenizer error: {e}"))?;

        let vb = unsafe { VarBuilder::from_mmaped_safetensors(&[weights_path], DTYPE, &device)? };
        let model = BertModel::load(vb, &config)?;

        Ok(Self {
            model,
            tokenizer,
            device,
        })
    }

    pub fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // Tokenize with padding
        let mut encodings = Vec::with_capacity(texts.len());
        for text in &texts {
            let encoding = self
                .tokenizer
                .encode(text.as_str(), true)
                .map_err(|e| anyhow::anyhow!("Encode error: {e}"))?;
            encodings.push(encoding);
        }

        // Find max length in this batch for padding
        let max_len = encodings
            .iter()
            .map(|e| e.get_ids().len())
            .max()
            .unwrap_or(0);

        let batch_size = texts.len();
        let mut all_ids = Vec::with_capacity(batch_size * max_len);
        let mut all_type_ids = Vec::with_capacity(batch_size * max_len);
        let mut all_attention_mask = Vec::with_capacity(batch_size * max_len);

        for encoding in encodings {
            let ids = encoding.get_ids();
            let type_ids = encoding.get_type_ids();
            let len = ids.len();

            all_ids.extend_from_slice(ids);
            all_ids.extend(std::iter::repeat(0).take(max_len - len));

            all_type_ids.extend_from_slice(type_ids);
            all_type_ids.extend(std::iter::repeat(0).take(max_len - len));

            let mask: Vec<u32> = encoding.get_attention_mask().to_vec();
            all_attention_mask.extend_from_slice(&mask);
            all_attention_mask.extend(std::iter::repeat(0).take(max_len - len));
        }

        let input_ids = Tensor::from_slice(&all_ids, (batch_size, max_len), &self.device)?;
        let token_type_ids =
            Tensor::from_slice(&all_type_ids, (batch_size, max_len), &self.device)?;
        let attention_mask =
            Tensor::from_slice(&all_attention_mask, (batch_size, max_len), &self.device)?;

        // Forward pass — shape: [batch_size, seq_len, hidden_size]
        let output = self.model.forward(&input_ids, &token_type_ids, Some(&attention_mask))?;

        // Mean pool over sequence dimension, considering the attention mask
        // [batch_size, seq_len, hidden_size] * [batch_size, seq_len, 1]
        let mask_expanded = attention_mask.unsqueeze(2)?.to_dtype(DTYPE)?;
        let masked_output = output.broadcast_mul(&mask_expanded)?;
        let sum_emb = masked_output.sum(1)?; // [batch_size, hidden_size]
        
        let sum_mask = mask_expanded.sum(1)?; // [batch_size, 1]
        let mean_emb = sum_emb.broadcast_div(&sum_mask)?;

        // L2 normalize: [batch_size, hidden_size]
        let norm = mean_emb.sqr()?.sum_keepdim(1)?.sqrt()?;
        let normalized = mean_emb.broadcast_div(&norm)?;

        let results_vec = normalized.to_vec2::<f32>()?;
        Ok(results_vec)
    }
}
