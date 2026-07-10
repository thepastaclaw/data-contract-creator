use anyhow::{anyhow, Result};
use serde_json::json;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, RequestMode, Response};

/// Service for interacting with OpenAI API
pub struct OpenAiService;

impl OpenAiService {
    /// Context shared by generation and edit prompts
    const SHARED_REQUIREMENTS: &'static str = r#"
*Requirements*:
The following requirements must be met in Dash Platform data contracts:
 - Indexes may only have "asc" sort order.
 - All "string" properties that are used in indexes must specify "maxLength", which must be no more than 63.
 - All "array" properties that are used in indexes must specify "maxItems", and it must be less than or equal to 255.
 - All "array" properties must specify `"byteArray": true`.
 - All "object" properties must define at least 1 property within themselves.
 - All properties must define a "position" field, which is a number starting at 0, incrementing for each property.
 - Contested unique indexes can be marked with `contested` to resolve ownership through governance.
"#;

    /// Context prepended before shared requirements when creating a new contract
    const FIRST_PROMPT_PRE: &'static str = r#"
I'm going to ask you to generate a Dash Platform data contract after giving you some context and rules. 

*Background info*: 
Dash Platform is a blockchain for decentralized applications that are backed by data contracts. 
Data contracts are JSON schemas that are meant to define the structures of data an application can store. 
They must define at least one document type, where a document type defines a type of document that can be submitted to a data contract.
This editor currently supports document-type schemas and their indexes/properties. Do not emit root-level `tokens` or `groups` objects because the application cannot import or edit those sections yet.

*Example*: 
Here is a simple, modern-style example of a data contract with one document type, "post" (newer document-type features such as contested indexes may also be used when appropriate):

{"post":{"type":"object","description":"A public post in a social app","$comment":"Stores user-authored posts with metadata","properties":{"title":{"position":0,"type":"string","description":"Short post title","maxLength":63},"body":{"position":1,"type":"string","description":"Main content of the post","maxLength":1024},"authorId":{"position":2,"type":"array","description":"Identifier of the post author","byteArray":true,"minItems":32,"maxItems":32},"createdAt":{"position":3,"type":"integer","description":"Unix timestamp in milliseconds"}},"indices":[{"name":"authorId","properties":[{"authorId":"asc"}]},{"name":"createdAt","properties":[{"createdAt":"asc"}]}],"required":["title","body","authorId","createdAt"],"additionalProperties":false}}

While this example data contract only has one document type, data contracts should usually have more than one. For example, the social app above could also have document types for "comment" and "like" so users can interact with posts. Maybe the developer also wants to have user profiles, so they could include a "userProfile" document type.
"#;

    /// Context appended after shared requirements when creating a new contract
    const FIRST_PROMPT_POST: &'static str = r#"
*App description*: 
Now I will give you a user prompt that describes the application that you will generate a data contract for.

When creating the data contract, please:
 - Include descriptions for every document type and property. Be creative, extensive, and utilize multiple document types if possible.
 - Include both "description" and "$comment" fields for every document type (at the same level as "type", "properties", etc.).
 - Include indexes for any properties that it makes sense for a useful app to index. More is better. 
 - Return ONLY the JSON object, no markdown code fences, no explanation text.
 - Double check that all requirements and requests above are met. Again, all "array" properties must specify `"byteArray": true`.

App description: 

"#;

    /// Context prepended before shared requirements for follow-up prompts
    const SECOND_PROMPT_PRE: &'static str = r#"
I'm going to ask you to make some changes to a Dash Platform data contract after giving you some context and rules.

*Background info*:
Dash Platform data contracts are JSON schemas that define the structures of data an application can store.
This editor currently supports document-type schemas and their indexes/properties. Keep every top-level key as a document type; do not add root-level "groups" or "tokens" objects because the application cannot import or edit those sections yet.
"#;

    /// Context appended after shared requirements for follow-up prompts
    const SECOND_PROMPT_POST: &'static str = r#"
*Changes to be made*:
Make the following change(s) to this Dash Platform data contract JSON schema, along with any other edits needed to keep the resulting schema valid according to all the rules above.
Return ONLY the JSON object, no markdown code fences, no explanation text:

"#;

    /// Calls OpenAI API to generate or modify data contracts
    pub async fn generate_contract(prompt: &str, existing_schema: Option<&str>) -> Result<String> {
        let full_prompt = if let Some(schema) = existing_schema {
            format!(
                "{}{}{}\n\nExisting schema:\n{}\n\nUser request:\n{}",
                Self::SECOND_PROMPT_PRE,
                Self::SHARED_REQUIREMENTS,
                Self::SECOND_PROMPT_POST,
                schema,
                prompt
            )
        } else {
            format!(
                "{}{}{}{}",
                Self::FIRST_PROMPT_PRE,
                Self::SHARED_REQUIREMENTS,
                Self::FIRST_PROMPT_POST,
                prompt
            )
        };

        Self::call_api(&full_prompt).await
    }

    /// Makes the actual API call to OpenAI
    async fn call_api(prompt: &str) -> Result<String> {
        let params = json!({
            "model": "gpt-5-mini",
            "messages": [{"role": "user", "content": prompt}],
            "response_format": {"type": "json_object"},
            // gpt-5-mini only supports the default temperature (1); sending any
            // other value returns a 400, so we omit the field entirely.
            "max_completion_tokens": 8192
        });

        let mut opts = RequestInit::new();
        let headers =
            web_sys::Headers::new().map_err(|e| anyhow!("Failed to create headers: {:?}", e))?;

        headers
            .append("Content-Type", "application/json")
            .map_err(|e| anyhow!("Failed to set content type: {:?}", e))?;

        // Note: For local testing, uncomment and add your API key:
        // headers.append("Authorization", &format!("Bearer {}", "YOUR_API_KEY_HERE"))
        //     .map_err(|e| anyhow!("Failed to set authorization: {:?}", e))?;

        opts.method("POST");
        opts.headers(&headers);
        opts.body(Some(&JsValue::from_str(&params.to_string())));
        opts.mode(RequestMode::Cors);

        // Use the Lambda endpoint for production, OpenAI directly for local testing
        let url = "https://22vazdmku2qz3prrn57elhdj2i0wyejr.lambda-url.us-west-2.on.aws/";
        // For local testing, use: "https://api.openai.com/v1/chat/completions" (model: gpt-5-mini)

        let request = Request::new_with_str_and_init(url, &opts)
            .map_err(|e| anyhow!("Failed to create request: {:?}", e))?;

        let window = web_sys::window().ok_or_else(|| anyhow!("Failed to obtain window object"))?;

        let response = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(|e| {
                anyhow!(
                    "Fetch request failed: {:?}",
                    e.as_string().unwrap_or_default()
                )
            })?;

        let response: Response = response.dyn_into().map_err(|e| {
            anyhow!(
                "Failed to convert response: {:?}",
                e.as_string().unwrap_or_default()
            )
        })?;

        let text = JsFuture::from(
            response
                .text()
                .map_err(|_| anyhow!("Failed to read response text"))?,
        )
        .await
        .map_err(|e| {
            anyhow!(
                "Failed to get response text: {:?}",
                e.as_string().unwrap_or_default()
            )
        })?;

        let text = text
            .as_string()
            .ok_or_else(|| anyhow!("Failed to convert response to string"))?;

        if !response.ok() {
            let status = response.status();
            let error_message = Self::extract_error_message(&text);
            return Err(anyhow!("HTTP {} error from API: {}", status, error_message));
        }

        Self::extract_json_schema(&text)
    }

    /// Extracts error message from API response
    fn extract_error_message(text: &str) -> String {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(text) {
            json.get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or(text)
                .to_string()
        } else {
            text.to_string()
        }
    }

    /// Extracts JSON schema from OpenAI response
    fn extract_json_schema(response_text: &str) -> Result<String> {
        let json: serde_json::Value = serde_json::from_str(response_text)
            .map_err(|e| anyhow!("Failed to parse API response: {}", e))?;

        let content = json
            .get("choices")
            .and_then(|choices| choices.get(0))
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
            .and_then(|content| content.as_str())
            .ok_or_else(|| anyhow!("Invalid response format from API"))?;

        // `response_format: {"type": "json_object"}` guarantees the API returns
        // a single JSON object in `content`. Parse it directly and require an
        // object. Fenced, prose-wrapped, or non-object content means the request
        // contract was not honored (e.g. a proxy that stripped `response_format`),
        // so fail loudly instead of trying to salvage a substring.
        let trimmed = content.trim();
        let value: serde_json::Value = serde_json::from_str(trimmed)
            .map_err(|e| anyhow!("API response was not valid JSON: {}", e))?;

        if !value.is_object() {
            return Err(anyhow!("API response JSON was not an object"));
        }

        Ok(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::OpenAiService;

    fn api_response(content: &str) -> String {
        serde_json::json!({
            "choices": [{"message": {"content": content}}]
        })
        .to_string()
    }

    #[test]
    fn parses_pure_json_object_directly() {
        let response = api_response("{\"post\":{\"type\":\"object\"}}");
        let schema = OpenAiService::extract_json_schema(&response).unwrap();
        assert_eq!(schema, "{\"post\":{\"type\":\"object\"}}");
    }

    #[test]
    fn trims_surrounding_whitespace_from_pure_json() {
        let response = api_response("  \n{\"a\":1}\n ");
        let schema = OpenAiService::extract_json_schema(&response).unwrap();
        assert_eq!(schema, "{\"a\":1}");
    }

    #[test]
    fn rejects_markdown_fenced_json() {
        // Under JSON mode the API must return a bare object; fences mean the
        // request contract was not honored and must fail loudly.
        let response = api_response("```json\n{\"a\":1}\n```");
        assert!(OpenAiService::extract_json_schema(&response).is_err());
    }

    #[test]
    fn rejects_prose_wrapped_json() {
        let response = api_response("Here is your contract: {\"a\":1}");
        assert!(OpenAiService::extract_json_schema(&response).is_err());
    }

    #[test]
    fn rejects_non_object_json() {
        // A syntactically valid array/scalar is still not a contract object.
        assert!(OpenAiService::extract_json_schema(&api_response("[1,2,3]")).is_err());
        assert!(OpenAiService::extract_json_schema(&api_response("42")).is_err());
    }

    #[test]
    fn errors_when_no_json_present() {
        let response = api_response("sorry, I cannot help with that");
        assert!(OpenAiService::extract_json_schema(&response).is_err());
    }
}
