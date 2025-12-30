//! Gemini API request and response types.

use serde::{Deserialize, Serialize};

use super::types::RefactoringOracleResponse;

/// Gemini API request structure
#[derive(Debug, Serialize)]
pub struct GeminiRequest {
    pub contents: Vec<GeminiContent>,
    #[serde(rename = "generationConfig")]
    pub generation_config: GeminiGenerationConfig,
}

#[derive(Debug, Serialize)]
pub struct GeminiContent {
    pub parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize)]
pub struct GeminiPart {
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct GeminiGenerationConfig {
    pub temperature: f32,
    #[serde(rename = "topK")]
    pub top_k: i32,
    #[serde(rename = "topP")]
    pub top_p: f32,
    #[serde(rename = "maxOutputTokens")]
    pub max_output_tokens: i32,
    #[serde(rename = "responseMimeType")]
    pub response_mime_type: String,
}

#[derive(Debug, Deserialize)]
pub struct GeminiResponse {
    pub candidates: Vec<GeminiCandidate>,
}

#[derive(Debug, Deserialize)]
pub struct GeminiCandidate {
    pub content: GeminiResponseContent,
}

#[derive(Debug, Deserialize)]
pub struct GeminiResponseContent {
    pub parts: Vec<GeminiResponsePart>,
}

#[derive(Debug, Deserialize)]
pub struct GeminiResponsePart {
    pub text: String,
}

/// Result from analyzing a single slice
#[derive(Debug, Clone)]
pub struct SliceAnalysisResult {
    /// Slice identifier
    pub slice_id: usize,
    /// Primary module/directory this slice covers
    pub primary_module: Option<String>,
    /// Oracle response for this slice
    pub response: RefactoringOracleResponse,
}
